use std::fmt::Write;
use std::path::Path;

use crate::util::{
    cargo_metadata, get_matrix, gha_error, gha_output, gha_print, DEFAULT_PLATFORMS,
};
use crate::ImageTarget;
use clap::Args;
use cross::docker::{self, BuildCommandExt, BuildResultExt, ImagePlatform, Progress};
use cross::shell::MessageInfo;
use cross::{CommandExt, ToUtf8};

#[derive(Args, Debug)]
pub struct BuildDockerImage {
    #[clap(long, hide = true, env = "GITHUB_REF_TYPE")]
    pub ref_type: Option<String>,
    #[clap(long, hide = true, env = "GITHUB_REF_NAME")]
    ref_name: Option<String>,
    /// Pass extra flags to the build
    #[clap(long, env = "CROSS_BUILD_OPTS")]
    build_opts: Option<String>,
    #[clap(action, long = "latest", hide = true, env = "LATEST")]
    is_latest: bool,
    /// Specify a tag to use instead of the derived one, eg `local`
    #[clap(long)]
    pub tag: Option<String>,
    /// Repository name for image.
    #[clap(long, default_value = docker::CROSS_IMAGE)]
    pub repository: String,
    /// Newline separated labels
    #[clap(long, env = "LABELS")]
    /// Print but do not execute the build commands.
    pub labels: Option<String>,
    #[clap(long)]
    pub dry_run: bool,
    /// Force a push when `--push` is set, but not `--tag`
    #[clap(long, hide = true)]
    pub force: bool,
    /// Push build to registry.
    #[clap(short, long)]
    pub push: bool,
    /// Set output to /dev/null
    #[clap(short, long)]
    pub no_output: bool,
    /// Docker build progress output type.
    #[clap(
        long,
        value_parser = clap::builder::PossibleValuesParser::new(["auto", "plain", "tty"]),
    )]
    pub progress: Option<String>,
    /// Do not load from cache when building the image.
    #[clap(long)]
    pub no_cache: bool,
    /// Option `--cache-to` for docker, only would work if push is not set to true
    ///
    /// Additionally you can use {base_name} to replace with base name of the image
    /// If not specified, would not be passed to docker unless `--push` is used
    #[clap(long)]
    pub cache_to: Option<String>,
    /// Option `--cache-from` for docker, would only work if engine supports cache from type and no_cache is not set to true
    ///
    /// Additionally you can use {base_name} to replace with base name of the image
    #[clap(long, default_value = "type=registry,ref={base_name}:main")]
    pub cache_from: String,
    /// Continue building images even if an image fails to build.
    #[clap(long)]
    pub no_fastfail: bool,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// If no target list is provided, parse list from CI.
    #[clap(long)]
    pub from_ci: bool,
    /// Additional build arguments to pass to Docker.
    #[clap(long)]
    pub build_arg: Vec<String>,
    // [os/arch[/variant]=]toolchain
    #[clap(long, short = 'a', action = clap::builder::ArgAction::Append)]
    pub platform: Vec<ImagePlatform>,
    /// Targets to build for
    #[clap()]
    pub targets: Vec<ImageTarget>,
}

fn locate_dockerfile(
    target: ImageTarget,
    docker_root: &Path,
    cross_toolchain_root: &Path,
) -> cross::Result<(ImageTarget, String)> {
    let dockerfile_name = format!("Dockerfile.{target}");
    let dockerfile_root = if cross_toolchain_root.join(&dockerfile_name).exists() {
        &cross_toolchain_root
    } else if docker_root.join(&dockerfile_name).exists() {
        &docker_root
    } else {
        eyre::bail!("unable to find dockerfile for target \"{target}\"");
    };
    let dockerfile = dockerfile_root.join(dockerfile_name).to_utf8()?.to_string();
    Ok((target, dockerfile))
}

