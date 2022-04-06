mod toml;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use once_cell::sync::OnceCell;
use serde::Deserialize;

static WORKSPACE: OnceCell<PathBuf> = OnceCell::new();

/// Returns the cargo workspace for the manifest
pub fn get_cargo_workspace() -> &'static Path {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    WORKSPACE.get_or_init(|| {
        #[derive(Deserialize)]
        struct Manifest {
            workspace_root: PathBuf,
        }
        let output = std::process::Command::new(
            std::env::var("CARGO")
                .ok()
                .unwrap_or_else(|| "cargo".to_string()),
        )
        .arg("metadata")
        .arg("--format-version=1")
        .arg("--no-deps")
        .current_dir(manifest_dir)
        .output()
        .unwrap();
        let manifest: Manifest = serde_json::from_slice(&output.stdout).unwrap();
        manifest.workspace_root
    })
}

pub fn walk_dir<'a>(
    root: &'_ Path,
    skip: &'a [impl AsRef<OsStr>],
) -> impl Iterator<Item = Result<walkdir::DirEntry, walkdir::Error>> + 'a {
    walkdir::WalkDir::new(root).into_iter().filter_entry(|e| {
        if skip
            .iter()
            .map(|s| -> &std::ffi::OsStr { s.as_ref() })
            .any(|dir| e.file_name() == dir)
        {
            return false;
        } else if e.file_type().is_dir() {
            return true;
        }
        e.path().extension() == Some("md".as_ref())
    })
}
