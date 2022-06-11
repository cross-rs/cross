use crate::{CrossToml, Result, Target, TargetList};

use std::collections::HashMap;
use std::env;
use std::str::FromStr;

#[derive(Debug)]
struct Environment(&'static str, Option<HashMap<&'static str, &'static str>>);

impl Environment {
    fn new(map: Option<HashMap<&'static str, &'static str>>) -> Self {
        Environment("CROSS", map)
    }

    fn build_var_name(&self, name: &str) -> String {
        format!("{}_{}", self.0, name.to_ascii_uppercase().replace('-', "_"))
    }

    fn get_var(&self, name: &str) -> Option<String> {
        self.1
            .as_ref()
            .and_then(|internal_map| internal_map.get(name).map(|v| v.to_string()))
            .or_else(|| env::var(name).ok())
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
        format!("BUILD_{key}")
    }

    fn get_build_var(&self, key: &str) -> Option<String> {
        self.get_var(&self.build_var_name(&Self::build_path(key)))
    }

    fn get_target_var(&self, target: &Target, key: &str) -> Option<String> {
        self.get_var(&self.build_var_name(&Self::target_path(target, key)))
    }

    fn xargo(&self, target: &Target) -> (Option<bool>, Option<bool>) {
        self.get_values_for("XARGO", target, bool_from_envvar)
    }

    fn image(&self, target: &Target) -> Option<String> {
        self.get_target_var(target, "IMAGE")
    }

    fn runner(&self, target: &Target) -> Option<String> {
        self.get_target_var(target, "RUNNER")
    }

    fn passthrough(&self, target: &Target) -> (Option<Vec<String>>, Option<Vec<String>>) {
        self.get_values_for("ENV_PASSTHROUGH", target, split_to_cloned_by_ws)
    }

    fn volumes(&self, target: &Target) -> (Option<Vec<String>>, Option<Vec<String>>) {
        self.get_values_for("ENV_VOLUMES", target, split_to_cloned_by_ws)
    }

    fn target(&self) -> Option<String> {
        self.get_build_var("TARGET")
    }
}

fn split_to_cloned_by_ws(string: &str) -> Vec<String> {
    string.split_whitespace().map(String::from).collect()
}

pub fn bool_from_envvar(envvar: &str) -> bool {
    if let Ok(value) = bool::from_str(envvar) {
        value
    } else if let Ok(value) = i32::from_str(envvar) {
        value != 0
    } else {
        !envvar.is_empty()
    }
}

#[derive(Debug)]
pub struct Config {
    toml: Option<CrossToml>,
    env: Environment,
}

impl Config {
    pub fn new(toml: Option<CrossToml>) -> Self {
        Config {
            toml,
            env: Environment::new(None),
        }
    }

    pub fn confusable_target(&self, target: &Target) {
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
                    eprintln!("Warning: a target named \"{mentioned_target}\" is mentioned in the Cross configuration, but the current specified target is \"{target}\".");
                    eprintln!(" > Is the target misspelled in the Cross configuration?");
                }
            }
        }
    }

    fn bool_from_config(
        &self,
        target: &Target,
        env: impl Fn(&Environment, &Target) -> (Option<bool>, Option<bool>),
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

    fn string_from_config(
        &self,
        target: &Target,
        env: impl Fn(&Environment, &Target) -> Option<String>,
        config: impl Fn(&CrossToml, &Target) -> Option<String>,
    ) -> Result<Option<String>> {
        let env_value = env(&self.env, target);
        if let Some(env_value) = env_value {
            return Ok(Some(env_value));
        }
        self.toml
            .as_ref()
            .map_or(Ok(None), |t| Ok(config(t, target)))
    }

    fn vec_from_config(
        &self,
        target: &Target,
        env: impl Fn(&Environment, &Target) -> (Option<Vec<String>>, Option<Vec<String>>),
        config_build: impl for<'a> Fn(&'a CrossToml) -> &'a [String],
        config_target: impl for<'a> Fn(&'a CrossToml, &Target) -> &'a [String],
    ) -> Result<Vec<String>> {
        let (env_build, env_target) = env(&self.env, target);

        let mut collected = self.sum_of_env_toml_values(env_build, config_build)?;
        collected.extend(self.sum_of_env_toml_values(env_target, |t| config_target(t, target))?);

        Ok(collected)
    }

    #[cfg(test)]
    fn new_with(toml: Option<CrossToml>, env: Environment) -> Self {
        Config { toml, env }
    }

    pub fn xargo(&self, target: &Target) -> Option<bool> {
        self.bool_from_config(target, Environment::xargo, CrossToml::xargo)
    }

    pub fn image(&self, target: &Target) -> Result<Option<String>> {
        self.string_from_config(target, Environment::image, CrossToml::image)
    }

    pub fn runner(&self, target: &Target) -> Result<Option<String>> {
        self.string_from_config(target, Environment::runner, CrossToml::runner)
    }

    pub fn env_passthrough(&self, target: &Target) -> Result<Vec<String>> {
        self.vec_from_config(
            target,
            Environment::passthrough,
            CrossToml::env_passthrough_build,
            CrossToml::env_passthrough_target,
        )
    }

    pub fn env_volumes(&self, target: &Target) -> Result<Vec<String>> {
        self.vec_from_config(
            target,
            Environment::volumes,
            CrossToml::env_volumes_build,
            CrossToml::env_volumes_target,
        )
    }

    pub fn target(&self, target_list: &TargetList) -> Option<Target> {
        if let Some(env_value) = self.env.target() {
            return Some(Target::from(&env_value, target_list));
        }
        self.toml
            .as_ref()
            .and_then(|t| t.default_target(target_list))
    }

    fn sum_of_env_toml_values<'a>(
        &'a self,
        env_values: Option<Vec<String>>,
        toml_getter: impl FnOnce(&'a CrossToml) -> &'a [String],
    ) -> Result<Vec<String>> {
        let mut collect = vec![];
        if let Some(mut vars) = env_values {
            collect.append(&mut vars);
        } else if let Some(toml_values) = self.toml.as_ref().map(toml_getter) {
            collect.extend(toml_values.iter().cloned());
        }

        Ok(collect)
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
                "aarch64-unknown-linux-gnu".to_string(),
                "armv7-unknown-linux-musleabihf".to_string(),
            ],
        }
    }

    fn target() -> Target {
        let target_list = target_list();
        Target::from("aarch64-unknown-linux-gnu", &target_list)
    }

    mod test_environment {

        use super::*;

        #[test]
        pub fn parse_error_in_env() {
            let mut map = std::collections::HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "tru");

            let env = Environment::new(Some(map));
            assert_eq!(env.xargo(&target()), (Some(true), None));
        }

        #[test]
        pub fn build_and_target_set_returns_tuple() {
            let mut map = std::collections::HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "true");
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "false");

            let env = Environment::new(Some(map));
            assert_eq!(env.xargo(&target()), (Some(true), Some(false)));
        }

        #[test]
        pub fn target_build_var_name() {
            let map = std::collections::HashMap::new();

            let env = Environment::new(Some(map));
            assert_eq!(env.build_var_name("build_xargo"), "CROSS_BUILD_XARGO");
            assert_eq!(
                env.build_var_name("target_aarch64-unknown-linux-gnu_XARGO"),
                "CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO"
            );
            assert_eq!(
                env.build_var_name("target-aarch64-unknown-linux-gnu_image"),
                "CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_IMAGE"
            )
        }

        #[test]
        pub fn collect_passthrough() {
            let mut map = std::collections::HashMap::new();
            map.insert("CROSS_BUILD_ENV_PASSTHROUGH", "TEST1 TEST2");
            map.insert(
                "CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_ENV_PASSTHROUGH",
                "PASS1 PASS2",
            );

            let env = Environment::new(Some(map));

            let (build, target) = env.passthrough(&target());
            assert!(build.as_ref().unwrap().contains(&"TEST1".to_string()));
            assert!(build.as_ref().unwrap().contains(&"TEST2".to_string()));
            assert!(target.as_ref().unwrap().contains(&"PASS1".to_string()));
            assert!(target.as_ref().unwrap().contains(&"PASS2".to_string()));
        }
    }

    #[cfg(test)]
    mod test_config {

        use super::*;
        use std::matches;

        fn toml(content: &str) -> Result<crate::CrossToml> {
            Ok(CrossToml::parse(content).wrap_err("couldn't parse toml")?.0)
        }

        #[test]
        pub fn env_and_toml_build_xargo_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "true");

            let env = Environment::new(Some(map));
            let config = Config::new_with(Some(toml(TOML_BUILD_XARGO_FALSE)?), env);
            assert!(matches!(config.xargo(&target()), Some(true)));

            Ok(())
        }

        #[test]
        pub fn env_target_and_toml_target_xargo_target_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "true");
            let env = Environment::new(Some(map));

            let config = Config::new_with(Some(toml(TOML_TARGET_XARGO_FALSE)?), env);
            assert!(matches!(config.xargo(&target()), Some(true)));

            Ok(())
        }

        #[test]
        pub fn env_target_and_toml_build_xargo_then_use_toml() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "true");
            let env = Environment::new(Some(map));
            let config = Config::new_with(Some(toml(TOML_BUILD_XARGO_FALSE)?), env);
            assert!(matches!(config.xargo(&target()), Some(true)));

            Ok(())
        }

        #[test]
        pub fn volumes_use_env_over_toml() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_BUILD_ENV_VOLUMES", "VOLUME1 VOLUME2");
            let env = Environment::new(Some(map));
            let config = Config::new_with(Some(toml(TOML_BUILD_VOLUMES)?), env);
            let expected = vec!["VOLUME1".to_string(), "VOLUME2".into()];

            let result = config.env_volumes(&target()).unwrap();
            assert!(result.len() == 2);
            assert!(result.contains(&expected[0]));
            assert!(result.contains(&expected[1]));

            Ok(())
        }

        #[test]
        pub fn volumes_use_toml_when_no_env() -> Result<()> {
            let map = HashMap::new();
            let env = Environment::new(Some(map));
            let config = Config::new_with(Some(toml(TOML_BUILD_VOLUMES)?), env);
            let expected = vec!["VOLUME3".to_string(), "VOLUME4".into()];

            let result = config.env_volumes(&target()).unwrap();
            assert!(result.len() == 2);
            assert!(result.contains(&expected[0]));
            assert!(result.contains(&expected[1]));

            Ok(())
        }

        #[test]
        pub fn no_env_and_no_toml_default_target_then_none() -> Result<()> {
            let config = Config::new_with(None, Environment::new(None));
            let config_target = config.target(&target_list());
            assert!(matches!(config_target, None));

            Ok(())
        }

        #[test]
        pub fn env_and_toml_default_target_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_BUILD_TARGET", "armv7-unknown-linux-musleabihf");
            let env = Environment::new(Some(map));
            let config = Config::new_with(Some(toml(TOML_DEFAULT_TARGET)?), env);

            let config_target = config.target(&target_list()).unwrap();
            assert!(matches!(
                config_target.triple(),
                "armv7-unknown-linux-musleabihf"
            ));

            Ok(())
        }

        #[test]
        pub fn no_env_but_toml_default_target_then_use_toml() -> Result<()> {
            let env = Environment::new(None);
            let config = Config::new_with(Some(toml(TOML_DEFAULT_TARGET)?), env);

            let config_target = config.target(&target_list()).unwrap();
            assert!(matches!(
                config_target.triple(),
                "aarch64-unknown-linux-gnu"
            ));

            Ok(())
        }

        static TOML_BUILD_XARGO_FALSE: &str = r#"
    [build]
    xargo = false
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

        static TOML_DEFAULT_TARGET: &str = r#"
    [build]
    default-target = "aarch64-unknown-linux-gnu"
    "#;
    }
}
