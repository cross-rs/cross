use std::collections::BTreeMap;
use std::fmt::Write;
use std::io::Write as _;
use std::path::Path;

use crate::util::{cargo_metadata, gha_error, gha_output, gha_print};
use clap::Args;
use cross::docker::ImagePlatform;
use cross::shell::MessageInfo;
use cross::{docker, CommandExt, ToUtf8};
use eyre::Context;
use serde::Serialize;

#[derive(Args, Debug)]
pub struct BuildDockerImage {
    #[clap(long, hide = true, env = "GITHUB_REF_TYPE")]
    pub ref_type: Option<String>,
    #[clap(long, hide = true, env = "GITHUB_REF_NAME")]
    ref_name: Option<String>,
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
    pub labels: Option<String>,
    /// Provide verbose diagnostic output.
    #[clap(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Coloring: auto, always, never
    #[clap(long)]
    pub color: Option<String>,
    /// Print but do not execute the build commands.
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
        default_value = "auto"
    )]
    pub progress: String,
    /// Use a bake build, specifying multiple platforms makes this implicit
    #[clap(long)]
    pub bake: bool,
    /// With bake action, append to the manifest instead of overwriting it.
    #[clap(long)]
    pub append: bool,
    /// Do not load from cache when building the image.
    #[clap(long)]
    pub no_cache: bool,
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
    pub targets: Vec<crate::ImageTarget>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BakeTarget {
    #[serde(skip, default)]
    image_target: Option<crate::ImageTarget>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    inherits: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dockerfile: Option<String>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    labels: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    platforms: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    args: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    no_cache: Option<bool>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    cache_from: Vec<String>,
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
        bake,
        append,
        no_cache,
        no_fastfail,
        from_ci,
        build_arg,
        platform,
        mut targets,
        ..
    }: BuildDockerImage,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let verbose = match verbose {
        0 => msg_info.is_verbose() as u8,
        v => v,
    };
    let metadata = cargo_metadata(msg_info)?;
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
    let root = metadata.workspace_root;
    let docker_root = root.join("docker");
    let cross_toolchains_root = docker_root.join("cross-toolchains").join("docker");
    let targets = targets
        .into_iter()
        .map(|t| locate_dockerfile(t, &docker_root, &cross_toolchains_root))
        .collect::<cross::Result<Vec<_>>>()?;

    let platforms = if platform.is_empty() {
        vec![ImagePlatform::DEFAULT]
    } else {
        platform
    };

    if push && tag_override.is_none() && ref_name.is_none() {
        panic!("Refusing to push without tag or branch. Specify a repository and tag with `--repository <repository> --tag <tag>`")
    }

    let progress = if gha || progress == "plain" {
        "plain"
    } else {
        &progress
    };

    let labels = labels
        .as_deref()
        .unwrap_or("")
        .split('\n')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let mut results = vec![];

    let bake = bake || platforms.len() > 1;

    if !bake {
        for (platform, (target, dockerfile)) in targets
            .iter()
            .flat_map(|t| platforms.iter().map(move |p| (p, t)))
        {
            if gha && targets.len() > 1 {
                gha_print("::group::Build {target}");
            } else {
                msg_info.note(format_args!("Build {target} for {}", platform.target))?;
            }
            let mut docker_build = docker::command(engine);
            docker_build.args(&["buildx", "build"]);
            docker_build.current_dir(&docker_root);

            docker_build.args(&["--platform", &platform.docker_platform()]);

            if push {
                if let Some("") = tag_override.as_deref() {
                    docker_build.args(&["--output", "type=registry,push-by-digest=true"]);
                } else {
                    docker_build.args(&["--output", "type=registry"]);
                }
                docker_build.args(&["--cache-to", "type=inline"]);
            } else if no_output {
                docker_build.args(&["--output", "type=tar,dest=/dev/null"]);
            } else {
                docker_build.args(&["--output", "type=docker"]);
            }

            let tags = get_tags(
                target,
                &repository,
                &version,
                is_latest,
                ref_type.as_deref(),
                ref_name.as_deref(),
                tag_override.as_deref(),
            )?;

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

            for tag in &tags {
                docker_build.args(&["--tag", tag]);
            }

            for label in &labels {
                docker_build.args(&["--label", label]);
            }

            for label in get_default_labels(target, platform) {
                docker_build.args(&["--label", &format!("{}={}", label.0, label.1)]);
            }

            docker_build.args(&["-f", dockerfile]);
            docker_build.args(&["--progress", progress]);

            for arg in &build_arg {
                docker_build.args(&["--build-arg", arg]);
            }

            if verbose > 1 {
                docker_build.args(&["--build-arg", "VERBOSE=1"]);
            }

            if target.needs_workspace_root_context() {
                docker_build.arg(&root);
            } else {
                docker_build.arg(".");
            }

            if !dry_run && (force || !push || gha) {
                let result = docker_build.run(msg_info, false);
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
                gha_output("image", &tags[0]);
                gha_output("images", &format!("'{}'", serde_json::to_string(&tags)?));
                if targets.len() > 1 {
                    gha_print("::endgroup::");
                }
            }
        }
    } else {
        let tag = tag_override.clone();
        let tag_override = if platforms.len() > 1 || append {
            Some("".to_owned())
        } else {
            tag_override
        };

        // Bake
        let labels = labels
            .into_iter()
            .map(|s| -> cross::Result<_> {
                let s = s
                    .split_once('=')
                    .ok_or_else(|| eyre::eyre!("invalid label `{s}`"))?;
                Ok((s.0.to_string(), s.1.to_string()))
            })
            .collect::<cross::Result<BTreeMap<_, _>>>()?;
        let build_args = build_arg
            .into_iter()
            .map(|s| -> cross::Result<_> {
                let s = s
                    .split_once('=')
                    .ok_or_else(|| eyre::eyre!("invalid build arg `{s}`"))?;
                Ok((s.0.to_string(), s.1.to_string()))
            })
            .collect::<cross::Result<BTreeMap<_, _>>>()?;
        let mut defaults = vec![];
        let mut bake_targets = targets
            .iter()
            .map(|(target, dockerfile)| -> cross::Result<_> {
                Ok((
                    target.to_string().replace('.', "-"),
                    BakeTarget {
                        image_target: None,
                        inherits: vec!["base".to_owned()],
                        dockerfile: Some(dockerfile.clone()),

                        context: if target.needs_workspace_root_context() {
                            Some(root.to_utf8()?.to_owned())
                        } else {
                            None
                        },
                        tags: get_tags(
                            target,
                            &repository,
                            &version,
                            is_latest,
                            ref_type.as_deref(),
                            ref_name.as_deref(),
                            tag_override.as_deref(),
                        )?,
                        labels: labels.clone(),
                        platforms: vec![],
                        args: build_args.clone(),
                        no_cache: None,
                        cache_from: if !no_cache {
                            vec![format!(
                                "type=registry,ref={}",
                                target.image_name(&repository, "main")
                            )]
                        } else {
                            vec![]
                        },
                    },
                ))
            })
            .collect::<cross::Result<BTreeMap<_, _>>>()?;
        for platform in &platforms {
            for (target, _dockerfile) in &targets {
                let name = format!("{target}-{}", platform.docker_platform())
                    .replace('.', "-")
                    .replace('/', "-");
                let target = BakeTarget {
                    image_target: Some(target.clone()),
                    inherits: vec![target.to_string()],
                    dockerfile: None,
                    context: if target.needs_workspace_root_context() {
                        Some(root.to_utf8()?.to_owned())
                    } else {
                        None
                    },
                    tags: vec![],
                    labels: get_default_labels(target, platform).into_iter().collect(),
                    platforms: vec![platform.docker_platform()],
                    args: build_args.clone(),
                    no_cache: None,
                    cache_from: if !no_cache {
                        vec![format!(
                            "type=registry,ref={}",
                            target.image_name(&repository, "main")
                        )]
                    } else {
                        vec![]
                    },
                };
                bake_targets.insert(name.clone(), target);
                defaults.push(name);
            }
        }

        bake_targets.insert(
            "base".to_owned(),
            BakeTarget {
                image_target: None,
                inherits: vec![],
                context: Some(".".to_owned()),
                dockerfile: None,
                tags: vec![],
                labels,
                platforms: vec![],
                args: build_args,
                no_cache: Some(no_cache),
                cache_from: vec![],
            },
        );
        let mut docker_bake = docker::command(engine);
        docker_bake.args(&["buildx", "bake"]);
        docker_bake.current_dir(&docker_root);
        docker_bake.arg("--pull");

        docker_bake.args(&["--progress", progress]);

        if verbose > 1 {
            docker_bake.args(&["--set", "*.args.VERBOSE=1"]);
        }

        if push {
            if let Some("") = tag_override.as_deref() {
                docker_bake.args(&["--set", "*.output=type=registry,push-by-digest=true"]);
            } else {
                docker_bake.args(&["--set", "*.output=type=registry"]);
            }
            //docker_bake.args(&["--set", "*.cache-to=type=inline"]);
        } else if no_output {
            docker_bake.args(&["--set", "*.output=type=tar,dest=/dev/null"]);
        } else {
            // if multi-platform, this will fail.
            docker_bake.args(&["--set", "*.output=type=docker"]);
        }

        if dry_run {
            docker_bake.arg("--print");
        }

        let mut temp_build_def = unsafe { cross::temp::TempFile::new()? };
        let mut temp_metadata = unsafe { cross::temp::TempFile::new()? };
        let content = serde_json::to_string_pretty(&serde_json::json!({
            "group": {
                "default": {
                    "targets": defaults,
                }
            },
            "target": bake_targets,
        }))?;
        write!(temp_build_def.file(), "{}", content).wrap_err("couldn't write to temp file")?;
        docker_bake.args(&["-f", temp_build_def.file().path().to_utf8()?]);
        docker_bake.args(&["--metadata-file", temp_metadata.file().path().to_utf8()?]);
        docker_bake.run(msg_info, false)?;

        if dry_run {
            return Ok(());
        }
        let metadata: serde_json::Value =
            serde_json::from_str(&cross::file::read(temp_metadata.path())?)?;

        let images = if platforms.len() > 1 || append {
            let mut bake_images: BTreeMap<crate::ImageTarget, Vec<String>> = BTreeMap::new();
            for (name, target) in bake_targets
                .iter()
                .filter_map(|(name, target)| -> Option<_> {
                    // Only get bake targets with a specific target
                    Some((name, target.image_target.as_ref()?))
                })
            {
                if let Some(image) = metadata.pointer(&format!("/{name}/containerimage.digest")) {
                    bake_images.entry(target.clone()).or_default().push(
                        image
                            .as_str()
                            .ok_or_else(|| eyre::eyre!("digest should be a string"))?
                            .to_owned(),
                    );
                }
            }
            let mut images = vec![];
            for (target, digest) in bake_images {
                let mut docker_itc = docker::command(engine);
                docker_itc.args(&["buildx", "imagetools", "create"]);

                if append {
                    docker_itc.arg("--append");
                }

                if !push || dry_run {
                    docker_itc.arg("--dry-run");
                }

                docker_itc.args(
                    digest
                        .into_iter()
                        .map(|digest| format!("{}@{digest}", target.image_name(&repository, ""))),
                );
                let tags = get_tags(
                    &target,
                    &repository,
                    &version,
                    is_latest,
                    ref_type.as_deref(),
                    ref_name.as_deref(),
                    tag.as_deref(),
                )?;
                for tag in &tags {
                    docker_itc.args(&["--tag", tag]);
                }
                images.extend(tags);

                docker_itc.run(msg_info, false)?;
            }
            images
        } else if let Some(map) = metadata.as_object() {
            let mut images = vec![];
            for (_, val) in map {
                if let Some(image) = val.pointer("/image.name") {
                    images.push(
                        image
                            .as_str()
                            .ok_or_else(|| eyre::eyre!("digest should be a string"))?
                            .to_owned(),
                    )
                }
            }
            images
        } else {
            eyre::bail!("metadata was not an object: {:?}", metadata)
        };
        if gha {
            gha_output("image", &images[0]);
            gha_output("images", &format!("'{}'", serde_json::to_string(&images)?));
        }
    }
    if gha {
        std::env::set_var("GITHUB_STEP_SUMMARY", job_summary(&results)?);
    }
    if results.iter().any(|r| r.is_err()) {
        results
            .into_iter()
            .filter_map(Result::err)
            .fold(Err(eyre::eyre!("encountered error(s)")), |_, e| Err(e.1))?;
    }
    Ok(())
}

fn get_default_labels(
    target: &crate::ImageTarget,
    platform: &ImagePlatform,
) -> Vec<(String, String)> {
    vec![
        (
            format!("{}.for-cross-target", cross::CROSS_LABEL_DOMAIN),
            target.name.clone(),
        ),
        (
            format!("{}.runs-with", cross::CROSS_LABEL_DOMAIN),
            platform.target.to_string(),
        ),
    ]
}
pub fn get_tags(
    target: &crate::ImageTarget,
    repository: &str,
    version: &str,
    is_latest: bool,
    ref_type: Option<&str>,
    ref_name: Option<&str>,
    tag_override: Option<&str>,
) -> cross::Result<Vec<String>> {
    if let Some(tag) = tag_override {
        return Ok(vec![target.image_name(repository, tag)]);
    }

    let mut tags = vec![];
    match (ref_type, ref_name) {
        (Some(ref_type), Some(ref_name)) => tags.extend(determine_image_name(
            target, repository, ref_type, ref_name, is_latest, version,
        )?),
        _ => {
            tags.push(target.image_name(repository, "local"));
        }
    }
    Ok(tags)
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
    results: &[Result<crate::ImageTarget, (crate::ImageTarget, eyre::ErrReport)>],
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
