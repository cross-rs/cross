mod toml;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use once_cell::sync::OnceCell;
use rustc_version::VersionMeta;

use crate::{docker::ImagePlatform, rustc::QualifiedToolchain, TargetTriple, ToUtf8};

static WORKSPACE: OnceCell<PathBuf> = OnceCell::new();

/// Returns the cargo workspace for the manifest
pub fn get_cargo_workspace() -> &'static Path {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mut msg_info = crate::shell::Verbosity::Verbose(2).into();
    #[allow(clippy::unwrap_used)]
    WORKSPACE.get_or_init(|| {
        crate::cargo_metadata_with_args(Some(manifest_dir.as_ref()), None, &mut msg_info)
            .unwrap()
            .unwrap()
            .workspace_root
    })
}

pub fn walk_dir<'a>(
    root: &'_ Path,
    skip: &'static [impl AsRef<OsStr> + Send + Sync + 'a],
    ext: impl for<'s> Fn(Option<&'s std::ffi::OsStr>) -> bool + Sync + Send + 'static,
) -> impl Iterator<Item = Result<ignore::DirEntry, ignore::Error>> {
    ignore::WalkBuilder::new(root)
        .filter_entry(move |e| {
            if skip
                .iter()
                .map(|s| -> &std::ffi::OsStr { s.as_ref() })
                .any(|dir| e.file_name() == dir)
            {
                return false;
            } else if e.file_type().map_or(false, |f| f.is_dir()) {
                return true;
            }
            ext(e.path().extension())
        })
        .build()
}

#[test]
pub fn target_mismatch() {
    use crate::{warn_host_version_mismatch, VersionMatch};

    fn make_meta(input: &str) -> VersionMeta {
        let mut split = input.split(' ');
        let version = split.next().unwrap();
        let hash_short = &split.next().unwrap().strip_prefix('(').unwrap();
        let date = split.next().unwrap().strip_suffix(')').unwrap();
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
        let mut msg_info = crate::shell::MessageInfo::default();
        assert_eq!(
            expected,
            warn_host_version_mismatch(
                &host_meta,
                &QualifiedToolchain::new(
                    "xxxx",
                    &None,
                    &ImagePlatform::from_const_target(TargetTriple::X86_64UnknownLinuxGnu),
                    Path::new("/toolchains/xxxx-x86_64-unknown-linux-gnu"),
                    false,
                ),
                &target_meta.0,
                &target_meta.1,
                &mut msg_info,
            )
            .unwrap(),
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

#[test]
fn check_newlines() -> crate::Result<()> {
    for file in walk_dir(get_cargo_workspace(), &[".git", "target"], |_| true) {
        let file = file?;
        if !file.file_type().map_or(true, |f| f.is_file()) {
            continue;
        }
        eprintln!("File: {:?}", file.path());
        assert!(
            crate::file::read(file.path())
                .unwrap_or_else(|_| String::from("\n"))
                .ends_with('\n'),
            "file {:?} does not end with a newline",
            file.path().to_utf8()?
        );
    }
    Ok(())
}
