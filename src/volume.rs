use std::env::home_dir;
use std::fs;
use std::path::Path;

use Target;
use Toml;
use docker;
use errors::*;
use extensions::CommandExt;
use id;

// TODO: replace with something like "japaric/cross-volume-manager"
static VOLUME_IMAGE: &'static str = "volmgr";
static INSTALL_XARGO: &'static str = r#"
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- \
        --git japaric/xargo \
        --tag v0.3.5 \
        --target x86_64-unknown-linux-gnu \
        --to /volwork/xargo
"#;

pub struct VolumeInfo {
    pub xargo_dir: String,
    pub rust_dir: String,
    pub cargo_dir: String,
}

pub fn populate_volume(
    target: &Target,
    args_toolchain: Option<String>,
    toml: Option<&Toml>,
    uses_xargo: bool,
    verbose: bool,
) -> Result<VolumeInfo> {
    // TODO: Take any direction from `args`?
    // Maybe allow the user to specify xargo/cargo/rust dirs to
    // avoid volume management with a specific (self compiled)
    // version of Rust?

    let base_path = working_path(&[])?;
    let base_mapping = format!("{}:/volwork", &base_path);
    let toolchain = match (args_toolchain, toml) {
        // `cross +nightly ...`
        (Some(tc), _) => tc,
        // `cross ...`
        (None, Some(toml)) => {
            toml.toolchain(target)?
                .ok_or_else(|| Error::from(format!("target.{}.toolchain not found in Cross.toml!", target.triple())))?
                .to_owned()
        },
        (None, None) => {
            bail!("No cross toolchain specified!")
        }
    };

    let rust_toolchain = format!("RUST_TOOLCHAIN={}", toolchain);

    // Avoid copy and paste
    let docker_cmd = || {
        let mut cmd = docker::docker_command("run");
        cmd.arg("--rm");
        cmd.args(&["--user", &format!("{}:{}", id::user(), id::group())]);
        cmd.args(&["-e", &format!("USER={}", id::username())]);
        cmd.args(&["-e", &rust_toolchain]);
        cmd.args(&["-v", &base_mapping]);
        cmd.args(&["-t", VOLUME_IMAGE]);
        cmd
    };

    let fqtc = format!("{}-x86_64-unknown-linux-gnu", toolchain);
    let target_path = working_path(&[".rustup", "toolchains", &fqtc])?;

    if !Path::new(&base_path).exists() {
        // If no cross directory exists, create a new one with the currently
        // requested toolchain
        println!("Initializing Cross workspace...");

        fs::create_dir_all(&base_path).ok();

        docker_cmd()
            .run_and_get_status(verbose)?;
    } else if !Path::new(&target_path).exists() {
        // Top level exists, but requested target does not
        println!("Installing toolchain {}...", fqtc);
        let cmd = format!("~/.cargo/bin/rustup toolchain install {}", fqtc);
        docker_cmd()
            .args(&["sh", "-c", &cmd])
            .run_and_get_status(verbose)?;
    }

    let xargo_dir = working_path(&["xargo"])?;
    let cargo_dir = working_path(&[".cargo"])?;

    let needs_xargo_install = uses_xargo && !Path::new(&xargo_dir).exists();

    if needs_xargo_install {
        println!("Installing Xargo...");
        fs::create_dir_all(&xargo_dir).ok();

        docker_cmd()
            .args(&["sh", "-c", INSTALL_XARGO])
            .run_and_get_status(verbose)?;
    } else {
        println!("Skipping Xargo install...");
    }

    let target_check_path = working_path(&[
        ".rustup",
        "toolchains",
        &fqtc,
        "lib",
        "rustlib",
        target.triple(),
    ])?;

    if !Path::new(&target_check_path).exists()
    {
        println!("Targeting {} {}", toolchain, target.triple());

        let cmd = format!(
            r#"
            # Place the correct binaries in PATH
            export PATH=/volwork/.rustup/toolchains/{toolchain}-x86_64-unknown-linux-gnu/bin:/volwork/.cargo/bin:$PATH && \

            # Make sure we are using the correct toolchain
            rustup default {toolchain}

            # Install the target if possible through rustup
            rustup target list | grep -wq {target}
            if [ $? -eq 0 ]; then
                echo "adding {target}"
                rustup target add {target};
            fi
            "#,
            toolchain = toolchain,
            target = target.triple()
        );

        docker_cmd()
            .args(&["sh", "-c", &cmd])
            .run_and_get_status(verbose)?;
    } else {
        println!("Skipping targeting");
    }

    Ok(VolumeInfo {
        xargo_dir: xargo_dir,
        cargo_dir: cargo_dir,
        rust_dir: target_path,
    })
}

fn working_path(items: &[&str]) -> Result<String> {
    let mut path_builder = home_dir().ok_or(Error::from("No Home directory"))?;
    path_builder.push(".cross");
    path_builder.push("volumes");

    for i in items {
        path_builder.push(Path::new(i));
    }

    Ok(format!("{}", path_builder.display()))
}
