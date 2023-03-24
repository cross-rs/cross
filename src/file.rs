use std::borrow::Cow;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
#[cfg(target_family = "windows")]
use std::path::Prefix;
use std::path::{Component, Path, PathBuf};

use crate::errors::*;

pub trait ToUtf8 {
    fn to_utf8(&self) -> Result<&str>;
}

impl ToUtf8 for OsStr {
    fn to_utf8(&self) -> Result<&str> {
        self.to_str()
            .ok_or_else(|| eyre::eyre!("unable to convert `{self:?}` to UTF-8 string"))
    }
}

impl ToUtf8 for Path {
    fn to_utf8(&self) -> Result<&str> {
        self.as_os_str().to_utf8()
    }
}

pub trait PathExt {
    fn as_posix_relative(&self) -> Result<String>;
    fn as_posix_absolute(&self) -> Result<String>;
}

#[cfg(target_family = "windows")]
fn format_prefix(prefix: &str) -> Result<String> {
    match prefix {
        "" => Ok("".to_owned()),
        _ => Ok(format!("/mnt/{}", prefix.to_lowercase())),
    }
}

#[cfg(target_family = "windows")]
fn fmt_disk(disk: u8) -> String {
    (disk as char).to_string()
}

#[cfg(target_family = "windows")]
fn fmt_ns_disk(disk: &std::ffi::OsStr) -> Result<String> {
    let disk = disk.to_utf8()?;
    Ok(match disk.len() {
        // ns can be similar to `\\.\COM42`, or also `\\.\C:\`
        2 => {
            let c = disk.chars().next().expect("cannot be empty");
            if c.is_ascii_alphabetic() && disk.ends_with(':') {
                fmt_disk(c as u8)
            } else {
                disk.to_owned()
            }
        }
        _ => disk.to_owned(),
    })
}

#[cfg(target_family = "windows")]
fn fmt_unc(server: &std::ffi::OsStr, volume: &std::ffi::OsStr) -> Result<String> {
    let server = server.to_utf8()?;
    let volume = volume.to_utf8()?;
    let bytes = volume.as_bytes();
    if server == "localhost"
        && bytes.len() == 2
        && bytes[1] == b'$'
        && bytes[0].is_ascii_alphabetic()
    {
        Ok(fmt_disk(bytes[0]))
    } else {
        Ok(format!("{}/{}", server, volume))
    }
}

impl PathExt for Path {
    fn as_posix_relative(&self) -> Result<String> {
        if cfg!(target_os = "windows") {
            let push = |p: &mut String, c: &str| {
                if !p.is_empty() && p != "/" {
                    p.push('/');
                }
                p.push_str(c);
            };

            // iterate over components to join them
            let mut output = String::new();
            for component in self.components() {
                match component {
                    Component::Prefix(prefix) => {
                        eyre::bail!("unix paths cannot handle windows prefix {prefix:?}.")
                    }
                    Component::RootDir => output = "/".to_owned(),
                    Component::CurDir => push(&mut output, "."),
                    Component::ParentDir => push(&mut output, ".."),
                    Component::Normal(path) => push(&mut output, path.to_utf8()?),
                }
            }
            Ok(output)
        } else {
            self.to_utf8().map(|x| x.to_owned())
        }
    }

    #[cfg(not(target_family = "windows"))]
    fn as_posix_absolute(&self) -> Result<String> {
        absolute_path(self)?.to_utf8().map(ToOwned::to_owned)
    }

    // this is similar to as_posix_relative, but it handles drive
    // separators and will only work with absolute paths.
    #[cfg(target_family = "windows")]
    fn as_posix_absolute(&self) -> Result<String> {
        let path = absolute_path(self)?;

        let push = |p: &mut String, c: &str, r: bool| {
            if !r {
                p.push('/');
            }
            p.push_str(c);
        };
        // iterate over components to join them
        let mut output = String::new();
        let mut root_prefix = String::new();
        let mut was_root = false;
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => {
                    root_prefix = match prefix.kind() {
                        Prefix::Verbatim(verbatim) => verbatim.to_utf8()?.to_owned(),
                        Prefix::VerbatimUNC(server, volume) => fmt_unc(server, volume)?,
                        // we should never get this, but it's effectively just
                        // a root_prefix since we force absolute paths.
                        Prefix::VerbatimDisk(disk) => fmt_disk(disk),
                        Prefix::UNC(server, volume) => fmt_unc(server, volume)?,
                        Prefix::DeviceNS(ns) => fmt_ns_disk(ns)?,
                        Prefix::Disk(disk) => fmt_disk(disk),
                    }
                }
                Component::RootDir => output = format!("{}/", format_prefix(&root_prefix)?),
                Component::CurDir => push(&mut output, ".", was_root),
                Component::ParentDir => push(&mut output, "..", was_root),
                Component::Normal(path) => push(&mut output, path.to_utf8()?, was_root),
            }
            was_root = component == Component::RootDir;
        }

        // remove trailing '/'
        if was_root {
            output.truncate(output.len() - 1);
        }

        Ok(output)
    }
}

