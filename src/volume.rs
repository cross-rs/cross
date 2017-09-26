#![allow(unused_variables, dead_code, unused_imports, unused_must_use)]

use errors::*;
use Target;
use Toml;
use docker;
use extensions::CommandExt;
use std::path::Path;
use std::fs;
use id;

// TODO: replace with something like "japaric/cross-volume-manager"
static VOLUME_IMAGE: &'static str = "volmgr";
static WORKING_DIR: &'static str = "/tmp/solocross";

pub struct VolumeInfo {
    pub xargo_dir: String,
    pub rust_dir: String,
    pub cargo_dir: String,
}

pub fn populate_volume(
    target: &Target,
    args: &[String],
    toml: Option<&Toml>,
    uses_xargo: bool,
    verbose: bool) -> Result<VolumeInfo> {


    let toolchain = toml.expect("Cross.toml required (for now)...")
        .toolchain(target)
        .expect("Badly formed Cross.toml...")
        .unwrap_or("stable");

    // TODO: get toolchain from Toml
    // TODO: use path.join, otherwise wont work on Windows
    let rust_toolchain = format!("RUST_TOOLCHAIN={}", toolchain);
    let xargo_dir = format!("{}/xargo", WORKING_DIR);
    let cargo_dir = format!("{}/{}/cargo", WORKING_DIR, toolchain);
    let rust_dir = format!("{}/{}/rust", WORKING_DIR, toolchain);

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

        let cmd = "curl -LSfs http://japaric.github.io/trust/install.sh | sh -s -- --git japaric/xargo --tag v0.3.5 --target x86_64-unknown-linux-gnu --to /xargo";

        docker::docker_command("run")
            .arg("--rm")
            .args(&["--user", &format!("{}:{}", id::user(), id::group())])
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
    let cmd = format!(r#"
        export PATH=/rust/bin:/cargo/bin:$PATH && \
        mkdir -p ~/.rustup/toolchains && \
        ln -s /rust/settings.toml ~/.rustup/settings.toml && \
        ln -s /rust ~/.rustup/toolchains/{toolchain}-x86_64-unknown-linux-gnu;
        rustup target list | grep -wq {target}
        if [ $? -eq 0 ]; then rustup target add {target}; fi
        "#, toolchain=toolchain, target=target.triple());

    docker::docker_command("run")
        .arg("--rm")
        .args(&["--user", &format!("{}:{}", id::user(), id::group())])
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
