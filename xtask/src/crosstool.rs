use std::cmp::Ordering;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use crate::util::{project_dir, write_to_string};
use clap::Args;
use cross::shell::MessageInfo;
use cross::ToUtf8;

const DEFAULT_GCC_VERSION: &str = "8.3.0";
const DEFAULT_GLIBC_VERSION: &str = "2.17.0";
const DEFAULT_UCLIBC_VERSION: &str = "1.0.31";
const DEFAULT_MUSL_VERSION: &str = "1.1.24";
const DEFAULT_NEWLIB_VERSION: &str = "3.1.0.20181231";
const DEFAULT_LINUX_VERSION: &str = "4.19.21";
const DOCKER: &str = "docker";
const CT_NG: &str = "crosstool-ng";
const TOOLCHAINS: &str = "cross-toolchains";
const CT_CONFIG: &str = "crosstool-config";

#[derive(Args, Debug)]
pub struct ConfigureCrosstool {
    /// The gcc version to configure for.
    #[clap(long, env = "GCC_VERSION")]
    pub gcc_version: Option<String>,
    /// The glibc version to configure for.
    #[clap(long, env = "GLIBC_VERSION")]
    pub glibc_version: Option<String>,
    /// The uclibc version to configure for.
    #[clap(long, env = "UCLIBC_VERSION")]
    pub uclibc_version: Option<String>,
    /// The musl version to configure for.
    #[clap(long, env = "MUSL_VERSION")]
    pub musl_version: Option<String>,
    /// The newlib version to configure for.
    #[clap(long, env = "NEWLIB_VERSION")]
    pub newlib_version: Option<String>,
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

fn configure_glibc(
    glibc_version: &str,
) -> cross::Result<(&'static str, String, String, Option<String>)> {
    // configure the `CT_GLIBC` values
    let glibc_versions: Vec<&str> = glibc_version.split('.').collect();
    if !matches!(glibc_versions.len(), 2 | 3) {
        eyre::bail!("invalid glibc version, got {glibc_version}");
    }

    let glibc_major = glibc_versions[0].parse::<u32>()?;
    let glibc_minor = glibc_versions[1].parse::<u32>()?;
    let _glibc_patch = glibc_versions.get(2).unwrap_or(&"0").parse::<u32>()?;
    if glibc_major != 2 {
        eyre::bail!("glibc major versions other than 2 currently unsupported, got {glibc_major}");
    }
    let ct_glibc_v = format!(
        r#"CT_GLIBC_V_{glibc_major}_{glibc_minor}=y
# CT_GLIBC_NO_VERSIONS is not set
CT_GLIBC_VERSION="{glibc_major}.{glibc_minor}""#
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

    Ok(("GLIBC", ct_glibc_v, ct_glibc, None))
}

fn configure_uclibc(
    uclibc_version: &str,
) -> cross::Result<(&'static str, String, String, Option<String>)> {
    // configure the `CT_UCLIBC` values
    let uclibc_versions: Vec<&str> = uclibc_version.split('.').collect();
    if !matches!(uclibc_versions.len(), 3 | 4) {
        eyre::bail!("invalid uclibc version, got {uclibc_version}");
    }

    let uclibc_major = uclibc_versions[0].parse::<u32>()?;
    let uclibc_minor = uclibc_versions[1].parse::<u32>()?;
    let uclibc_patch = uclibc_versions[2].parse::<u32>()?;
    let uclibc_dev = uclibc_versions.get(3).unwrap_or(&"0").parse::<u32>()?;
    let (key, version) = match uclibc_dev {
        0 => (
            format!("{uclibc_major}_{uclibc_minor}_{uclibc_patch}"),
            format!("{uclibc_major}.{uclibc_minor}.{uclibc_patch}"),
        ),
        _ => (
            format!("{uclibc_major}_{uclibc_minor}_{uclibc_patch}_{uclibc_dev}"),
            format!("{uclibc_major}.{uclibc_minor}.{uclibc_patch}.{uclibc_dev}"),
        ),
    };
    let (ct_uclibc_v, ct_extras) = match uclibc_major {
        0 => (
            format!(
                r#"CT_UCLIBC_V_{key}=y
# CT_UCLIBC_NO_VERSIONS is not set
CT_UCLIBC_VERSION="{version}""#
            ),
            r#"CT_UCLIBC_USE_UCLIBC_ORG=y
CT_UCLIBC_USE="UCLIBC"
CT_UCLIBC_PKG_NAME="uClibc"
CT_UCLIBC_SRC_RELEASE=y
CT_UCLIBC_PATCH_ORDER="global"
CT_UCLIBC_MIRRORS="https://uclibc.org/downloads/"
CT_UCLIBC_ARCHIVE_FILENAME="@{pkg_name}-@{version}"
CT_UCLIBC_ARCHIVE_DIRNAME="@{pkg_name}-@{version}"
CT_UCLIBC_ARCHIVE_FORMATS=".tar.xz .tar.bz2"
CT_UCLIBC_SIGNATURE_FORMAT="packed/.asc""#,
        ),
        _ => (
            format!(
                r#"CT_UCLIBC_NG_V_{key}=y
# CT_UCLIBC_NG_NO_VERSIONS is not set
CT_UCLIBC_NG_VERSION="{version}""#
            ),
            r#"CT_UCLIBC_USE_UCLIBC_NG_ORG=y
CT_UCLIBC_USE="UCLIBC_NG"
CT_UCLIBC_NG_PKG_NAME="uClibc-ng"
CT_UCLIBC_NG_SRC_RELEASE=y
CT_UCLIBC_NG_PATCH_ORDER="global"
CT_UCLIBC_NG_MIRRORS="http://downloads.uclibc-ng.org/releases/${CT_UCLIBC_NG_VERSION}"
CT_UCLIBC_NG_ARCHIVE_FILENAME="@{pkg_name}-@{version}"
CT_UCLIBC_NG_ARCHIVE_DIRNAME="@{pkg_name}-@{version}"
CT_UCLIBC_NG_ARCHIVE_FORMATS=".tar.xz .tar.lz .tar.bz2 .tar.gz"
CT_UCLIBC_NG_SIGNATURE_FORMAT="packed/.asc""#,
        ),
    };

    let mut ct_uclibc = String::new();
    let version = (uclibc_major, uclibc_minor, uclibc_patch, uclibc_dev);
    let uclibc_older = [
        (1, 0, 23, 0),
        (1, 0, 21, 0),
        (1, 0, 15, 0),
        (1, 0, 0, 0),
        (0, 9, 33, 2),
    ];
    for older in uclibc_older {
        let cmp = older.cmp(&version);
        let (major, minor, patch, dev) = older;
        let key = match dev {
            0 => format!("{major}_{minor}_{patch}"),
            _ => format!("{major}_{minor}_{patch}_{dev}"),
        };
        match cmp {
            Ordering::Greater => {
                write!(ct_uclibc, "\nCT_UCLIBC_{key}_or_older=y")?;
                write!(ct_uclibc, "\nCT_UCLIBC_older_than_{key}=y")?;
            }
            Ordering::Equal => {
                write!(ct_uclibc, "\nCT_UCLIBC_{key}_or_later=y")?;
                write!(ct_uclibc, "\nCT_UCLIBC_{key}_or_older=y")?;
            }
            Ordering::Less => {
                write!(ct_uclibc, "\nCT_UCLIBC_{key}_or_later=y")?;
                write!(ct_uclibc, "\nCT_UCLIBC_later_than_{key}=y")?;
            }
        }
    }

    Ok(("UCLIBC", ct_uclibc_v, ct_uclibc, Some(ct_extras.to_owned())))
}

fn configure_musl(
    musl_version: &str,
) -> cross::Result<(&'static str, String, String, Option<String>)> {
    let musl_versions: Vec<&str> = musl_version.split('.').collect();
    if !matches!(musl_versions.len(), 3) {
        eyre::bail!("invalid musl version, got {musl_version}");
    }

    let musl_major = musl_versions[0].parse::<u32>()?;
    let musl_minor = musl_versions[1].parse::<u32>()?;
    let musl_patch = musl_versions[2].parse::<u32>()?;
    let ct_musl_v = format!(
        r#"CT_MUSL_V_{musl_major}_{musl_minor}_{musl_patch}=y
# CT_MUSL_NO_VERSIONS is not set
CT_MUSL_VERSION="{musl_major}.{musl_minor}.{musl_patch}""#
    );

    Ok(("MUSL", ct_musl_v, "".to_owned(), None))
}

fn configure_newlib(
    newlib_version: &str,
) -> cross::Result<(&'static str, String, String, Option<String>)> {
    let newlib_versions: Vec<&str> = newlib_version.split('.').collect();
    if !matches!(newlib_versions.len(), 3 | 4) {
        eyre::bail!("invalid newlib version, got {newlib_version}");
    }

    let newlib_major = newlib_versions[0].parse::<u32>()?;
    let newlib_minor = newlib_versions[1].parse::<u32>()?;
    let newlib_patch = newlib_versions[2].parse::<u32>()?;
    let newlib_dev = newlib_versions.get(3).unwrap_or(&"0").parse::<u32>()?;
    let ct_newlib_v = format!(
        r#"CT_NEWLIB_V_{newlib_major}_{newlib_minor}=y
# CT_NEWLIB_NO_VERSIONS is not set
CT_NEWLIB_VERSION="{newlib_major}.{newlib_minor}.{newlib_patch}.{newlib_dev}""#
    );

    let mut ct_newlib = String::new();
    let version = (newlib_major, newlib_minor);
    let newlib_older = [(2, 2), (2, 1), (2, 0)];
    for older in newlib_older {
        let cmp = older.cmp(&version);
        let (major, minor) = older;
        let key = format!("{major}_{minor}");
        match cmp {
            Ordering::Greater => {
                write!(ct_newlib, "\nCT_NEWLIB_{key}_or_older=y")?;
                write!(ct_newlib, "\nCT_NEWLIB_older_than_{key}=y")?;
            }
            Ordering::Equal => {
                write!(ct_newlib, "\nCT_NEWLIB_{key}_or_later=y")?;
                write!(ct_newlib, "\nCT_NEWLIB_{key}_or_older=y")?;
            }
            Ordering::Less => {
                write!(ct_newlib, "\nCT_NEWLIB_{key}_or_later=y")?;
                write!(ct_newlib, "\nCT_NEWLIB_later_than_{key}=y")?;
            }
        }
    }

    Ok(("NEWLIB", ct_newlib_v, ct_newlib, None))
}

fn configure_target(
    src_file: &Path,
    gcc_version: &str,
    glibc_version: &str,
    uclibc_version: &str,
    musl_version: &str,
    newlib_version: &str,
    linux_version: &str,
) -> cross::Result<String> {
    let file_name = src_file
        .file_name()
        .ok_or(eyre::eyre!("unable to get filename for {src_file:?}"))?
        .to_utf8()?;
    let mut contents = fs::read_to_string(src_file)?;

    // configure the `CT_GCC` values
    let gcc_versions: Vec<&str> = gcc_version.split('.').collect();
    if !matches!(gcc_versions.len(), 2 | 3) {
        eyre::bail!("invalid GCC version, got {gcc_version}");
    }
    let gcc_major = gcc_versions[0].parse::<u32>()?;
    let gcc_minor = gcc_versions[1].parse::<u32>()?;
    let gcc_patch = gcc_versions.get(2).unwrap_or(&"0").parse::<u32>()?;
    let ct_gcc_v = format!(
        r#"CT_GCC_V_{gcc_major}=y
# CT_GCC_NO_VERSIONS is not set
CT_GCC_VERSION="{gcc_major}.{gcc_minor}.{gcc_patch}""#
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
    if ct_gcc.starts_with('\n') {
        ct_gcc.remove(0);
    }
    contents = contents
        .replacen("%CT_GCC_V%", &ct_gcc_v, 1)
        .replacen("%CT_GCC%", &ct_gcc, 1);

    // configure the libc versions
    let (key, libc_v, mut libc, extras) = if file_name.contains("gnu") {
        configure_glibc(glibc_version)?
    } else if file_name.contains("uclibc") {
        configure_uclibc(uclibc_version)?
    } else if file_name.contains("musl") {
        configure_musl(musl_version)?
    } else if file_name.contains("none") {
        configure_newlib(newlib_version)?
    } else {
        eyre::bail!("unsupported rust target for file {file_name}: unknown libc version");
    };
    if libc.starts_with('\n') {
        libc.remove(0);
    }
    contents = contents
        .replacen(&format!("%CT_{key}_V%"), &libc_v, 1)
        .replacen(&format!("%CT_{key}%"), &libc, 1);
    if let Some(extras) = extras {
        contents = contents.replacen(&format!("%CT_{key}_EXTRAS%"), &extras, 1);
    }

    // configure the `CT_LINUX` values
    if file_name.contains("linux") {
        let linux_versions: Vec<&str> = linux_version.split('.').collect();
        if !matches!(linux_versions.len(), 2 | 3) {
            eyre::bail!("invalid linux version, got {linux_version}");
        }
        let linux_major = linux_versions[0].parse::<u32>()?;
        let linux_minor = linux_versions[1].parse::<u32>()?;
        let linux_patch = linux_versions.get(2).unwrap_or(&"0").parse::<u32>()?;
        let ct_linux_v = format!(
            r#"CT_LINUX_V_{linux_major}_{linux_minor}=y
# CT_LINUX_NO_VERSIONS is not set
CT_LINUX_VERSION="{linux_major}.{linux_minor}.{linux_patch}""#
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
        if ct_linux.starts_with('\n') {
            ct_linux.remove(0);
        }

        contents =
            contents
                .replacen("%CT_LINUX_V%", &ct_linux_v, 1)
                .replacen("%CT_LINUX%", &ct_linux, 1);
    }

    Ok(contents)
}

pub fn configure_crosstool(
    ConfigureCrosstool {
        gcc_version,
        glibc_version,
        uclibc_version,
        musl_version,
        newlib_version,
        linux_version,
        mut targets,
        ..
    }: ConfigureCrosstool,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let gcc_version = gcc_version.as_deref().unwrap_or(DEFAULT_GCC_VERSION);
    let glibc_version = glibc_version.as_deref().unwrap_or(DEFAULT_GLIBC_VERSION);
    let uclibc_version = uclibc_version.as_deref().unwrap_or(DEFAULT_UCLIBC_VERSION);
    let musl_version = musl_version.as_deref().unwrap_or(DEFAULT_MUSL_VERSION);
    let newlib_version = newlib_version.as_deref().unwrap_or(DEFAULT_NEWLIB_VERSION);
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
        let configured = configure_target(
            &src_file,
            gcc_version,
            glibc_version,
            uclibc_version,
            musl_version,
            newlib_version,
            linux_version,
        )?;
        write_to_string(&dst_file, &configured)?;
    }

    Ok(())
}
