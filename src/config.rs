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
            .or_else(|| {
                self.get_build_var("IMAGE")
            })
    }
    fn runner(&mut self, target: &Target) -> Option<String> {
        self.get_target_var(target, "RUNNER")
        
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
}

mod test_config {

    use super::Environment;
    use crate::{Target, TargetList};

  
    #[test]
    pub fn parse_error_in_env() {
         let target_list = TargetList {
            triples: vec!["aarch64-unknown-linux-gnu".to_string()],
        };
        let target = Target::from("aarch64-unknown-linux-gnu", &target_list);
        let env = Environment::new("CROSS");
        std::env::set_var("CROSS_BUILD_XARGO", "tru");

        let res = env.xargo(&target);
        if let Ok(_) = res {
            panic!("invalid bool string parsing should fail");
        }


    }

    #[test]
    pub fn var_priority_target_before_build() -> Result<(), Box<dyn std::error::Error>> {
        let env = Environment::new("CROSS");
        std::env::set_var("CROSS_BUILD_XARGO", "true");
        std::env::set_var("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "false");

        // target used before build
        let target_list = TargetList {
            triples: vec!["aarch64-unknown-linux-gnu".to_string()],
        };
        let target = Target::from("aarch64-unknown-linux-gnu", &target_list);
        assert_eq!(env.xargo(&target)?, Some(false));

        // build used if no target
        std::env::remove_var("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO");
        assert_eq!(env.xargo(&target)?, Some(true));

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
    pub fn test() {
        let test = Some("test");
        let res = test.and_then(|s| None).or_else(|| Some(1));
        // println!("{:?}", res);
    }
}