pub fn read<P>(path: P) -> Result<String>
where
    P: AsRef<Path>,
{
    read_(path.as_ref())
}

pub fn create_dir_all(path: impl AsRef<Path>) -> Result<()> {
    fs::create_dir_all(path.as_ref())
        .wrap_err_with(|| format!("couldn't create directory {:?}", path.as_ref()))
}

fn read_(path: &Path) -> Result<String> {
    let mut s = String::new();
    File::open(path)
        .wrap_err_with(|| format!("couldn't open {path:?}"))?
        .read_to_string(&mut s)
        .wrap_err_with(|| format!("couldn't read {path:?}"))?;
    Ok(s)
}

pub fn canonicalize(path: impl AsRef<Path>) -> Result<PathBuf> {
    _canonicalize(path.as_ref())
        .wrap_err_with(|| format!("when canonicalizing path `{:?}`", path.as_ref()))
}

fn _canonicalize(path: &Path) -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // Docker does not support UNC paths, this will try to not use UNC paths
        dunce::canonicalize(path).map_err(Into::into)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Path::new(&path).canonicalize().map_err(Into::into)
    }
}

fn is_wsl_absolute(path: &str) -> bool {
    if !cfg!(target_os = "windows") {
        return false;
    }
    let path = path.strip_prefix("/mnt/").unwrap_or(path);
    let maybe_drive = path.split_once('/').map_or(path, |x| x.0);

    maybe_drive.len() == 1 && matches!(maybe_drive.chars().next(), Some('a'..='z'))
}

// Fix for issue #581. target_dir must be absolute.
pub fn absolute_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let as_ref = path.as_ref();
    Ok(
        if as_ref.is_absolute()
            || cfg!(target_family = "windows") && is_wsl_absolute(as_ref.to_utf8()?)
        {
            as_ref.to_path_buf()
        } else {
            env::current_dir()?.join(path)
        },
    )
}

/// Pretty format a file path. Also removes the path prefix from a command if wanted
pub fn pretty_path(path: impl AsRef<Path>, strip: impl for<'a> Fn(&'a str) -> bool) -> String {
    let path = path.as_ref();
    // TODO: Use Path::file_prefix
    let file_stem = path.file_stem();
    let file_name = path.file_name();
    let path = if let (Some(file_stem), Some(file_name)) = (file_stem, file_name) {
        if let Some(file_name) = file_name.to_str() {
            if strip(file_name) {
                Cow::Borrowed(file_stem)
            } else {
                Cow::Borrowed(path.as_os_str())
            }
        } else {
            Cow::Borrowed(path.as_os_str())
        }
    } else {
        maybe_canonicalize(path)
    };

    if let Some(path) = path.to_str() {
        shell_escape(path).to_string()
    } else {
        format!("{path:?}")
    }
}

pub fn shell_escape(string: &str) -> Cow<'_, str> {
    let escape: &[char] = if cfg!(target_os = "windows") {
        &['%', '$', '`', '!', '"']
    } else {
        &['$', '\'', '\\', '!', '"']
    };

    if string.contains(escape) {
        Cow::Owned(format!("{string:?}"))
    } else if string.contains(' ') {
        Cow::Owned(format!("\"{string}\""))
    } else {
        Cow::Borrowed(string)
    }
}

#[must_use]
pub fn maybe_canonicalize(path: &Path) -> Cow<'_, OsStr> {
    canonicalize(path).map_or_else(
        |_| path.as_os_str().into(),
        |p| Cow::Owned(p.as_os_str().to_owned()),
    )
}

