use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use crate::util::{project_dir, write_to_string};
use clap::Args;
use cross::shell::MessageInfo;
use cross::ToUtf8;

const DEFAULT_GCC_VERSION: &str = "8.3.0";
const DEFAULT_GLIBC_VERSION: &str = "2.17.0";
const DEFAULT_LINUX_VERSION: &str = "4.19.21";
const DOCKER: &str = "docker";
const CT_NG: &str = "crosstool-ng";
const TOOLCHAINS: &str = "cross-toolchains";
const CT_CONFIG: &str = "crosstool-config";

#[derive(Args, Debug)]
pub struct ConfigureCrosstool {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Coloring: auto, always, never
    #[clap(long)]
    pub color: Option<String>,
    /// The gcc version to configure for.
    #[clap(long, env = "GCC_VERSION")]
    pub gcc_version: Option<String>,
    /// The glibc version to configure for.
    #[clap(long, env = "GLIBC_VERSION")]
    pub glibc_version: Option<String>,
    /// The linux version to configure for.
    #[clap(long, env = "LINUX_VERSION")]
    pub linux_version: Option<String>,
    /// Targets to build for
    #[clap()]
    targets: Vec<String>,
}

fn locate_ctng_config(
    target: &str,
    root: &Path,
    cross_toolchains_root: &Path,
) -> cross::Result<(PathBuf, PathBuf)> {
    let config_name = format!("{target}.config");
    let template_name = format!("{config_name}.in");
    let ct_ng_root = root.join(CT_NG);
    let cross_toolchains_ctng_root = cross_toolchains_root.join(CT_NG);
    let (src_root, dst_root) = if cross_toolchains_ctng_root.join(&template_name).exists() {
        (
            &cross_toolchains_ctng_root,
            cross_toolchains_root.join(DOCKER).join(CT_CONFIG),
        )
    } else if ct_ng_root.join(&template_name).exists() {
        (&ct_ng_root, root.join(DOCKER).join(CT_CONFIG))
    } else {
        eyre::bail!("unable to find config for target \"{target}\"");
    };
    Ok((src_root.join(template_name), dst_root.join(config_name)))
}

fn read_config_dir(dir: &Path) -> cross::Result<Vec<String>> {
    let mut targets = vec![];
    for entry in fs::read_dir(dir)? {
        let file = entry?;
        let basename = file.file_name().to_utf8()?.to_string();
        if let Some(target) = basename.strip_suffix(".config.in") {
            targets.push(target.to_string());
        }
    }

    Ok(targets)
}

