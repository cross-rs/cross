use crate::{Toml, Target, Result};

use std::env::var;
use std::collections::HashMap;
#[derive(Debug)]
struct Environment{
    prefix: &'static str,
    cache: HashMap<String, String>,

}

impl Environment {


    fn new(prefix: &'static str) -> Self {
        Environment{
            prefix,
            cache: HashMap::new(),
        }
    }

    

    fn build_var_name(&self, name : &str, key: Option<&str>) -> String {
        let mut var_name = format!("{}_{}", self.prefix, name.to_ascii_uppercase().replace("-", "_"));
        if let Some(key) = key {
            var_name = format!("{}_{}", var_name, key.to_ascii_uppercase().replace("-", "_"));
        }
        var_name

    }

    fn get_var(name: &str) -> Option<(&str, String)> {
        var(name)
        .map_or(None, |v| Some((name, v)))
    } 

    fn xargo(&mut self, target: &Target) -> Option<&str> {
        let var_name_target = self.build_var_name(&format!("target_{}",target.triple()), Some("XARGO"));
        let var_name_build = self.build_var_name("build", Some("XARGO"));
        let value = Environment::get_var(&var_name_target)
        .or_else(|| Environment::get_var(&var_name_build));
       
        if let Some((name, value)) = value {
            self.cache.insert(name.to_string(), value);
            Some(self.cache.get(name).unwrap())
        }
        else {None}
    }
    fn image(&mut self, target: &Target) -> Option<&str> {
        let var_name_target = self.build_var_name(&format!("target_{}",target.triple()), Some("IMAGE"));
        let var_name_build = self.build_var_name("build", Some("IMAGE"));
        let value = Environment::get_var(&var_name_target)
        .or_else(|| Environment::get_var(&var_name_build));
       
        if let Some((name, value)) = value {
            self.cache.insert(name.to_string(), value);
            Some(self.cache.get(name).unwrap())
        }
        else {None}


    }
    // Runner can now be defined in CROSS_BUILD_RUNNER as well
    fn runner(&mut self, target: &Target) -> Option<&str> {

        let var_name_target = self.build_var_name(&format!("target_{}",target.triple()), Some("RUNNER"));
        let var_name_build = self.build_var_name("build", Some("RUNNER"));
        let value = Environment::get_var(&var_name_target)
        .or_else(|| Environment::get_var(&var_name_build));
       
        if let Some((name, value)) = value {
            self.cache.insert(name.to_string(), value);
            Some(self.cache.get(name).unwrap())
        }
        else {None}

        
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
            env: Environment::new( "CROSS"),
        }
    }
    pub fn xargo(&mut self, target: &Target) -> Result<Option<bool>> {
        let env_value = self.env.xargo(target);
        if let Some(env_value) = env_value {
            return Ok(Some(env_value.parse().unwrap()));
        }
        self.toml.as_ref().map_or(Ok(None), |t| t.xargo(target))
    }
    pub fn image(&mut self, target: &Target) -> Result<Option<&str>> {
        let env_value = self.env.image(target);
        if let Some(env_value) = env_value {
            return Ok(Some(env_value));
        }
        self.toml.as_ref().map_or(Ok(None), |t| t.image(target))
    
        
    }
    pub fn runner(&mut self, target: &Target) -> Result<Option<&str>> {
        let env_value = self.env.runner(target);
        if let Some(env_value) = env_value {
            return Ok(Some(env_value));
        }
        self.toml.as_ref().map_or(Ok(None), |t| t.runner(target))


    }
   
   
   
}

mod test_config {

    use super::Environment;
    use crate::{TargetList, Target};

    #[test]
    pub fn var_not_set__none() {
        let target_list = TargetList {triples: vec!["aarch64-unknown-linux-gnu".to_string()]};
        let target = Target::from("aarch64-unknown-linux-gnu", &target_list);
        let env = Environment::new("CROSS");
    }

    #[test]
    pub fn var_priority_target_before_build() {
        let mut env = Environment::new("CROSS");
        std::env::set_var("CROSS_BUILD_XARGO", "true");
        std::env::set_var("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO", "false");

        // target used before build
        let target_list = TargetList {triples: vec!["aarch64-unknown-linux-gnu".to_string()]};
        let target = Target::from("aarch64-unknown-linux-gnu", &target_list);
        assert_eq!(env.xargo(&target), Some("false"));

        // build used if no target
        std::env::remove_var("CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO");
        assert_eq!(env.xargo(&target), Some("true"));

        

    }

    #[test]
    pub fn target_build_var_name() {
        let target = "aarch64-unknown-linux-gnu";
        let env = Environment::new("CROSS");
        assert_eq!(env.build_var_name("build_xargo", None), "CROSS_BUILD_XARGO");
        assert_eq!(env.build_var_name("target_aarch64-unknown-linux-gnu", Some("xargo")), "CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO");
        assert_eq!(env.build_var_name(target, Some("image")), "CROSS_AARCH64_UNKNOWN_LINUX_GNU_IMAGE")

    }
    #[test]
    pub fn test() {
        let test = Some("test");
        let res = test.and_then(|s| {None}).or_else(|| Some(1));
        println!("{:?}", res);

    }


}
