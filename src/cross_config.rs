use std::collections::HashMap;
use std::str::FromStr;

use crate::config::{bool_from_envvar, split_to_cloned_by_ws, Environment};
use crate::cross_toml::CargoConfigBehavior;
use crate::docker::custom::PreBuild;
use crate::docker::{ImagePlatform, PossibleImage};
use crate::shell::MessageInfo;
use crate::{CrossToml, Result, Target, TargetList};

#[derive(Debug)]
struct CrossEnvironment(Environment);

impl CrossEnvironment {
    fn new(map: Option<HashMap<&'static str, &'static str>>) -> Self {
        CrossEnvironment(Environment::new("CROSS", map))
    }

    fn get_values_for<T>(
        &self,
        var: &str,
        target: &Target,
        convert: impl Fn(&str) -> T,
    ) -> (Option<T>, Option<T>) {
        let target_values = self.get_target_var(target, var).map(|ref s| convert(s));

        let build_values = self.get_build_var(var).map(|ref s| convert(s));

        (build_values, target_values)
    }

    fn target_path(target: &Target, key: &str) -> String {
        format!("TARGET_{target}_{key}")
    }

    fn build_path(key: &str) -> String {
        if !key.starts_with("BUILD_") {
            format!("BUILD_{key}")
        } else {
            key.to_owned()
        }
    }

    fn get_build_var(&self, key: &str) -> Option<String> {
        self.0.get_var(&self.0.var_name(&Self::build_path(key)))
    }

    fn get_target_var(&self, target: &Target, key: &str) -> Option<String> {
        self.0
            .get_var(&self.0.var_name(&Self::target_path(target, key)))
    }

    fn xargo(&self, target: &Target) -> (Option<bool>, Option<bool>) {
        self.get_values_for("XARGO", target, bool_from_envvar)
    }

    fn build_std(&self, target: &Target) -> (Option<bool>, Option<bool>) {
        self.get_values_for("BUILD_STD", target, bool_from_envvar)
    }

    fn zig(&self, target: &Target) -> (Option<bool>, Option<bool>) {
        self.get_values_for("ZIG", target, bool_from_envvar)
    }

    fn zig_version(&self, target: &Target) -> (Option<String>, Option<String>) {
        self.get_values_for("ZIG_VERSION", target, ToOwned::to_owned)
    }

    fn zig_image(&self, target: &Target) -> Result<(Option<PossibleImage>, Option<PossibleImage>)> {
        let get_build = |env: &CrossEnvironment, var: &str| env.get_build_var(var);
        let get_target = |env: &CrossEnvironment, var: &str| env.get_target_var(target, var);
        let env_build = get_possible_image(
            self,
            "ZIG_IMAGE",
            "ZIG_IMAGE_TOOLCHAIN",
            get_build,
            get_build,
        )?;
        let env_target = get_possible_image(
            self,
            "ZIG_IMAGE",
            "ZIG_IMAGE_TOOLCHAIN",
            get_target,
            get_target,
        )?;

        Ok((env_build, env_target))
    }

    fn image(&self, target: &Target) -> Result<Option<PossibleImage>> {
        let get_target = |env: &CrossEnvironment, var: &str| env.get_target_var(target, var);
        get_possible_image(self, "IMAGE", "IMAGE_TOOLCHAIN", get_target, get_target)
    }

    fn dockerfile(&self, target: &Target) -> (Option<String>, Option<String>) {
        self.get_values_for("DOCKERFILE", target, |s| s.to_owned())
    }

    fn dockerfile_context(&self, target: &Target) -> (Option<String>, Option<String>) {
        self.get_values_for("DOCKERFILE_CONTEXT", target, |s| s.to_owned())
    }

    fn pre_build(&self, target: &Target) -> (Option<PreBuild>, Option<PreBuild>) {
        self.get_values_for("PRE_BUILD", target, |v| {
            let v: Vec<_> = v.split('\n').map(String::from).collect();
            if v.len() == 1 {
                PreBuild::Single {
                    line: v.into_iter().next().expect("should contain one item"),
                    env: true,
                }
            } else {
                PreBuild::Lines(v)
            }
        })
    }

