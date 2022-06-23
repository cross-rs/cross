use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
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
    fn as_posix(&self) -> Result<String>;
}

fn push_posix_path(path: &mut String, component: &str) {
    if !path.is_empty() && path != "/" {
        path.push('/');
    }
    path.push_str(component);
}

impl PathExt for Path {
    fn as_posix(&self) -> Result<String> {
        if cfg!(target_os = "windows") {
            // iterate over components to join them
            let mut output = String::new();
            for component in self.components() {
                match component {
                    Component::Prefix(prefix) => {
                        eyre::bail!("unix paths cannot handle windows prefix {prefix:?}.")
                    }
                    Component::RootDir => output = "/".to_string(),
                    Component::CurDir => push_posix_path(&mut output, "."),
                    Component::ParentDir => push_posix_path(&mut output, ".."),
                    Component::Normal(path) => push_posix_path(&mut output, path.to_utf8()?),
                }
            }
            Ok(output)
        } else {
            self.to_utf8().map(|x| x.to_string())
        }
    }
}

pub fn read<P>(path: P) -> Result<String>
where
    P: AsRef<Path>,
{
    read_(path.as_ref())
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
        dunce::canonicalize(&path).map_err(Into::into)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Path::new(&path).canonicalize().map_err(Into::into)
    }
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

pub fn maybe_canonicalize(path: &Path) -> Cow<'_, OsStr> {
    canonicalize(path)
        .map(|p| Cow::Owned(p.as_os_str().to_owned()))
        .unwrap_or_else(|_| path.as_os_str().into())
}

pub fn write_file(path: impl AsRef<Path>, overwrite: bool) -> Result<File> {
    let path = path.as_ref();
    fs::create_dir_all(
        &path
            .parent()
            .ok_or_else(|| eyre::eyre!("could not find parent directory for `{path:?}`"))?,
    )
    .wrap_err_with(|| format!("couldn't create directory `{path:?}`"))?;

    let mut open = fs::OpenOptions::new();
    open.write(true);

    if overwrite {
        open.truncate(true).create(true);
    } else {
        open.create_new(true);
    }

    open.open(&path)
        .wrap_err(format!("couldn't write to file `{path:?}`"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Debug;

    fn result_eq<T: PartialEq + Eq + Debug>(x: Result<T>, y: Result<T>) {
        match (x, y) {
            (Ok(x), Ok(y)) => assert_eq!(x, y),
            (x, y) => panic!("assertion failed: `(left == right)`\nleft: {x:?}\nright: {y:?}"),
        }
    }

    #[test]
    fn as_posix() {
        result_eq(Path::new(".").join("..").as_posix(), Ok("./..".to_string()));
        result_eq(Path::new(".").join("/").as_posix(), Ok("/".to_string()));
        result_eq(
            Path::new("foo").join("bar").as_posix(),
            Ok("foo/bar".to_string()),
        );
        result_eq(
            Path::new("/foo").join("bar").as_posix(),
            Ok("/foo/bar".to_string()),
        );
    }

    #[test]
    #[cfg(target_family = "windows")]
    fn as_posix_prefix() {
        assert_eq!(Path::new("C:").join(".."), Path::new("C:.."));
        assert!(Path::new("C:").join("..").as_posix().is_err());
    }

    #[test]
    #[cfg(target_family = "windows")]
    fn pretty_path_windows() {
        assert_eq!(
            pretty_path("C:\\path\\bin\\cargo.exe", |f| f.contains("cargo")),
            "cargo".to_string()
        );
        assert_eq!(
            pretty_path("C:\\Program Files\\Docker\\bin\\docker.exe", |_| false),
            "\"C:\\Program Files\\Docker\\bin\\docker.exe\"".to_string()
        );
        assert_eq!(
            pretty_path("C:\\Program Files\\single'quote\\cargo.exe", |c| c
                .contains("cargo")),
            "cargo".to_string()
        );
        assert_eq!(
            pretty_path("C:\\Program Files\\single'quote\\cargo.exe", |_| false),
            "\"C:\\Program Files\\single'quote\\cargo.exe\"".to_string()
        );
        assert_eq!(
            pretty_path("C:\\Program Files\\%not_var%\\cargo.exe", |_| false),
            "\"C:\\\\Program Files\\\\%not_var%\\\\cargo.exe\"".to_string()
        );
    }

    #[test]
    #[cfg(target_family = "unix")]
    fn pretty_path_linux() {
        assert_eq!(
            pretty_path("/usr/bin/cargo", |f| f.contains("cargo")),
            "cargo".to_string()
        );
        assert_eq!(
            pretty_path("/home/user/my rust/bin/cargo", |_| false),
            "\"/home/user/my rust/bin/cargo\"".to_string(),
        );
        assert_eq!(
            pretty_path("/home/user/single'quote/cargo", |c| c.contains("cargo")),
            "cargo".to_string()
        );
        assert_eq!(
            pretty_path("/home/user/single'quote/cargo", |_| false),
            "\"/home/user/single'quote/cargo\"".to_string()
        );
    }
}
