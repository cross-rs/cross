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

pub fn write_file(path: impl AsRef<Path>, overwrite: bool) -> Result<File> {
    let path = path.as_ref();
    fs::create_dir_all(
        &path.parent().ok_or_else(|| {
            eyre::eyre!("could not find parent directory for `{}`", path.display())
        })?,
    )
    .wrap_err_with(|| format!("couldn't create directory `{}`", path.display()))?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(!overwrite)
        .open(&path)
        .wrap_err(format!("could't write to file `{}`", path.display()))
}
