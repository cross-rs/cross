use std::path::{Path, PathBuf};
use std::{fs, mem};

use crate::util;
use clap::Args;
use cross::shell::MessageInfo;
use cross::ToUtf8;

/// Create and print a temporary directory to stdout.
#[derive(Args, Debug)]
pub struct MakeTempDir {
    /// `tmp` to create the temporary directory inside.
    /// Defaults to `"${target_dir}/tmp"`.
    tmpdir: Option<PathBuf>,
}

pub fn make_temp_dir(MakeTempDir { tmpdir }: MakeTempDir) -> cross::Result<()> {
    let mut msg_info = MessageInfo::create(0, false, None)?;
    let dir = temp_dir(tmpdir.as_deref(), &mut msg_info)?;
    msg_info.print(dir.to_utf8()?)
}

pub fn temp_dir(parent: Option<&Path>, msg_info: &mut MessageInfo) -> cross::Result<PathBuf> {
    let parent = match parent {
        Some(parent) => parent.to_owned(),
        None => util::project_dir(msg_info)?.join("target").join("tmp"),
    };
    fs::create_dir_all(&parent)?;
    let dir = tempfile::TempDir::new_in(&parent)?;
    let path = dir.path().to_owned();
    mem::drop(dir);

    fs::create_dir(&path)?;
    Ok(path)
}
