use crate::{Result, Target, Toml};

use std::{collections::HashMap, env::var};
// #[cfg(not(test))]
// #[derive(Debug)]
// struct Environment(&'static str);
#[derive(Debug)]
struct Environment(&'static str, Option<HashMap<&'static str, &'static str>>);

impl Environment {
    
    fn new() -> Self {
        Environment("CROSS", None)
    }

    #[cfg(test)]
    fn new_with(map: HashMap<&'static str, &'static str>) -> Self {
        Environment("CROSS", Some(map))
    }

    fn build_var_name(&self, name: &str) -> String {
        format!("{}_{}", self.0, name.to_ascii_uppercase().replace("-", "_"))
    }

    #[cfg(not(test))]
    fn get_var(&self, name: &str) -> Option<String> {
        var(name).ok().and_then(|v| Some(v))
    }
    #[cfg(test)]
    fn get_var(&self, name: &str) -> Option<String> {
        self.1.as_ref().and_then(|map| map.get(name).and_then(|v| Some(v.to_string())))
    }
    fn target_path(target: &Target, key: &str) -> String {
        format!("TARGET_{}_{}", target.triple(), key)
    }
    fn build_path(key: &str) -> String {
        format!("BUILD_{}", key)
    }

    fn get_build_var(&self, key: &str) -> Option<String> {
        self.get_var(&self.build_var_name(&Environment::build_path(key)))
    }

    fn get_target_var(&self, target: &Target, key: &str) -> Option<String> {
        self.get_var(&self.build_var_name(&Environment::target_path(target, key)))
    }

    fn xargo(&self, target: &Target) -> Result<(Option<bool>, Option<bool>)> {
        let (build_xargo, target_xargo) = (
            self.get_build_var("XARGO"),
            self.get_target_var(target, "XARGO"),
        );
        let build_env = if let Some(build) = build_xargo {
            Some(build.parse::<bool>().or_else(|_| {
                Err(format!(
                    "error parsing {} from XARGO environment variable",
                    build
                ))
            })?)
        } else {
            None
        };
        let target_env = if let Some(t) = target_xargo {
            Some(t.parse::<bool>().or_else(|_| {
                Err(format!(
                    "error parsing {} from XARGO environment variable",
                    t
                ))
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
        let target_envs = self.get_target_var(target, "env_passthrough");
        let mut passthrough_target = None;
        if let Some(envs) = target_envs {
            passthrough_target = Some(vec![]);
            passthrough_target
                .as_mut()
                .map(|vec| vec.extend(envs.split_whitespace().map(|v| v.to_string())));
        }
        let build_envs = self.get_build_var("env_passthrough");
        let mut passthrough_build = None;
        if let Some(envs) = build_envs {
            passthrough_build = Some(vec![]);
            passthrough_build
                .as_mut()
                .map(|vec| vec.extend(envs.split_whitespace().map(|v| v.to_string())));
        }

        (passthrough_build, passthrough_target)
    }

    fn volumes(&self, target: &Target) -> (Option<Vec<String>>, Option<Vec<String>>) {
        let target_envs = self.get_target_var(target, "ENV_VOLUMES");
        let mut volumes_target = None;
        if let Some(envs) = target_envs {
            volumes_target = Some(vec![]);
            volumes_target
                .as_mut()
                .map(|vec| vec.extend(envs.split_whitespace().map(|v| v.to_string())));
        }
        let build_envs = self.get_build_var("ENV_VOLUMES");
        let mut volumes_build = None;
        if let Some(envs) = build_envs {
            volumes_build = Some(vec![]);
            volumes_build
                .as_mut()
                .map(|vec| vec.extend(envs.split_whitespace().map(|v| v.to_string())));
        }
        // validate no =

        (volumes_build, volumes_target)
    }
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
            env: Environment::new(),
        }
    }
    
    #[cfg(test)]
    fn new_with(toml: Option<Toml>, env: Environment) -> Self {
        Config {
            toml,
            env,
        }
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
        let mut collect = vec![];
        let (build_env, target_env) = self.env.passthrough(target);
        if let Some(mut vars) = build_env {
            collect.extend(vars.drain(..));
        } else {
            if let Some(ref toml) = self.toml {
                collect.extend(
                    toml.env_passthrough_build()?
                        .drain(..)
                        .map(|v| v.to_string()),
                );
            }
        }
        if let Some(mut vars) = target_env {
            collect.extend(vars.drain(..));
        } else {
            if let Some(ref toml) = self.toml {
                collect.extend(
                    toml.env_passthrough_target(target)?
                        .drain(..)
                        .map(|v| v.to_string()),
                );
            }
        }
        println!("{:?}", collect);
        Ok(collect)
    }

    pub fn env_volumes(&self, target: &Target) -> Result<Vec<String>> {
        let mut collect = vec![];
        let (build_env, target_env) = self.env.volumes(target);
        if let Some(mut vars) = build_env {
            collect.extend(vars.drain(..));
        } else {
            if let Some(ref toml) = self.toml {
                collect.extend(toml.env_volumes_build()?.drain(..).map(|v| v.to_string()));
            }
        }
        if let Some(mut vars) = target_env {
            collect.extend(vars.drain(..));
        } else {
            if let Some(ref toml) = self.toml {
                collect.extend(
                    toml.env_volumes_target(target)?
                        .drain(..)
                        .map(|v| v.to_string()),
                );
            }
        }
        println!("env volumes {:?}", collect);
        Ok(collect)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Target, TargetList};
    use super::*;

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

            let env = Environment::new_with(map);

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

            let env = Environment::new_with(map);

            assert_eq!(env.xargo(&target())?, (Some(true), Some(false)));

            Ok(())
        }

        #[test]
        pub fn target_build_var_name() {
            let map = std::collections::HashMap::new();

            let env = Environment::new_with(map);
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

            let env = Environment::new_with(map);

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
        use std::env::{remove_var, set_var, var};
        use std::matches;

       
        fn toml(content: &str) -> Result<crate::Toml> {
            Ok(crate::Toml {
                table: if let Ok(toml::Value::Table(table)) = content.parse() {
                    table
                } else {
                    panic!("Could not parse toml");
                    // return crate::errors::Error("couldn't parse toml as TOML table");
                },
            })
        }

        #[test]
        pub fn env_and_toml_build_xargo_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_BUILD_XARGO", "true");

            let env = Environment::new_with(map);
            let config = Config::new_with(Some(toml(toml_build_xargo_false)?), env);
            assert!(matches!(config.xargo(&target()), Ok(Some(true))));

            Ok(())
        }
        #[test]
        pub fn env_target_and_toml_target_xargo_target_then_use_env() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "true");
            let env = Environment::new_with(map);
            
            
            let config = Config::new_with(Some(toml(toml_target_xargo_false)?), env);
            assert!(matches!(config.xargo(&target()), Ok(Some(true))));
            
            Ok(())
        }
        #[test]
        pub fn env_target_and_toml_build_xargo_then_use_toml() -> Result<()> {
            let mut map = HashMap::new();
            map.insert("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "true");
            let env = Environment::new_with(map);
            let config = Config::new_with(Some(toml(toml_build_xargo_false)?), env);
            assert!(matches!(config.xargo(&target()), Ok(Some(false))));

            Ok(())
        }

        static toml_build_xargo_false: &str = r#"
    [build]
    xargo = false
    "#;

        static toml_target_xargo_false: &str = r#"
    [target.aarch64-unknown-linux-gnu]
    xargo = false
    "#;
    }
}