pub fn build_docker_image(
    BuildDockerImage {
        ref_type,
        ref_name,
        build_opts,
        is_latest,
        tag: tag_override,
        repository,
        labels,
        dry_run,
        force,
        push,
        no_output,
        progress,
        no_cache,
        no_fastfail,
        from_ci,
        build_arg,
        platform,
        cache_from,
        cache_to,
        mut targets,
        ..
    }: BuildDockerImage,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let metadata = cargo_metadata(msg_info)?;
    let version = metadata
        .get_package("cross")
        .expect("cross expected in workspace")
        .version
        .clone();
    if targets.is_empty() {
        if from_ci {
            targets = get_matrix()
                .iter()
                .filter(|m| m.os.starts_with("ubuntu"))
                .filter(|m| !m.disabled)
                .map(|m| m.to_image_target())
                .collect();
        } else {
            targets = walkdir::WalkDir::new(metadata.workspace_root.join("docker"))
                .max_depth(1)
                .contents_first(true)
                .into_iter()
                .filter_map(|e| e.ok().filter(|f| f.file_type().is_file()))
                .filter_map(|f| {
                    f.file_name()
                        .to_string_lossy()
                        .strip_prefix("Dockerfile.")
                        .map(ToOwned::to_owned)
                        .map(|s| s.parse().unwrap())
                })
                .collect();
        }
    }
    let gha = std::env::var("GITHUB_ACTIONS").is_ok();
    let mut progress = progress.map(|x| x.parse().unwrap());
    if gha {
        progress = Some(Progress::Plain);
    }
    let root = metadata.workspace_root;
    let docker_root = root.join("docker");
    let cross_toolchains_root = docker_root.join("cross-toolchains").join("docker");
    let targets = targets
        .into_iter()
        .map(|t| locate_dockerfile(t, &docker_root, &cross_toolchains_root))
        .collect::<cross::Result<Vec<_>>>()?;

    let platforms = if platform.is_empty() {
        DEFAULT_PLATFORMS.to_vec()
    } else {
        platform
    };

    let mut results = vec![];
    for (platform, (target, dockerfile)) in targets
        .iter()
        .flat_map(|t| platforms.iter().map(move |p| (p, t)))
    {
        if gha && targets.len() > 1 {
            gha_print("::group::Build {target}");
        } else {
            msg_info.note(format_args!("Build {target} for {}", platform.target))?;
        }
        let mut docker_build = engine.command();
        docker_build.invoke_build_command();
        let has_buildkit = docker::Engine::has_buildkit();
        docker_build.current_dir(&docker_root);

        let docker_platform = platform.docker_platform();
        let mut dockerfile = dockerfile.clone();
        docker_build.args(["--platform", &docker_platform]);
        let uppercase_triple = target.name.to_ascii_uppercase().replace('-', "_");
        docker_build.args([
            "--build-arg",
            &format!("CROSS_TARGET_TRIPLE={}", uppercase_triple),
        ]);
        // add our platform, and determine if we need to use a native docker image
        if has_native_image(docker_platform.as_str(), target, msg_info)? {
            let dockerfile_name = match target.sub.as_deref() {
                Some(sub) => format!("Dockerfile.native.{sub}"),
                None => "Dockerfile.native".to_owned(),
            };
            let dockerfile_path = docker_root.join(&dockerfile_name);
            if !dockerfile_path.exists() {
                eyre::bail!(
                    "unable to find native dockerfile named {dockerfile_name} for target {target}."
                );
            }
            dockerfile = dockerfile_path.to_utf8()?.to_string();
        }

        if push {
            docker_build.arg("--push");
        } else if engine.kind.supports_output_flag() && no_output {
            docker_build.args(["--output", "type=tar,dest=/dev/null"]);
        } else if no_output {
            msg_info.fatal("cannot specify `--no-output` with engine that does not support the `--output` flag", 1);
        } else if has_buildkit {
            docker_build.arg("--load");
        }

        let mut tags = vec![];

        match (ref_type.as_deref(), ref_name.as_deref()) {
            (Some(ref_type), Some(ref_name)) => tags.extend(determine_image_name(
                target,
                &repository,
                ref_type,
                ref_name,
                is_latest,
                &version,
            )?),
            _ => {
                if push && tag_override.is_none() {
                    panic!("Refusing to push without tag or branch. Specify a repository and tag with `--repository <repository> --tag <tag>`")
                }
                tags.push(target.image_name(&repository, "local"));
            }
        }

        if let Some(ref tag) = tag_override {
            tags = vec![target.image_name(&repository, tag)];
        }

        if engine.kind.supports_pull_flag() {
            docker_build.arg("--pull");
        }
        let base_name = format!("{repository}/{}", target.name);
        if no_cache {
            docker_build.arg("--no-cache");
        } else if engine.kind.supports_cache_from_type() {
            docker_build.args([
                "--cache-from",
                &cache_from.replace("{base_name}", &base_name),
            ]);
        } else {
            // we can't use `image_name` since podman doesn't support tags
            // with `--cache-from`. podman only supports an image format
            // of registry/repo although it does when pulling images. this
            // affects building from cache with target+subs images since we
            // can't use caches from registry. this is only an issue if
            // building with podman without a local cache, which never
            // happens in practice.
            docker_build.args(["--cache-from", &base_name]);
        }

        if push {
            docker_build.args(["--cache-to", "type=inline"]);
        } else if let Some(ref cache_to) = cache_to {
            docker_build.args(["--cache-to", &cache_to.replace("{base_name}", &base_name)]);
        }

        for tag in &tags {
            docker_build.args(["--tag", tag]);
        }

        for label in labels
            .as_deref()
            .unwrap_or("")
            .split('\n')
            .filter(|s| !s.is_empty())
        {
            docker_build.args(["--label", label]);
        }

        docker_build.cross_labels(&target.name, platform.target.triple());
        docker_build.args(["--file", &dockerfile]);

        docker_build.progress(progress)?;
        docker_build.verbose(msg_info.verbosity);
        for arg in &build_arg {
            docker_build.args(["--build-arg", arg]);
        }

        if let Some(opts) = &build_opts {
            docker_build.args(docker::Engine::parse_opts(opts)?);
        }

        docker_build.arg(match target.needs_workspace_root_context() {
            true => root.as_path(),
            false => Path::new("."),
        });

        if !dry_run && (force || !push || gha) {
            let result = docker_build
                .run(msg_info, false)
                .engine_warning(engine)
                .buildkit_warning();
            if gha && targets.len() > 1 {
                if let Err(e) = &result {
                    // TODO: Determine what instruction errorred, and place warning on that line with appropriate warning
                    gha_error(&format!("file=docker/{dockerfile},title=Build failed::{e}"));
                }
            }
            results.push(
                result
                    .map(|_| target.clone())
                    .map_err(|e| (target.clone(), e)),
            );
            if !no_fastfail && results.last().unwrap().is_err() {
                break;
            }
        } else {
            docker_build.print(msg_info)?;
            if !dry_run {
                msg_info.fatal("refusing to push, use --force to override", 1);
            }
        }
        if gha {
            gha_output("image", &tags[0])?;
            gha_output("images", &format!("'{}'", serde_json::to_string(&tags)?))?;
            if targets.len() > 1 {
                gha_print("::endgroup::");
            }
        }
    }
    if gha {
        std::env::set_var("GITHUB_STEP_SUMMARY", job_summary(&results)?);
    }
    if results.iter().any(|r| r.is_err()) {
        #[allow(unknown_lints, clippy::manual_try_fold)]
        return Err(crate::util::with_section_reports(
            eyre::eyre!("some error(s) encountered"),
            results.into_iter().filter_map(Result::err).map(|e| e.1),
        ));
    }
    Ok(())
}

