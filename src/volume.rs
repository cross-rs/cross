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

pub struct VolumeInfo {
    pub xargo_dir: String,
    pub rust_dir: String,
    pub cargo_dir: String,
}

pub fn populate_volume(
    target: &Target,
    _args: &[String],
    toml: Option<&Toml>,
    uses_xargo: bool,
    verbose: bool) -> Result<VolumeInfo> {

    // TODO: Take any direction from `args`?
    // Maybe allow the user to specify xargo/cargo/rust dirs to
    // avoid volume management with a specific (self compiled)
    // version of Rust?

    let toolchain = match toml {
        Some(t) => t.toolchain(target)?.unwrap_or("stable"),
        None => "stable"
    };

    let rust_toolchain = format!("RUST_TOOLCHAIN={}", toolchain);
    let xargo_dir = working_path(&["xargo"])?;
    let cargo_dir = working_path(&[&toolchain, "cargo"])?;
    let rust_dir = working_path(&[&toolchain, "rust"])?;

    let cargo_mapping = format!("{}:/cargo", &cargo_dir);
    let rust_mapping = format!("{}:/rust", &rust_dir);
    let xargo_mapping = format!("{}:/xargo", &xargo_dir);

    if !(Path::new(&cargo_dir).exists() && Path::new(&rust_dir).exists()) {
        // create the directories we are going to mount before we mount them,
        // otherwise `docker` will create them but they will be owned by `root`
        fs::create_dir_all(&cargo_dir).ok();
        fs::create_dir_all(&rust_dir).ok();

        println!("Installing toolchain...");

        docker::docker_command("run")
            .arg("--rm")
            .args(&["--user", &format!("{}:{}", id::user(), id::group())])
            .args(&["-e", &format!("USER={}", id::username())])
            .args(&["-e", &rust_toolchain])
            .args(&["-v", &cargo_mapping])
            .args(&["-v", &rust_mapping])
            .args(&["-t", VOLUME_IMAGE])
            .run_and_get_status(verbose)?;
    } else {
        println!("Skipping toolchain install...");
    }

    let needs_xargo_install = uses_xargo && !Path::new(&xargo_dir).exists();

    if needs_xargo_install {
        fs::create_dir_all(&xargo_dir).ok();

        let cmd = r#"
            curl -LSfs http://japaric.github.io/trust/install.sh | \
            sh -s -- \
                --git japaric/xargo \
                --tag v0.3.5 \
                --target x86_64-unknown-linux-gnu \
                --to /xargo
        "#;

        docker::docker_command("run")
            .arg("--rm")
            .args(&["--user", &format!("{}:{}", id::user(), id::group())])
            .args(&["-e", &format!("USER={}", id::username())])
            .args(&["-v", &xargo_mapping])
            .args(&["-t", VOLUME_IMAGE])
            .args(&["sh", "-c", cmd])
            .run_and_get_status(verbose)?;
    } else {
        println!("Skipping Xargo install...");
    }

    println!("Targeting");

    // Run target install, rustup will be polite and do nothing if necessary
    // If the target isn't available, skip it
    //
    // We have to fake a bit of the Rustup environment to get it to play nicely
    let cmd = format!(r#"
        export PATH=/rust/bin:/cargo/bin:$PATH && \
        mkdir -p ~/.rustup/toolchains && \
        ln -s /rust/settings.toml ~/.rustup/settings.toml && \
        ln -s /rust ~/.rustup/toolchains/{toolchain}-x86_64-unknown-linux-gnu;
        rustup target list | grep -wq {target}
        if [ $? -eq 0 ]; then
            rustup target add {target};
        fi
        "#, toolchain=toolchain, target=target.triple());

    docker::docker_command("run")
        .arg("--rm")
        .args(&["--user", &format!("{}:{}", id::user(), id::group())])
        .args(&["-e", &format!("USER={}", id::username())])
        .args(&["-v", &rust_mapping])
        .args(&["-v", &cargo_mapping])
        .args(&["-t", VOLUME_IMAGE])
        .args(&["sh", "-c", &cmd])
        .run_and_get_status(verbose)?;

    Ok(VolumeInfo {
        xargo_dir: xargo_dir,
        cargo_dir: cargo_dir,
        rust_dir: rust_dir,
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
