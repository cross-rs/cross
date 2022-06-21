use std::{
    collections::BTreeMap,
    path::Path,
    process::{Command, Stdio},
};

use crate::util::{format_repo, pull_image};
use clap::Args;
use cross::CommandExt;
use serde::Deserialize;

use crate::util::ImageTarget;

// Store raw text data in the binary so we don't need a data directory
// when extracting all targets, or running our target info script.
const TARGET_INFO_SCRIPT: &str = include_str!("target_info.sh");

#[derive(Args, Debug)]
pub struct TargetInfo {
    /// If not provided, get info for all targets.
    targets: Vec<crate::ImageTarget>,
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    verbose: bool,
    /// Image registry.
    #[clap(long, default_value_t = String::from("ghcr.io"))]
    registry: String,
    /// Image repository.
    #[clap(long, default_value_t = String::from("cross-rs"))]
    repository: String,
    /// Image tag.
    #[clap(long, default_value_t = String::from("main"))]
    tag: String,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

fn image_info(
    engine: &Path,
    target: &crate::ImageTarget,
    image: &str,
    tag: &str,
    verbose: bool,
    has_test: bool,
) -> cross::Result<TargetInfoOutput> {
    if !tag.starts_with("local") {
        pull_image(engine, image, verbose)?;
    }

    let mut command = Command::new(engine);
    command.arg("run");
    command.arg("--rm");
    command.args(&["-e", &format!("TARGET={}", target.triplet)]);
    if has_test {
        command.args(&["-e", "HAS_TEST=1"]);
    } else {
        command.args(&["-e", "HAS_TEST="]);
    }
    command.arg(image);
    command.args(&["bash", "-c", TARGET_INFO_SCRIPT]);

    serde_json::from_str(&command.run_and_get_stdout(verbose)?).map_err(Into::into)
}

#[derive(Debug, Deserialize)]
struct TargetInfoOutput {
    pub libc: Version,
    pub cc: Version,
    pub cxx: Truthy,
    pub qemu: Version,
    pub has_test: Truthy,
    pub libc_is_newlib: Truthy,
    pub libc_os: Truthy,
    pub bionic: Truthy,
}

impl TargetInfoOutput {
    pub fn flags(&self) -> String {
        let mut string = String::new();

        if self.libc_is_newlib.is_yes() {
            string.push_str("[4]")
        }
        if self.libc_os.is_yes() {
            string.push_str("[3]")
        }
        if self.bionic.is_yes() {
            string.push_str("[1]")
        }

        if !string.is_empty() {
            string.insert(0, ' ');
        }
        string
    }
}

#[derive(Debug, Deserialize)]
#[serde(from = "&str")]
pub enum Version {
    NotApplicable,
    Version(String),
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Version::NotApplicable => f.pad("N/A"),
            Version::Version(v) => f.pad(v),
        }
    }
}

impl From<&str> for Version {
    fn from(version: &str) -> Self {
        match version {
            "" => Version::NotApplicable,
            v => Version::Version(v.to_string()),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(from = "&str")]
pub enum Truthy {
    Yes,
    No,
}

impl Truthy {
    /// Returns `true` if the truthy is [`Yes`].
    ///
    /// [`Yes`]: Truthy::Yes
    #[must_use]
    pub fn is_yes(&self) -> bool {
        matches!(self, Self::Yes)
    }
}

impl std::fmt::Display for Truthy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Truthy::Yes => f.pad("✓"),
            Truthy::No => f.pad(""),
        }
    }
}

impl From<&str> for Truthy {
    fn from(version: &str) -> Self {
        match version {
            "" => Truthy::No,
            _ => Truthy::Yes,
        }
    }
}

fn calc_width<'a>(
    iter: impl Iterator<Item = &'a cross::Result<(ImageTarget, TargetInfoOutput)>>,
    f: impl Fn(&'a ImageTarget, &'a TargetInfoOutput) -> usize,
) -> usize {
    iter.filter_map(|r| r.as_ref().ok())
        .map(|(target, info)| f(target, info))
        .max()
        .unwrap_or_default()
        + 2
}

pub fn target_info(
    TargetInfo {
        mut targets,
        verbose,
        registry,
        repository,
        tag,
        ..
    }: TargetInfo,
    engine: &Path,
) -> cross::Result<()> {
    let matrix = crate::util::get_matrix();
    let test_map: BTreeMap<crate::ImageTarget, bool> = matrix
        .iter()
        .map(|i| (i.to_image_target(), i.has_test(&i.target)))
        .collect();

    if targets.is_empty() {
        targets = matrix
            .iter()
            .map(|t| t.to_image_target())
            .filter(|t| t.has_ci_image())
            .collect();
    }

    let process = |target: &ImageTarget| {
        let image = target.image_name(&format_repo(&registry, &repository), &tag);
        let has_test = test_map
            .get(target)
            .cloned()
            .ok_or_else(|| eyre::eyre!("invalid target name {}", target))?;
        eprintln!("doing {target}");
        match image_info(engine, target, &image, &tag, verbose, has_test) {
            Ok(r) => Ok((target.clone(), r)),
            Err(e) => Err(eyre::eyre!("target {target} failed with: {e}")),
        }
    };

    let results: Vec<_>;
    #[cfg(feature = "rayon")]
    {
        use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
        results = targets.par_iter().map(process).collect();
    }

    #[cfg(not(feature = "rayon"))]
    {
        results = targets.into_iter().map(|ref t| process(t)).collect();
    }

    let t_w = calc_width(results.iter(), |t, info| {
        t.alt().len() + info.flags().len() + 2
    });
    let libc_w = calc_width(results.iter(), |_, info| info.libc.to_string().len() + 2);
    let cc_w = calc_width(results.iter(), |_, info| info.cc.to_string().len() + 2);
    let qemu_w = calc_width(results.iter(), |_, info| info.qemu.to_string().len() + 2);
    results.into_iter().filter_map(Result::ok).for_each(|(
        target,
        ref info @ TargetInfoOutput {
            ref libc,
            ref cc,
            ref cxx,
            ref qemu,
            ref has_test,
            ..
        },
    )| {
        println!(
            "|{target: >t_w$} | {libc: <libc_w$}| {cc: <cc_w$}|{cxx: ^5}| {qemu: <qemu_w$}|{has_test: ^5}|",
            target = format!("`{}`{}", target.alt(), info.flags())
        )
    });

    Ok(())
}