fn has_native_image(
    platform: &str,
    target: &ImageTarget,
    msg_info: &mut MessageInfo,
) -> cross::Result<bool> {
    let note_host_target_detection = |msg_info: &mut MessageInfo| -> cross::Result<()> {
        msg_info.note("using the rust target triple to determine the host triple to determine if the docker platform is native. this may fail if cross-compiling xtask.")
    };

    Ok(match target.sub.as_deref() {
        // FIXME: add additional subs for new Linux distros, such as alpine.
        None | Some("centos") => match (platform, target.name.as_str()) {
            ("linux/386", "i686-unknown-linux-gnu")
            | ("linux/amd64", "x86_64-unknown-linux-gnu")
            | ("linux/arm64" | "linux/arm64/v8", "aarch64-unknown-linux-gnu")
            | ("linux/ppc64le", "powerpc64le-unknown-linux-gnu")
            | ("linux/riscv64", "riscv64gc-unknown-linux-gnu")
            | ("linux/s390x", "s390x-unknown-linux-gnu") => true,
            ("linux/arm/v6", "arm-unknown-linux-gnueabi") if target.is_armv6() => {
                note_host_target_detection(msg_info)?;
                true
            }
            ("linux/arm" | "linux/arm/v7", "armv7-unknown-linux-gnueabihf")
                if target.is_armv7() =>
            {
                note_host_target_detection(msg_info)?;
                true
            }
            _ => false,
        },
        Some(_) => false,
    })
}

pub fn determine_image_name(
    target: &ImageTarget,
    repository: &str,
    ref_type: &str,
    ref_name: &str,
    is_latest: bool,
    version: &str,
) -> cross::Result<Vec<String>> {
    let mut tags = vec![];
    match (ref_type, ref_name) {
        ("tag", ref_name) if ref_name.starts_with('v') => {
            let tag_version = ref_name
                .strip_prefix('v')
                .expect("tag name should start with v");
            if version != tag_version {
                eyre::bail!("git tag does not match package version.")
            }
            tags.push(target.image_name(repository, version));
            // Check for unstable releases, tag stable releases as `latest`
            if is_latest {
                tags.push(target.image_name(repository, "latest"))
            }
        }
        ("branch", ref_name) => {
            if let Some(gh_queue) = ref_name.strip_prefix("gh-readonly-queue/") {
                let (_, source) = gh_queue
                    .split_once('/')
                    .ok_or_else(|| eyre::eyre!("invalid gh-readonly-queue branch name"))?;
                tags.push(target.image_name(repository, source));
            } else {
                tags.push(target.image_name(repository, ref_name));
            }

            if ["staging", "trying"]
                .iter()
                .any(|branch| branch != &ref_name)
            {
                tags.push(target.image_name(repository, "edge"));
            }
        }
        _ => eyre::bail!("no valid choice to pick for image name"),
    }
    Ok(tags)
}

pub fn job_summary(
    results: &[Result<ImageTarget, (ImageTarget, eyre::ErrReport)>],
) -> cross::Result<String> {
    let mut summary = "# SUMMARY\n\n".to_string();
    let success: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    let errors: Vec<_> = results.iter().filter_map(|r| r.as_ref().err()).collect();

    if !success.is_empty() {
        summary.push_str("## Success\n\n| Target |\n| ------ |\n");
    }

    for target in success {
        writeln!(summary, "| {} |", target.alt())?;
    }

    if !errors.is_empty() {
        // TODO: Tee error output and show in summary
        summary.push_str("\n## Errors\n\n| Target |\n| ------ |\n");
    }

    for (target, _) in errors {
        writeln!(summary, "| {target} |")?;
    }
    Ok(summary)
}
