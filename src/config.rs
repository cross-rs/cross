use crate::{Result, Target, Toml};

use std::collections::HashMap;
use std::env;
#[derive(Debug)]
struct Environment(&'static str, Option<HashMap<&'static str, &'static str>>);

impl Environment {
    fn new(map: Option<HashMap<&'static str, &'static str>>) -> Self {
        Environment("CROSS", map)
    }

    fn build_var_name(&self, name: &str) -> String {
        format!("{}_{}", self.0, name.to_ascii_uppercase().replace("-", "_"))
    }

    fn get_var(&self, name: &str) -> Option<String> {
        self.1
            .as_ref()
            .map(|internal_map| internal_map.get(name).map(|v| v.to_string()))
            .flatten()
            .or_else(|| env::var(name).ok())
    }

    fn target_path(target: &Target, key: &str) -> String {
        format!("TARGET_{}_{}", target.triple(), key)
    }

    fn build_path(key: &str) -> String {
        format!("BUILD_{}", key)
    }

    fn get_build_var(&self, key: &str) -> Option<String> {
        self.get_var(&self.build_var_name(&Self::build_path(key)))
    }

    fn get_target_var(&self, target: &Target, key: &str) -> Option<String> {
        self.get_var(&self.build_var_name(&Self::target_path(target, key)))
    }

    fn xargo(&self, target: &Target) -> Result<(Option<bool>, Option<bool>)> {
        let (build_xargo, target_xargo) = (
            self.get_build_var("XARGO"),
            self.get_target_var(target, "XARGO"),
        );
        let build_env =
            if let Some(value) = build_xargo {
                Some(value.parse::<bool>().map_err(|_| {
                    format!("error parsing {} from XARGO environment variable", value)
                })?)
            } else {
                None
            };
        let target_env =
            if let Some(value) = target_xargo {
                Some(value.parse::<bool>().map_err(|_| {
                    format!("error parsing {} from XARGO environment variable", value)
                })?)
            } else {
                None
            };

        Ok((build_env, target_env))
    }

    fn image(&self, target: &Target) -> Option<String> {
        self.get_target_var(target, "IMAGE")
    }

    fn runner(&self, target: &Target) -> Option<String> {
        self.get_target_var(target, "RUNNER")
    }

    fn passthrough(&self, target: &Target) -> (Option<Vec<String>>, Option<Vec<String>>) {
        self.get_values_for("ENV_PASSTHROUGH", target)
    }

    fn volumes(&self, target: &Target) -> (Option<Vec<String>>, Option<Vec<String>>) {
        self.get_values_for("ENV_VOLUMES", target)
    }

    fn get_values_for(
        &self,
        var: &str,
        target: &Target,
    ) -> (Option<Vec<String>>, Option<Vec<String>>) {
        let target_values = self
            .get_target_var(target, var)
            .map(|ref s| split_to_cloned_by_ws(s));

        let build_values = self
            .get_build_var(var)
            .map(|ref s| split_to_cloned_by_ws(s));

        (build_values, target_values)
    }
}

fn split_to_cloned_by_ws(string: &str) -> Vec<String> {
    string.split_whitespace().map(String::from).collect()
}

#[derive(Debug)]
pub struct Config {
    toml: Option<Toml>,
    env: Environment,
}

impl Config {
    pub fn new(toml: Option<Toml>) -> Self {
        Config {
            toml,
            env: Environment::new(None),
        }
    }

    #[cfg(test)]
    fn new_with(toml: Option<Toml>, env: Environment) -> Self {
        Config { toml, env }
    }

    pub fn xargo(&self, target: &Target) -> Result<Option<bool>> {
        let (build_xargo, target_xargo) = self.env.xargo(target)?;
        let (toml_build_xargo, toml_target_xargo) = if let Some(ref toml) = self.toml {
            toml.xargo(target)?
        } else {
            (None, None)
        };

        match (build_xargo, toml_build_xargo) {
            (xargo @ Some(_), _) => return Ok(xargo),
            (None, xargo @ Some(_)) => return Ok(xargo),
            (None, None) => {}
        };

        match (target_xargo, toml_target_xargo) {
            (xargo @ Some(_), _) => return Ok(xargo),
            (None, xargo @ Some(_)) => return Ok(xargo),
            (None, None) => {}
        };
        Ok(None)
    }

    pub fn image(&self, target: &Target) -> Result<Option<String>> {
        let env_value = self.env.image(target);
        if let Some(env_value) = env_value {
            return Ok(Some(env_value));
        }
        self.toml.as_ref().map_or(Ok(None), |t| t.image(target))
    }

    pub fn runner(&self, target: &Target) -> Result<Option<String>> {
        let env_value = self.env.runner(target);
        if let Some(env_value) = env_value {
            return Ok(Some(env_value));
        }
        self.toml.as_ref().map_or(Ok(None), |t| t.runner(target))
    }

    pub fn env_passthrough(&self, target: &Target) -> Result<Vec<String>> {
        let (env_build, env_target) = self.env.passthrough(target);

        let toml_getter = || self.toml.as_ref().map(|t| t.env_passthrough_build());
        let mut collected = Self::sum_of_env_toml_values(toml_getter, env_build)?;

        let toml_getter = || self.toml.as_ref().map(|t| t.env_passthrough_target(target));
        collected.extend(Self::sum_of_env_toml_values(toml_getter, env_target)?);

        Ok(collected)
    }

    pub fn env_volumes(&self, target: &Target) -> Result<Vec<String>> {
        let (env_build, env_target) = self.env.volumes(target);
        let toml_getter = || self.toml.as_ref().map(|t| t.env_volumes_build());
        let mut collected = Self::sum_of_env_toml_values(toml_getter, env_build)?;

        let toml_getter = || self.toml.as_ref().map(|t| t.env_volumes_target(target));
        collected.extend(Self::sum_of_env_toml_values(toml_getter, env_target)?);

        Ok(collected)
    }

    fn sum_of_env_toml_values<'a>(
        toml_getter: impl FnOnce() -> Option<Result<Vec<&'a str>>>,
        env_values: Option<Vec<String>>,
    ) -> Result<Vec<String>> {
        let mut collect = vec![];
        if let Some(mut vars) = env_values {
            collect.extend(vars.drain(..));
        } else if let Some(toml_values) = toml_getter() {
            collect.extend(toml_values?.drain(..).map(|v| v.to_string()));
        }

        Ok(collect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Target, TargetList};

    fn target() -> Target {
        let target_list = TargetList {
            triples: vec!["aarch64-unknown-linux-gnu".to_string()],
        };

        Target::from("aarch64-unknown-linux-gnu", &target_list)
    }

    mod test_environment {

        use super::*;

        #[test]
        pub fn parse_error_in_env() {
            let mut map = std::collections::HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "tru");

            let env = Environment::new(Some(map));

            let res = env.xargo(&target());
            if let Ok(_) = res {
                panic!("invalid bool string parsing should fail");
            }
        }

        #[test]
        pub fn build_and_target_set_returns_tuple() -> Result<()> {
            let mut map = std::collections::HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "true");
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "false");

            let env = Environment::new(Some(map));

            assert_eq!(env.xargo(&target())?, (Some(true), Some(false)));

            Ok(())
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
            assert_eq!(build.as_ref().unwrap().contains(&"TEST1".to_string()), true);
            assert_eq!(build.as_ref().unwrap().contains(&"TEST2".to_string()), true);
            assert_eq!(
                target.as_ref().unwrap().contains(&"PASS1".to_string()),
                true
            );
            assert_eq!(
                target.as_ref().unwrap().contains(&"PASS2".to_string()),
                true
            );
        }
    }

    mod test_config {

        use super::*;
        use std::matches;

        fn toml(content: &str) -> Result<crate::Toml> {
            Ok(crate::Toml {
                table: if let Ok(toml::Value::Table(table)) = content.parse() {
                    table
                } else {
                    return Err("couldn't parse toml as TOML table".into());
                },
            })
        }

        #[test]
        pub fn env_and_toml_build_xargo_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "true");

            let env = Environment::new(Some(map));
            let config = Config::new_with(Some(toml(TOML_BUILD_XARGO_FALSE)?), env);
            assert!(matches!(config.xargo(&target()), Ok(Some(true))));

            Ok(())
        }

        #[test]
        pub fn env_target_and_toml_target_xargo_target_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "true");
            let env = Environment::new(Some(map));

            let config = Config::new_with(Some(toml(TOML_TARGET_XARGO_FALSE)?), env);
            assert!(matches!(config.xargo(&target()), Ok(Some(true))));

            Ok(())
        }

        #[test]
        pub fn env_target_and_toml_build_xargo_then_use_toml() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "true");
            let env = Environment::new(Some(map));
            let config = Config::new_with(Some(toml(TOML_BUILD_XARGO_FALSE)?), env);
            assert!(matches!(config.xargo(&target()), Ok(Some(false))));

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
    }
}
