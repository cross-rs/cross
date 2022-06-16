use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::errors::*;

pub fn read<P>(path: P) -> Result<String>
where
    P: AsRef<Path>,
{
    read_(path.as_ref())
}

fn read_(path: &Path) -> Result<String> {
    let mut s = String::new();
    File::open(path)
        .wrap_err_with(|| format!("couldn't open {}", path.display()))?
        .read_to_string(&mut s)
        .wrap_err_with(|| format!("couldn't read {}", path.display()))?;
    Ok(s)
}

pub fn canonicalize(path: impl AsRef<Path>) -> Result<PathBuf> {
    _canonicalize(path.as_ref())
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
        &path.parent().ok_or_else(|| {
            eyre::eyre!("could not find parent directory for `{}`", path.display())
        })?,
    )
    .wrap_err_with(|| format!("couldn't create directory `{}`", path.display()))?;

    let mut open = fs::OpenOptions::new();
    open.write(true);

    if overwrite {
        open.truncate(true).create(true);
    } else {
        open.create_new(true);
    }

    open.open(&path)
        .wrap_err(format!("couldn't write to file `{}`", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
