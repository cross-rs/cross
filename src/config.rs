use crate::{Result, Target, Toml};

use std::collections::HashMap;
use std::env::var;
#[derive(Debug)]
struct Environment {
    prefix: &'static str,
    cache: HashMap<String, String>,
}

impl Environment {
    fn new(prefix: &'static str) -> Self {
        Environment {
            prefix,
            cache: HashMap::new(),
        }
    }

    fn build_var_name(&self, path: &str) -> String {
        format!(
            "{}_{}",
            self.prefix,
            path.to_ascii_uppercase().replace("-", "_")
        )
    }

    fn get_var(name: &str) -> Option<String> {
        var(name).map_or(None, |v| Some(v))
    }
    fn target_path(target: &Target, key: &str) -> String {
        format!("TARGET_{}_{}", target.triple(), key)
    }
    fn build_path(key: &str) -> String {
        format!("BUILD_{}", key)
    }

    fn get_build_var(&self, key: &str) -> Option<String> {
        Environment::get_var(&self.build_var_name(&Environment::build_path(key)))
    }

    fn get_target_var(&self, target: &Target, key: &str) -> Option<String> {
        Environment::get_var(&self.build_var_name(&Environment::target_path(target, key)))
    }

    fn xargo(&self, target: &Target) -> Result<Option<bool>> {
        let value =
            self.get_target_var(target, "XARGO")
                .or_else(|| {
                    self.get_build_var("XARGO")
                });

        if let Some(value) = value {
            Ok(Some(
                value
                    .parse::<bool>()
                    .or_else(|_| Err(format!("error parsing {} from XARGO environment variable", value)))?,
            ))
        } else {
            Ok(None)
        }
    }
    fn image(&mut self, target: &Target) -> Option<String> {
        self.get_target_var(target, "IMAGE")
    }
    fn runner(&mut self, target: &Target) -> Option<String> {
        self.get_target_var(target, "RUNNER")
        
    }

    fn passthrough(&self, target: &Target) -> (Option<Vec<String>>, Option<Vec<String>>) {
        let target_envs = self.get_target_var(target, "env_passthrough");
        let mut passthrough_target = None;
        if let Some(envs) = target_envs {
            passthrough_target = Some(vec![]);
            passthrough_target.as_mut().map(|vec| vec.extend(envs.split_whitespace().map(|v| v.to_string())));
        }
        let build_envs = self.get_build_var("env_passthrough");
        let mut passthrough_build = None;
        if let Some(envs) = build_envs {
            passthrough_build = Some(vec![]);
            passthrough_build.as_mut().map(|vec| vec.extend(envs.split_whitespace().map(|v| v.to_string())));

        }
        // validate no =

        (passthrough_build, passthrough_target)
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
            env: Environment::new("CROSS"),
        }
    }
    pub fn xargo(&mut self, target: &Target) -> Result<Option<bool>> {

        if let v @ Some(_) = self.env.xargo(target)? {
            return Ok(v);
        }

        if let Some(ref toml) = self.toml {
            toml.xargo(target)
        } else {
            Ok(None)
        }
    }
    pub fn image(&mut self, target: &Target) -> Result<Option<String>> {
        let env_value = self.env.image(target);
        if let Some(env_value) = env_value {
            return Ok(Some(env_value));
        }
        self.toml.as_ref().map_or(Ok(None), |t| t.image(target))
    }
    pub fn runner(&mut self, target: &Target) -> Result<Option<String>> {
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
                collect.extend(toml.env_passthrough_build()?.drain(..).map(|v| v.to_string()));
            }
        }
        if let Some(mut vars) = target_env {
            collect.extend(vars.drain(..));

        } else {
            if let Some(ref toml) = self.toml {
                collect.extend(toml.env_passthrough_target(target)?.drain(..).map(|v| v.to_string()));
            }
        }
        println!("{:?}", collect);
        Ok(collect)

    }

    
}

#[cfg(test)]
mod test_environment {

    use super::Environment;
    use crate::{Target, TargetList};
    use std::env::{set_var, remove_var};

  
    #[test]
    pub fn parse_error_in_env() {
         let target_list = TargetList {
            triples: vec!["aarch64-unknown-linux-gnu".to_string()],
        };
        let target = Target::from("aarch64-unknown-linux-gnu", &target_list);
        let env = Environment::new("CROSS");
        set_var("CROSS_BUILD_XARGO", "tru");

        let res = env.xargo(&target);
        if let Ok(_) = res {
            panic!("invalid bool string parsing should fail");
        }


    }

    #[test]
    pub fn var_priority_target_before_build() -> Result<(), Box<dyn std::error::Error>> {
        let env = Environment::new("CROSS");
        set_var("CROSS_BUILD_XARGO", "true");
        set_var("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "false");

        let target_list = TargetList {
            triples: vec!["aarch64-unknown-linux-gnu".to_string()],
        };
        let target = Target::from("aarch64-unknown-linux-gnu", &target_list);
        assert_eq!(env.xargo(&target)?, Some(false));

        // build used if no target
        remove_var("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO");
        assert_eq!(env.xargo(&target)?, Some(true));

        remove_var("CROSS_BUILD_XARGO");
        Ok(())
    }

    #[test]
    pub fn target_build_var_name() {
        let env = Environment::new("CROSS");
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
        let target_list = TargetList {
            triples: vec!["aarch64-unknown-linux-gnu".to_string()],
        };
        let target = Target::from("aarch64-unknown-linux-gnu", &target_list);

        set_var("CROSS_BUILD_ENV_PASSTHROUGH", "TEST1 TEST2");
        set_var("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_ENV_PASSTHROUGH", "PASS1 PASS2");
        let env = Environment::new("CROSS");
       
        let (build, target) = env.passthrough(&target);
        println!("{:?}, {:?}", build, target);
        assert_eq!(build.as_ref().unwrap().contains(&"TEST1".to_string()), true);
        assert_eq!(build.as_ref().unwrap().contains(&"TEST2".to_string()), true);
        assert_eq!(target.as_ref().unwrap().contains(&"PASS1".to_string()), true);
        assert_eq!(target.as_ref().unwrap().contains(&"PASS2".to_string()), true);

    }
}