pub fn write_file(path: impl AsRef<Path>, overwrite: bool) -> Result<File> {
    let path = path.as_ref();
    create_dir_all(
        path.parent()
            .ok_or_else(|| eyre::eyre!("could not find parent directory for `{path:?}`"))?,
    )?;

    let mut open = fs::OpenOptions::new();
    open.write(true);

    if overwrite {
        open.truncate(true).create(true);
    } else {
        open.create_new(true);
    }

    open.open(path)
        .wrap_err(format!("couldn't write to file `{path:?}`"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Debug;

    macro_rules! p {
        ($path:expr) => {
            Path::new($path)
        };
    }

    #[track_caller]
    fn result_eq<T: PartialEq + Eq + Debug>(x: Result<T>, y: Result<T>) {
        match (x, y) {
            (Ok(x), Ok(y)) => assert_eq!(x, y),
            (x, y) => panic!("assertion failed: `(left == right)`\nleft: {x:?}\nright: {y:?}"),
        }
    }

    #[test]
    fn as_posix_relative() {
        result_eq(
            p!(".").join("..").as_posix_relative(),
            Ok("./..".to_owned()),
        );
        result_eq(p!(".").join("/").as_posix_relative(), Ok("/".to_owned()));
        result_eq(
            p!("foo").join("bar").as_posix_relative(),
            Ok("foo/bar".to_owned()),
        );
        result_eq(
            p!("/foo").join("bar").as_posix_relative(),
            Ok("/foo/bar".to_owned()),
        );
    }

    #[test]
    #[cfg(target_family = "windows")]
    fn as_posix_prefix() {
        assert_eq!(p!("C:").join(".."), p!("C:.."));
        assert!(p!("C:").join("..").as_posix_relative().is_err());
    }

    #[test]
    #[cfg(target_family = "windows")]
    fn is_absolute_wslpath() {
        assert!(is_wsl_absolute("/mnt/c/Users"));
        assert!(is_wsl_absolute("/mnt/c"));
        assert!(is_wsl_absolute("/mnt/z/Users"));
        assert!(!is_wsl_absolute("/mnt"));
        assert!(!is_wsl_absolute("/mnt/cc"));
        assert!(!is_wsl_absolute("/mnt/zc"));
    }

    #[test]
    #[cfg(target_family = "windows")]
    fn as_posix_with_drive() {
        use regex::Regex;

        result_eq(p!(r"C:\").as_posix_absolute(), Ok("/mnt/c".to_owned()));
        result_eq(
            p!(r"C:\Users").as_posix_absolute(),
            Ok("/mnt/c/Users".to_owned()),
        );
        result_eq(
            p!(r"\\localhost\c$\Users").as_posix_absolute(),
            Ok("/mnt/c/Users".to_owned()),
        );
        result_eq(p!(r"\\.\C:\").as_posix_absolute(), Ok("/mnt/c".to_owned()));
        result_eq(
            p!(r"\\.\C:\Users").as_posix_absolute(),
            Ok("/mnt/c/Users".to_owned()),
        );

        result_eq(
            p!(r"/mnt/c/Users").as_posix_absolute(),
            Ok("/mnt/c/Users".to_owned()),
        );
        result_eq(p!(r"/mnt/c").as_posix_absolute(), Ok("/mnt/c".to_owned()));

        let regex = Regex::new("/mnt/[A-Za-z]/mnt").unwrap();
        let result = p!(r"/mnt").as_posix_absolute();
        assert!(result.is_ok());
        assert!(regex.is_match(&result.unwrap()));
    }

    #[test]
    #[cfg(target_family = "windows")]
    fn pretty_path_windows() {
        assert_eq!(
            pretty_path("C:\\path\\bin\\cargo.exe", |f| f.contains("cargo")),
            "cargo".to_owned()
        );
        assert_eq!(
            pretty_path("C:\\Program Files\\Docker\\bin\\docker.exe", |_| false),
            "\"C:\\Program Files\\Docker\\bin\\docker.exe\"".to_owned()
        );
        assert_eq!(
            pretty_path("C:\\Program Files\\single'quote\\cargo.exe", |c| c
                .contains("cargo")),
            "cargo".to_owned()
        );
        assert_eq!(
            pretty_path("C:\\Program Files\\single'quote\\cargo.exe", |_| false),
            "\"C:\\Program Files\\single'quote\\cargo.exe\"".to_owned()
        );
        assert_eq!(
            pretty_path("C:\\Program Files\\%not_var%\\cargo.exe", |_| false),
            "\"C:\\\\Program Files\\\\%not_var%\\\\cargo.exe\"".to_owned()
        );
    }

    #[test]
    #[cfg(target_family = "unix")]
    fn pretty_path_linux() {
        assert_eq!(
            pretty_path("/usr/bin/cargo", |f| f.contains("cargo")),
            "cargo".to_owned()
        );
        assert_eq!(
            pretty_path("/home/user/my rust/bin/cargo", |_| false),
            "\"/home/user/my rust/bin/cargo\"".to_owned(),
        );
        assert_eq!(
            pretty_path("/home/user/single'quote/cargo", |c| c.contains("cargo")),
            "cargo".to_owned()
        );
        assert_eq!(
            pretty_path("/home/user/single'quote/cargo", |_| false),
            "\"/home/user/single'quote/cargo\"".to_owned()
        );
    }
}
