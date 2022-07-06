use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::Result;

// open temporary directories and files so we ensure we cleanup on exit.
static mut FILES: Vec<tempfile::NamedTempFile> = vec![];
static mut DIRS: Vec<tempfile::TempDir> = vec![];

fn data_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| d.data_dir().to_path_buf())
}

pub fn dir() -> Result<PathBuf> {
    data_dir()
        .map(|p| p.join("cross-rs").join("tmp"))
        .ok_or(eyre::eyre!("unable to get data directory"))
}

pub(crate) fn has_tempfiles() -> bool {
    // SAFETY: safe, since we only check if the stack is empty.
    unsafe { !FILES.is_empty() || !DIRS.is_empty() }
}

/// # Safety
/// Safe as long as we have single-threaded execution.
pub(crate) unsafe fn clean() {
    // don't expose FILES or DIRS outside this module,
    // so we can only add or remove from the stack using
    // our wrappers, guaranteeing add/remove in order.
    FILES.clear();
    DIRS.clear();
}

/// # Safety
/// Safe as long as we have single-threaded execution.
unsafe fn push_tempfile() -> Result<&'static mut tempfile::NamedTempFile> {
    let parent = dir()?;
    fs::create_dir_all(&parent).ok();
    let file = tempfile::NamedTempFile::new_in(&parent)?;
    FILES.push(file);
    Ok(FILES.last_mut().expect("file list should not be empty"))
}

/// # Safety
/// Safe as long as we have single-threaded execution.
unsafe fn pop_tempfile() -> Option<tempfile::NamedTempFile> {
    FILES.pop()
}

#[derive(Debug)]
pub struct TempFile {
    file: &'static mut tempfile::NamedTempFile,
}

impl TempFile {
    /// # Safety
    /// Safe as long as we have single-threaded execution.
    pub unsafe fn new() -> Result<Self> {
        Ok(Self {
            file: push_tempfile()?,
        })
    }

    pub fn file(&mut self) -> &mut tempfile::NamedTempFile {
        self.file
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.file.path()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        // SAFETY: safe if we only modify the stack via `TempFile`.
        unsafe {
            pop_tempfile();
        }
    }
}

/// # Safety
/// Safe as long as we have single-threaded execution.
unsafe fn push_tempdir() -> Result<&'static Path> {
    let parent = dir()?;
    fs::create_dir_all(&parent).ok();
    let dir = tempfile::TempDir::new_in(&parent)?;
    DIRS.push(dir);
    Ok(DIRS.last().expect("should not be empty").path())
}

/// # Safety
/// Safe as long as we have single-threaded execution.
unsafe fn pop_tempdir() -> Option<tempfile::TempDir> {
    DIRS.pop()
}

#[derive(Debug)]
pub struct TempDir {
    path: &'static Path,
}

impl TempDir {
    /// # Safety
    /// Safe as long as we have single-threaded execution.
    pub unsafe fn new() -> Result<Self> {
        Ok(Self {
            path: push_tempdir()?,
        })
    }

    #[must_use]
    pub fn path(&self) -> &'static Path {
        self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // SAFETY: safe if we only modify the stack via `TempDir`.
        unsafe {
            pop_tempdir();
        }
    }
}