    fn runner(&self, target: &Target) -> Option<String> {
        self.get_target_var(target, "RUNNER")
    }

    fn cargo_config(
        &self,
        target: &Target,
    ) -> Result<(Option<CargoConfigBehavior>, Option<CargoConfigBehavior>)> {
        let (build, target) =
            self.get_values_for("ENV_CARGO_CONFIG", target, CargoConfigBehavior::from_str);
        Ok(match (build, target) {
            (Some(b), Some(t)) => (Some(b?), Some(t?)),
            (Some(b), None) => (Some(b?), None),
            (None, Some(t)) => (None, Some(t?)),
            (None, None) => (None, None),
        })
    }

    fn passthrough(&self, target: &Target) -> (Option<Vec<String>>, Option<Vec<String>>) {
        self.get_values_for("ENV_PASSTHROUGH", target, split_to_cloned_by_ws)
    }

    fn volumes(&self, target: &Target) -> (Option<Vec<String>>, Option<Vec<String>>) {
        self.get_values_for("ENV_VOLUMES", target, split_to_cloned_by_ws)
    }

    fn target(&self) -> Option<String> {
        self.get_build_var("TARGET")
            .or_else(|| std::env::var("CARGO_BUILD_TARGET").ok())
    }

    fn doctests(&self) -> Option<bool> {
        self.0
            .get_var("CROSS_UNSTABLE_ENABLE_DOCTESTS")
            .map(|s| bool_from_envvar(&s))
    }

    fn custom_toolchain(&self) -> bool {
        self.0
            .get_var("CROSS_CUSTOM_TOOLCHAIN")
            .map_or(false, |s| bool_from_envvar(&s))
    }

    fn custom_toolchain_compat(&self) -> Option<String> {
        self.0.get_var("CUSTOM_TOOLCHAIN_COMPAT")
    }

    fn build_opts(&self) -> Option<String> {
        self.0.get_var("CROSS_BUILD_OPTS")
    }
}

fn get_possible_image(
    env: &CrossEnvironment,
    image_var: &str,
    toolchain_var: &str,
    get_image: impl Fn(&CrossEnvironment, &str) -> Option<String>,
    get_toolchain: impl Fn(&CrossEnvironment, &str) -> Option<String>,
) -> Result<Option<PossibleImage>> {
    get_image(env, image_var)
        .map(Into::into)
        .map(|mut i: PossibleImage| {
            if let Some(toolchain) = get_toolchain(env, toolchain_var) {
                i.toolchain = toolchain
                    .split(',')
                    .map(|t| ImagePlatform::from_target(t.trim().into()))
                    .collect::<Result<Vec<_>>>()?;
                Ok(i)
            } else {
                Ok(i)
            }
        })
        .transpose()
}

#[derive(Debug)]
pub struct CrossConfig {
    toml: Option<CrossToml>,
    env: CrossEnvironment,
}

impl CrossConfig {
    pub fn new(toml: Option<CrossToml>) -> Self {
        CrossConfig {
            toml,
            env: CrossEnvironment::new(None),
        }
    }

    pub fn confusable_target(&self, target: &Target, msg_info: &mut MessageInfo) -> Result<()> {
        if let Some(keys) = self.toml.as_ref().map(|t| t.targets.keys()) {
            for mentioned_target in keys {
                let mentioned_target_norm = mentioned_target
                    .to_string()
                    .replace(|c| c == '-' || c == '_', "")
                    .to_lowercase();
                let target_norm = target
                    .to_string()
                    .replace(|c| c == '-' || c == '_', "")
                    .to_lowercase();
                if mentioned_target != target && mentioned_target_norm == target_norm {
                    msg_info.warn("a target named \"{mentioned_target}\" is mentioned in the Cross configuration, but the current specified target is \"{target}\".")?;
                    msg_info.status(" > Is the target misspelled in the Cross configuration?")?;
                }
            }
        }
        Ok(())
    }

