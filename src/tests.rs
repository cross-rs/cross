mod toml;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use once_cell::sync::OnceCell;
use rustc_version::VersionMeta;

static WORKSPACE: OnceCell<PathBuf> = OnceCell::new();

/// Returns the cargo workspace for the manifest
pub fn get_cargo_workspace() -> &'static Path {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    WORKSPACE.get_or_init(|| {
        crate::cargo::cargo_metadata_with_args(Some(manifest_dir.as_ref()), None, true)
            .unwrap()
            .unwrap()
            .workspace_root
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

#[test]
pub fn target_mismatch() {
    use crate::{warn_host_version_mismatch, VersionMatch};

    fn make_meta(input: &str) -> VersionMeta {
        let mut split = input.split(' ');
        let version = split.next().unwrap();
        let hash_short = &split.next().unwrap().strip_prefix('(').unwrap();
        let date = split.next().unwrap().strip_suffix(")").unwrap();
        rustc_version::version_meta_for(&format!(
            r#"rustc {version} ({hash_short} {date})
binary: rustc
commit-hash: {hash_short:f<40}
commit-date: {date}
host: xxxx
release: {version}
"#
        ))
        .unwrap()
    }

    fn make_rustc_version(input: &str) -> (rustc_version::Version, String) {
        let (version, meta) = input.split_once(' ').unwrap();
        (version.parse().unwrap(), meta.to_owned())
    }

    #[track_caller]
    fn compare(expected: VersionMatch, host: &str, targ: &str) {
        let host_meta = dbg!(make_meta(host));
        let target_meta = dbg!(make_rustc_version(targ));
        assert_eq!(
            expected,
            warn_host_version_mismatch(&host_meta, "xxxx", &target_meta.0, &target_meta.1).unwrap(),
            "\nhost = {}\ntarg = {}",
            host,
            targ
        );
    }

    compare(
        VersionMatch::Same,
        "1.0.0 (11111111 2022-01-01)",
        "1.0.0 (11111111 2022-01-01)",
    );
    compare(
        VersionMatch::Same,
        "1.0.0-nightly (11111111 2022-01-01)",
        "1.0.0-nightly (11111111 2022-01-01)",
    );
    compare(
        VersionMatch::OlderTarget,
        "1.2.0 (22222222 2022-02-02)",
        "1.0.0 (11111111 2022-01-01)",
    );
    compare(
        VersionMatch::NewerTarget,
        "1.0.0 (11111111 2022-01-01)",
        "1.2.0 (22222222 2022-02-02)",
    );
    compare(
        VersionMatch::Different,
        "1.0.0-nightly (11111111 2022-01-01)",
        "1.0.0-nightly (22222222 2022-01-01)",
    );
    compare(
        VersionMatch::OlderTarget,
        "1.0.0-nightly (22222222 2022-02-02)",
        "1.0.0-nightly (11111111 2022-01-01)",
    );
    compare(
        VersionMatch::NewerTarget,
        "1.0.0-nightly (11111111 2022-01-01)",
        "1.0.0-nightly (22222222 2022-02-02)",
    );
}
