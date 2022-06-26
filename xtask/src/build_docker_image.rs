use std::path::Path;

use crate::util::cargo_metadata;
use clap::Args;
use color_eyre::Section;
use cross::{docker, CommandExt, ToUtf8};
use std::fmt::Write;

#[derive(Args, Debug)]
pub struct BuildDockerImage {
    #[clap(long, hide = true, env = "GITHUB_REF_TYPE")]
    ref_type: Option<String>,
    #[clap(long, hide = true, env = "GITHUB_REF_NAME")]
    ref_name: Option<String>,
    #[clap(action, long = "latest", hide = true, env = "LATEST")]
    is_latest: bool,
    /// Specify a tag to use instead of the derived one, eg `local`
    #[clap(long)]
    tag: Option<String>,
    /// Repository name for image.
    #[clap(long, default_value = docker::CROSS_IMAGE)]
    repository: String,
    /// Newline separated labels
    #[clap(long, env = "LABELS")]
    labels: Option<String>,
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Print but do not execute the build commands.
    #[clap(long)]
    dry_run: bool,
    /// Force a push when `--push` is set, but not `--tag`
    #[clap(long, hide = true)]
    force: bool,
    /// Push build to registry.
    #[clap(short, long)]
    push: bool,
    /// Set output to /dev/null
    #[clap(short, long)]
    no_output: bool,
    /// Docker build progress output type.
    #[clap(
        long,
        value_parser = clap::builder::PossibleValuesParser::new(["auto", "plain", "tty"]),
        default_value = "auto"
    )]
    progress: String,
    /// Do not load from cache when building the image.
    #[clap(long)]
    no_cache: bool,
    /// Continue building images even if an image fails to build.
    #[clap(long)]
    no_fastfail: bool,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// If no target list is provided, parse list from CI.
    #[clap(long)]
    from_ci: bool,
    /// Additional build arguments to pass to Docker.
    #[clap(long)]
    build_arg: Vec<String>,
    /// Targets to build for
    #[clap()]
    targets: Vec<crate::ImageTarget>,
}

fn locate_dockerfile(
    target: crate::ImageTarget,
    docker_root: &Path,
    cross_toolchain_root: &Path,
) -> cross::Result<(crate::ImageTarget, String)> {
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
        is_latest,
        tag: tag_override,
        repository,
        labels,
        verbose,
        dry_run,
        force,
        push,
        no_output,
        progress,
        no_cache,
        no_fastfail,
        from_ci,
        build_arg,
        mut targets,
        ..
    }: BuildDockerImage,
    engine: &docker::Engine,
) -> cross::Result<()> {
    let metadata = cargo_metadata(verbose)?;
    let version = metadata
        .get_package("cross")
        .expect("cross expected in workspace")
        .version
        .clone();
    if targets.is_empty() {
        if from_ci {
            targets = crate::util::get_matrix()
                .iter()
                .filter(|m| m.os.starts_with("ubuntu"))
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
    let docker_root = metadata.workspace_root.join("docker");
    let cross_toolchains_root = docker_root.join("cross-toolchains").join("docker");
    let targets = targets
        .into_iter()
        .map(|t| locate_dockerfile(t, &docker_root, &cross_toolchains_root))
        .collect::<cross::Result<Vec<_>>>()?;

    let mut results = vec![];
    for (target, dockerfile) in &targets {
        if gha && targets.len() > 1 {
            println!("::group::Build {target}");
        }
        let mut docker_build = docker::command(engine);
        docker_build.args(&["buildx", "build"]);
        docker_build.current_dir(&docker_root);

        if push {
            docker_build.arg("--push");
        } else if no_output {
            docker_build.args(&["--output", "type=tar,dest=/dev/null"]);
        } else {
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

        docker_build.arg("--pull");
        if no_cache {
            docker_build.arg("--no-cache");
        } else {
            docker_build.args(&[
                "--cache-from",
                &format!(
                    "type=registry,ref={}",
                    target.image_name(&repository, "main")
                ),
            ]);
        }

        if push {
            docker_build.args(&["--cache-to", "type=inline"]);
        }

        for tag in &tags {
            docker_build.args(&["--tag", tag]);
        }

        for label in labels
            .as_deref()
            .unwrap_or("")
            .split('\n')
            .filter(|s| !s.is_empty())
        {
            docker_build.args(&["--label", label]);
        }

        docker_build.args([
            "--label",
            &format!("{}.for-cross-target={target}", cross::CROSS_LABEL_DOMAIN),
        ]);

        docker_build.args(&["-f", dockerfile]);

        if gha || progress == "plain" {
            docker_build.args(&["--progress", "plain"]);
        } else {
            docker_build.args(&["--progress", &progress]);
        }
        for arg in &build_arg {
            docker_build.args(&["--build-arg", arg]);
        }

        docker_build.arg(".");

        if !dry_run && (force || !push || gha) {
            let result = docker_build.run(verbose, false);
            if gha && targets.len() > 1 {
                if let Err(e) = &result {
                    // TODO: Determine what instruction errorred, and place warning on that line with appropriate warning
                    println!(
                        "::error file=docker/{},title=Build failed::{}",
                        dockerfile, e
                    )
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
            docker_build.print_verbose(true);
            if !dry_run {
                panic!("refusing to push, use --force to override");
            }
        }
        if gha {
            println!("::set-output name=image::{}", &tags[0]);
            println!(
                "::set-output name=images::'{}'",
                serde_json::to_string(&tags)?
            );
            if targets.len() > 1 {
                println!("::endgroup::");
            }
        }
    }
    if gha {
        std::env::set_var("GITHUB_STEP_SUMMARY", job_summary(&results)?);
    }
    if results.iter().any(|r| r.is_err()) {
        results.into_iter().filter_map(Result::err).fold(
            Err(eyre::eyre!("encountered error(s)")),
            |report: Result<(), color_eyre::Report>, e| report.error(e.1),
        )?;
    }
    Ok(())
}

pub fn determine_image_name(
    target: &crate::ImageTarget,
    repository: &str,
    ref_type: &str,
    ref_name: &str,
    is_latest: bool,
    version: &str,
) -> cross::Result<Vec<String>> {
    let mut tags = vec![];
    match (ref_type, ref_name) {
        (ref_type, ref_name) if ref_type == "tag" && ref_name.starts_with('v') => {
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
        (ref_type, ref_name) if ref_type == "branch" => {
            tags.push(target.image_name(repository, ref_name));

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
    results: &[Result<crate::ImageTarget, (crate::ImageTarget, cross::errors::CommandError)>],
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