    fn bool_from_config(
        &self,
        target: &Target,
        env: impl Fn(&CrossEnvironment, &Target) -> (Option<bool>, Option<bool>),
        config: impl Fn(&CrossToml, &Target) -> (Option<bool>, Option<bool>),
    ) -> Option<bool> {
        let (env_build, env_target) = env(&self.env, target);
        let (toml_build, toml_target) = if let Some(ref toml) = self.toml {
            config(toml, target)
        } else {
            (None, None)
        };

        match (env_target, toml_target) {
            (Some(value), _) => return Some(value),
            (None, Some(value)) => return Some(value),
            (None, None) => {}
        };

        match (env_build, toml_build) {
            (Some(value), _) => return Some(value),
            (None, Some(value)) => return Some(value),
            (None, None) => {}
        };

        None
    }

    fn vec_from_config(
        &self,
        target: &Target,
        env: impl for<'a> Fn(
            &'a CrossEnvironment,
            &Target,
        ) -> (Option<Vec<String>>, Option<Vec<String>>),
        config: impl for<'a> Fn(&'a CrossToml, &Target) -> (Option<&'a [String]>, Option<&'a [String]>),
        sum: bool,
    ) -> Result<Option<Vec<String>>> {
        if sum {
            let (mut env_build, env_target) = env(&self.env, target);
            env_build
                .as_mut()
                .map(|b| env_target.map(|mut t| b.append(&mut t)));
            self.sum_of_env_toml_values(env_build, |t| config(t, target))
        } else {
            self.get_from_ref(target, env, config)
        }
    }

    fn get_from_ref<T, U>(
        &self,
        target: &Target,
        env: impl for<'a> Fn(&'a CrossEnvironment, &Target) -> (Option<T>, Option<T>),
        config: impl for<'a> Fn(&'a CrossToml, &Target) -> (Option<&'a U>, Option<&'a U>),
    ) -> Result<Option<T>>
    where
        U: ToOwned<Owned = T> + ?Sized,
    {
        let (env_build, env_target) = env(&self.env, target);

        if let Some(env_target) = env_target {
            return Ok(Some(env_target));
        }

        let (build, target) = self
            .toml
            .as_ref()
            .map(|t| config(t, target))
            .unwrap_or_default();

        // FIXME: let expression
        if target.is_none() && env_build.is_some() {
            Ok(env_build)
        } else if target.is_none() {
            Ok(build.map(ToOwned::to_owned))
        } else {
            Ok(target.map(ToOwned::to_owned))
        }
    }

    fn get_from_value<T>(
        &self,
        target: &Target,
        env: impl Fn(&CrossEnvironment, &Target) -> (Option<T>, Option<T>),
        config: impl Fn(&CrossToml, &Target) -> (Option<T>, Option<T>),
    ) -> Result<Option<T>> {
        let (env_build, env_target) = env(&self.env, target);

        if let Some(env_target) = env_target {
            return Ok(Some(env_target));
        }

        let (build, target) = self
            .toml
            .as_ref()
            .map(|t| config(t, target))
            .unwrap_or_default();

        // FIXME: let expression
        if target.is_none() && env_build.is_some() {
            Ok(env_build)
        } else if target.is_none() {
            Ok(build)
        } else {
            Ok(target)
        }
    }

    #[cfg(test)]
    fn new_with(toml: Option<CrossToml>, env: CrossEnvironment) -> Self {
        CrossConfig { toml, env }
    }

    pub fn xargo(&self, target: &Target) -> Option<bool> {
        self.bool_from_config(target, CrossEnvironment::xargo, CrossToml::xargo)
    }

    pub fn build_std(&self, target: &Target) -> Option<bool> {
        self.bool_from_config(target, CrossEnvironment::build_std, CrossToml::build_std)
    }

    pub fn zig(&self, target: &Target) -> Option<bool> {
        self.bool_from_config(target, CrossEnvironment::zig, CrossToml::zig)
    }

    pub fn zig_version(&self, target: &Target) -> Result<Option<String>> {
        self.get_from_value(
            target,
            CrossEnvironment::zig_version,
            CrossToml::zig_version,
        )
    }

    pub fn zig_image(&self, target: &Target) -> Result<Option<PossibleImage>> {
        let (b, t) = self.env.zig_image(target)?;
        self.get_from_value(target, |_, _| (b.clone(), t.clone()), CrossToml::zig_image)
    }

    pub fn image(&self, target: &Target) -> Result<Option<PossibleImage>> {
        let env = self.env.image(target)?;
        self.get_from_ref(
            target,
            move |_, _| (None, env.clone()),
            |toml, target| (None, toml.image(target)),
        )
    }

    pub fn runner(&self, target: &Target) -> Result<Option<String>> {
        self.get_from_ref(
            target,
            |env, target| (None, env.runner(target)),
            |toml, target| (None, toml.runner(target)),
        )
    }

    pub fn doctests(&self) -> Option<bool> {
        self.env.doctests()
    }

    pub fn custom_toolchain(&self) -> bool {
        self.env.custom_toolchain()
    }

    pub fn custom_toolchain_compat(&self) -> Option<String> {
        self.env.custom_toolchain_compat()
    }

    pub fn build_opts(&self) -> Option<String> {
        self.env.build_opts()
    }

    pub fn env_cargo_config(&self, target: &Target) -> Result<Option<CargoConfigBehavior>> {
        let (env_build, env_target) = self.env.cargo_config(target)?;

        if let Some(env_target) = env_target {
            return Ok(Some(env_target));
        }

        let (build, target) = self
            .toml
            .as_ref()
            .map(|t| t.env_cargo_config(target))
            .unwrap_or_default();

        // FIXME: let expression
        if target.is_none() && env_build.is_some() {
            Ok(env_build)
        } else if target.is_none() {
            Ok(build)
        } else {
            Ok(target)
        }
    }

    pub fn env_passthrough(&self, target: &Target) -> Result<Option<Vec<String>>> {
        self.vec_from_config(
            target,
            CrossEnvironment::passthrough,
            CrossToml::env_passthrough,
            true,
        )
    }

    pub fn env_volumes(&self, target: &Target) -> Result<Option<Vec<String>>> {
        self.get_from_ref(target, CrossEnvironment::volumes, CrossToml::env_volumes)
    }

    pub fn target(&self, target_list: &TargetList) -> Option<Target> {
        if let Some(env_value) = self.env.target() {
            return Some(Target::from(&env_value, target_list));
        }
        self.toml
            .as_ref()
            .and_then(|t| t.default_target(target_list))
    }

    pub fn dockerfile(&self, target: &Target) -> Result<Option<String>> {
        self.get_from_ref(target, CrossEnvironment::dockerfile, CrossToml::dockerfile)
    }

    pub fn dockerfile_context(&self, target: &Target) -> Result<Option<String>> {
        self.get_from_ref(
            target,
            CrossEnvironment::dockerfile_context,
            CrossToml::dockerfile_context,
        )
    }

    pub fn dockerfile_build_args(
        &self,
        target: &Target,
    ) -> Result<Option<HashMap<String, String>>> {
        // This value does not support env variables
        self.toml
            .as_ref()
            .map_or(Ok(None), |t| Ok(t.dockerfile_build_args(target)))
    }

    pub fn pre_build(&self, target: &Target) -> Result<Option<PreBuild>> {
        self.get_from_ref(target, CrossEnvironment::pre_build, CrossToml::pre_build)
    }

    // FIXME: remove when we disable sums in 0.3.0.
    fn sum_of_env_toml_values<'a>(
        &'a self,
        env_values: Option<impl AsRef<[String]>>,
        toml_getter: impl FnOnce(&'a CrossToml) -> (Option<&'a [String]>, Option<&'a [String]>),
    ) -> Result<Option<Vec<String>>> {
        let mut defined = false;
        let mut collect = vec![];
        if let Some(vars) = env_values {
            collect.extend(vars.as_ref().iter().cloned());
            defined = true;
        } else if let Some((build, target)) = self.toml.as_ref().map(toml_getter) {
            if let Some(build) = build {
                collect.extend(build.iter().cloned());
                defined = true;
            }
            if let Some(target) = target {
                collect.extend(target.iter().cloned());
                defined = true;
            }
        }
        if !defined {
            Ok(None)
        } else {
            Ok(Some(collect))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::*;
    use crate::{Target, TargetList};

    fn target_list() -> TargetList {
        TargetList {
            triples: vec![
                "aarch64-unknown-linux-gnu".to_owned(),
                "armv7-unknown-linux-musleabihf".to_owned(),
            ],
        }
    }

    fn target() -> Target {
        let target_list = target_list();
        Target::from("aarch64-unknown-linux-gnu", &target_list)
    }

    fn target2() -> Target {
        let target_list = target_list();
        Target::from("armv7-unknown-linux-musleabihf", &target_list)
    }

    mod test_environment {

        use super::*;

        #[test]
        pub fn parse_error_in_env() {
            let mut map = std::collections::HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "tru");
            map.insert("CROSS_BUILD_STD", "false");

            let env = CrossEnvironment::new(Some(map));
            assert_eq!(env.xargo(&target()), (Some(true), None));
            assert_eq!(env.build_std(&target()), (Some(false), None));
        }

        #[test]
        pub fn build_and_target_set_returns_tuple() {
            let mut map = std::collections::HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "true");
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "false");

            let env = CrossEnvironment::new(Some(map));
            assert_eq!(env.xargo(&target()), (Some(true), Some(false)));
        }

        #[test]
        pub fn target_build_var_name() {
            let map = std::collections::HashMap::new();

            let env = CrossEnvironment::new(Some(map));
            assert_eq!(env.0.var_name("build_xargo"), "CROSS_BUILD_XARGO");
            assert_eq!(
                env.0.var_name("target_aarch64-unknown-linux-gnu_XARGO"),
                "CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO"
            );
            assert_eq!(
                env.0.var_name("target-aarch64-unknown-linux-gnu_image"),
                "CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_IMAGE"
            );
        }

        #[test]
        pub fn collect_passthrough() {
            let mut map = std::collections::HashMap::new();
            map.insert("CROSS_BUILD_ENV_PASSTHROUGH", "TEST1 TEST2");
            map.insert(
                "CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_ENV_PASSTHROUGH",
                "PASS1 PASS2",
            );

            let env = CrossEnvironment::new(Some(map));

            let (build, target) = env.passthrough(&target());
            assert!(build.as_ref().unwrap().contains(&"TEST1".to_owned()));
            assert!(build.as_ref().unwrap().contains(&"TEST2".to_owned()));
            assert!(target.as_ref().unwrap().contains(&"PASS1".to_owned()));
            assert!(target.as_ref().unwrap().contains(&"PASS2".to_owned()));
        }
    }

    #[cfg(test)]
    mod test_config {

        use super::*;

        macro_rules! s {
            ($x:literal) => {
                $x.to_owned()
            };
        }

        fn toml(content: &str) -> Result<crate::CrossToml> {
            Ok(
                CrossToml::parse_from_cross(content, &mut MessageInfo::default())
                    .wrap_err("couldn't parse toml")?
                    .0,
            )
        }

        #[test]
        pub fn env_and_toml_build_xargo_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "true");
            map.insert(
                "CROSS_BUILD_PRE_BUILD",
                "apt-get update\napt-get install zlib-dev",
            );

            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_BUILD_XARGO_FALSE)?), env);
            assert_eq!(config.xargo(&target()), Some(true));
            assert_eq!(config.build_std(&target()), None);
            assert_eq!(
                config.pre_build(&target())?,
                Some(PreBuild::Lines(vec![
                    s!("apt-get update"),
                    s!("apt-get install zlib-dev")
                ]))
            );

            Ok(())
        }

        #[test]
        pub fn env_target_and_toml_target_xargo_target_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "true");
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_BUILD_STD", "true");
            let env = CrossEnvironment::new(Some(map));

            let config = CrossConfig::new_with(Some(toml(TOML_TARGET_XARGO_FALSE)?), env);
            assert_eq!(config.xargo(&target()), Some(true));
            assert_eq!(config.build_std(&target()), Some(true));
            assert_eq!(config.pre_build(&target())?, None);

            Ok(())
        }

        #[test]
        pub fn env_target_and_toml_build_xargo_then_use_toml() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "true");

            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_BUILD_XARGO_FALSE)?), env);
            assert_eq!(config.xargo(&target()), Some(true));
            assert_eq!(config.build_std(&target()), None);
            assert_eq!(config.pre_build(&target())?, None);

            Ok(())
        }

        #[test]
        pub fn env_target_and_toml_build_pre_build_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert(
                "CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_PRE_BUILD",
                "dpkg --add-architecture arm64",
            );

            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_BUILD_PRE_BUILD)?), env);
            assert_eq!(
                config.pre_build(&target())?,
                Some(PreBuild::Single {
                    line: s!("dpkg --add-architecture arm64"),
                    env: true
                })
            );

            Ok(())
        }

        #[test]
        pub fn env_target_then_toml_target_then_env_build_then_toml_build() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_BUILD_DOCKERFILE", "Dockerfile3");
            map.insert(
                "CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_DOCKERFILE",
                "Dockerfile4",
            );

            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_BUILD_DOCKERFILE)?), env);
            assert_eq!(config.dockerfile(&target())?, Some(s!("Dockerfile4")));
            assert_eq!(config.dockerfile(&target2())?, Some(s!("Dockerfile3")));

            let map = HashMap::new();
            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_BUILD_DOCKERFILE)?), env);
            assert_eq!(config.dockerfile(&target())?, Some(s!("Dockerfile2")));
            assert_eq!(config.dockerfile(&target2())?, Some(s!("Dockerfile1")));

            Ok(())
        }

        #[test]
        pub fn toml_build_passthrough_then_use_target_passthrough_both() -> Result<()> {
            let map = HashMap::new();
            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_ARRAYS_BOTH)?), env);
            assert_eq!(
                config.env_passthrough(&target())?,
                Some(vec![s!("VAR1"), s!("VAR2"), s!("VAR3"), s!("VAR4")])
            );
            assert_eq!(
                config.env_volumes(&target())?,
                Some(vec![s!("VOLUME3"), s!("VOLUME4")])
            );

            Ok(())
        }

        #[test]
        pub fn toml_build_passthrough() -> Result<()> {
            let map = HashMap::new();
            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_ARRAYS_BUILD)?), env);
            assert_eq!(
                config.env_passthrough(&target())?,
                Some(vec![s!("VAR1"), s!("VAR2")])
            );
            assert_eq!(
                config.env_volumes(&target())?,
                Some(vec![s!("VOLUME1"), s!("VOLUME2")])
            );

            Ok(())
        }

        #[test]
        pub fn toml_target_passthrough() -> Result<()> {
            let map = HashMap::new();
            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_ARRAYS_TARGET)?), env);
            assert_eq!(
                config.env_passthrough(&target())?,
                Some(vec![s!("VAR3"), s!("VAR4")])
            );
            assert_eq!(
                config.env_volumes(&target())?,
                Some(vec![s!("VOLUME3"), s!("VOLUME4")])
            );

            Ok(())
        }

        #[test]
        pub fn volumes_use_env_over_toml() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_BUILD_ENV_VOLUMES", "VOLUME1 VOLUME2");
            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_BUILD_VOLUMES)?), env);
            let expected = vec![s!("VOLUME1"), s!("VOLUME2")];

            let result = config.env_volumes(&target()).unwrap().unwrap_or_default();
            dbg!(&result);
            assert!(result.len() == 2);
            assert!(result.contains(&expected[0]));
            assert!(result.contains(&expected[1]));

            Ok(())
        }

        #[test]
        pub fn volumes_use_toml_when_no_env() -> Result<()> {
            let map = HashMap::new();
            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_BUILD_VOLUMES)?), env);
            let expected = vec![s!("VOLUME3"), s!("VOLUME4")];

            let result = config.env_volumes(&target()).unwrap().unwrap_or_default();
            dbg!(&result);
            assert!(result.len() == 2);
            assert!(result.contains(&expected[0]));
            assert!(result.contains(&expected[1]));

            Ok(())
        }

        #[test]
        pub fn no_env_and_no_toml_default_target_then_none() -> Result<()> {
            let config = CrossConfig::new_with(None, CrossEnvironment::new(None));
            let config_target = config.target(&target_list());
            assert_eq!(config_target, None);

            Ok(())
        }

        #[test]
        pub fn env_and_toml_default_target_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_BUILD_TARGET", "armv7-unknown-linux-musleabihf");
            let env = CrossEnvironment::new(Some(map));
            let config = CrossConfig::new_with(Some(toml(TOML_DEFAULT_TARGET)?), env);

            let config_target = config.target(&target_list()).unwrap();
            assert_eq!(config_target.triple(), "armv7-unknown-linux-musleabihf");

            Ok(())
        }

        #[test]
        pub fn no_env_but_toml_default_target_then_use_toml() -> Result<()> {
            let env = CrossEnvironment::new(None);
            let config = CrossConfig::new_with(Some(toml(TOML_DEFAULT_TARGET)?), env);

            let config_target = config.target(&target_list()).unwrap();
            assert_eq!(config_target.triple(), "aarch64-unknown-linux-gnu");

            Ok(())
        }

        static TOML_BUILD_XARGO_FALSE: &str = r#"
    [build]
    xargo = false
    "#;

        static TOML_BUILD_PRE_BUILD: &str = r#"
    [build]
    pre-build = ["apt-get update && apt-get install zlib-dev"]
    "#;

        static TOML_BUILD_DOCKERFILE: &str = r#"
    [build]
    dockerfile = "Dockerfile1"
    [target.aarch64-unknown-linux-gnu]
    dockerfile = "Dockerfile2"
    "#;

        static TOML_TARGET_XARGO_FALSE: &str = r#"
    [target.aarch64-unknown-linux-gnu]
    xargo = false
    "#;

        static TOML_BUILD_VOLUMES: &str = r#"
    [build.env]
    volumes = ["VOLUME3", "VOLUME4"]
    [target.aarch64-unknown-linux-gnu]
    xargo = false
    "#;

        static TOML_ARRAYS_BOTH: &str = r#"
    [build.env]
    passthrough = ["VAR1", "VAR2"]
    volumes = ["VOLUME1", "VOLUME2"]

    [target.aarch64-unknown-linux-gnu.env]
    passthrough = ["VAR3", "VAR4"]
    volumes = ["VOLUME3", "VOLUME4"]
    "#;

        static TOML_ARRAYS_BUILD: &str = r#"
    [build.env]
    passthrough = ["VAR1", "VAR2"]
    volumes = ["VOLUME1", "VOLUME2"]
    "#;

        static TOML_ARRAYS_TARGET: &str = r#"
    [target.aarch64-unknown-linux-gnu.env]
    passthrough = ["VAR3", "VAR4"]
    volumes = ["VOLUME3", "VOLUME4"]
    "#;

        static TOML_DEFAULT_TARGET: &str = r#"
    [build]
    default-target = "aarch64-unknown-linux-gnu"
    "#;
    }
}