fn configure_target(
    src_file: &Path,
    gcc_version: &str,
    glibc_version: &str,
    linux_version: &str,
) -> cross::Result<String> {
    let gcc_versions: Vec<&str> = gcc_version.split('.').collect();
    let glibc_versions: Vec<&str> = glibc_version.split('.').collect();
    let linux_versions: Vec<&str> = linux_version.split('.').collect();
    if !matches!(gcc_versions.len(), 2 | 3) {
        eyre::bail!("invalid GCC version, got {gcc_version}");
    }
    if !matches!(glibc_versions.len(), 2 | 3) {
        eyre::bail!("invalid glibc version, got {glibc_version}");
    }
    if !matches!(linux_versions.len(), 2 | 3) {
        eyre::bail!("invalid linux version, got {linux_version}");
    }

    // configure the `CT_GCC` values
    let gcc_major = gcc_versions[0].parse::<u32>()?;
    let gcc_minor = gcc_versions[1].parse::<u32>()?;
    let gcc_patch = gcc_versions.get(2).unwrap_or(&"0").parse::<u32>()?;
    let ct_gcc_v = format!(
        "CT_GCC_V_{gcc_major}=y
# CT_GCC_NO_VERSIONS is not set
CT_GCC_VERSION=\"{gcc_major}.{gcc_minor}.{gcc_patch}\""
    );
    let mut ct_gcc = String::new();
    for major in (5..=7).rev() {
        if gcc_major > major {
            write!(ct_gcc, "\nCT_GCC_later_than_{major}=y")?;
        }
        if gcc_major >= major {
            write!(ct_gcc, "\nCT_GCC_{major}_or_later=y")?;
        }
    }
    if gcc_major > 4 || (gcc_major == 4 && gcc_major > 9) {
        ct_gcc.push_str("\nCT_GCC_later_than_4_9=y");
    }
    if gcc_major > 4 || (gcc_major == 4 && gcc_major >= 9) {
        ct_gcc.push_str("\nCT_GCC_4_9_or_later=y");
    }
    if gcc_major > 4 || (gcc_major == 4 && gcc_major > 8) {
        ct_gcc.push_str("\nCT_GCC_later_than_4_8=y");
    }
    if gcc_major > 4 || (gcc_major == 4 && gcc_major >= 8) {
        ct_gcc.push_str("\nCT_GCC_4_8_or_later=y");
    }

    // configure the `CT_GLIBC` values
    let glibc_major = glibc_versions[0].parse::<u32>()?;
    let glibc_minor = glibc_versions[1].parse::<u32>()?;
    let _glibc_patch = glibc_versions.get(2).unwrap_or(&"0").parse::<u32>()?;
    if glibc_major != 2 {
        eyre::bail!("glibc major versions other than 2 currently unsupported, got {glibc_major}");
    }
    let ct_glibc_v = format!(
        "CT_GLIBC_V_{glibc_major}_{glibc_minor}=y
# CT_GLIBC_NO_VERSIONS is not set
CT_GLIBC_VERSION=\"{glibc_major}.{glibc_minor}\""
    );
    let mut ct_glibc = String::new();
    let glibc_older = [29, 27, 26, 25, 24, 23, 20];
    for minor in glibc_older {
        if glibc_minor <= minor {
            write!(ct_glibc, "\nCT_GLIBC_2_{minor}_or_older=y")?;
        }
        if glibc_minor < minor {
            write!(ct_glibc, "\nCT_GLIBC_older_than_2_{minor}=y")?;
        }
    }
    if glibc_minor >= 17 {
        ct_glibc.push_str("\nCT_GLIBC_2_17_or_later=y");
    }
    if glibc_minor <= 17 {
        ct_glibc.push_str("\nCT_GLIBC_2_17_or_older=y");
    }
    if glibc_minor > 14 {
        ct_glibc.push_str("\nCT_GLIBC_later_than_2_14=y");
    }
    if glibc_minor >= 14 {
        ct_glibc.push_str("\nCT_GLIBC_2_14_or_later=y");
    }

    // configure the `CT_LINUX` values
    let linux_major = linux_versions[0].parse::<u32>()?;
    let linux_minor = linux_versions[1].parse::<u32>()?;
    let linux_patch = linux_versions.get(2).unwrap_or(&"0").parse::<u32>()?;
    let ct_linux_v = format!(
        "CT_LINUX_V_{linux_major}_{linux_minor}=y
# CT_LINUX_NO_VERSIONS is not set
CT_LINUX_VERSION=\"{linux_major}.{linux_minor}.{linux_patch}\""
    );
    let mut ct_linux = String::new();
    if linux_major < 4 || (linux_major == 4 && linux_minor < 8) {
        ct_linux.push_str("\nCT_LINUX_older_than_4_8=y");
        ct_linux.push_str("\nCT_LINUX_4_8_or_older=y");
    } else {
        ct_linux.push_str("\nCT_LINUX_later_than_4_8=y");
        ct_linux.push_str("\nCT_LINUX_4_8_or_later=y");
    }
    if linux_major < 3 || (linux_major == 3 && linux_minor < 7) {
        ct_linux.push_str("\nCT_LINUX_older_than_3_7=y");
        ct_linux.push_str("\nCT_LINUX_3_7_or_older=y");
        ct_linux.push_str("\nCT_LINUX_older_than_3_2=y");
        ct_linux.push_str("\nCT_LINUX_3_2_or_older=y");
    } else {
        ct_linux.push_str("\nCT_LINUX_later_than_3_7=y");
        ct_linux.push_str("\nCT_LINUX_3_7_or_later=y");
        ct_linux.push_str("\nCT_LINUX_later_than_3_2=y");
        ct_linux.push_str("\nCT_LINUX_3_2_or_later=y");
    }

    Ok(fs::read_to_string(src_file)?
        .replacen("%CT_GCC_V%", &ct_gcc_v, 1)
        .replacen("%CT_GCC%", &ct_gcc, 1)
        .replacen("%CT_GLIBC_V%", &ct_glibc_v, 1)
        .replacen("%CT_GLIBC%", &ct_glibc, 1)
        .replacen("%CT_LINUX_V%", &ct_linux_v, 1)
        .replacen("%CT_LINUX%", &ct_linux, 1))
}

pub fn configure_crosstool(
    ConfigureCrosstool {
        gcc_version,
        glibc_version,
        linux_version,
        mut targets,
        ..
    }: ConfigureCrosstool,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let gcc_version = gcc_version.as_deref().unwrap_or(DEFAULT_GCC_VERSION);
    let glibc_version = glibc_version.as_deref().unwrap_or(DEFAULT_GLIBC_VERSION);
    let linux_version = linux_version.as_deref().unwrap_or(DEFAULT_LINUX_VERSION);

    let root = project_dir(msg_info)?;
    let cross_toolchains_root = root.join(DOCKER).join(TOOLCHAINS);
    if targets.is_empty() {
        targets = read_config_dir(&root.join(CT_NG))?;
        let cross_toolchains_ctng_root = cross_toolchains_root.join(CT_NG);
        if cross_toolchains_ctng_root.exists() {
            targets.append(&mut read_config_dir(&cross_toolchains_ctng_root)?);
        }
    }
    let config_files = targets
        .into_iter()
        .map(|t| locate_ctng_config(&t, &root, &cross_toolchains_root))
        .collect::<cross::Result<Vec<_>>>()?;
    for (src_file, dst_file) in config_files {
        let configured = configure_target(&src_file, gcc_version, glibc_version, linux_version)?;
        write_to_string(&dst_file, &configured)?;
    }

    Ok(())
}
