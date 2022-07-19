use std::collections::HashMap;
use std::env;
use std::str::FromStr;

use crate::cargo_config::CargoConfig;
use crate::cross_config::CrossConfig;

pub fn bool_from_envvar(envvar: &str) -> bool {
    if let Ok(value) = bool::from_str(envvar) {
        value
    } else if let Ok(value) = i32::from_str(envvar) {
        value != 0
    } else {
        !envvar.is_empty()
    }
}

pub fn split_to_cloned_by_ws(string: &str) -> Vec<String> {
    string.split_whitespace().map(String::from).collect()
}

#[derive(Debug)]
pub struct Environment(&'static str, Option<HashMap<&'static str, &'static str>>);

impl Environment {
    pub fn new(name: &'static str, map: Option<HashMap<&'static str, &'static str>>) -> Self {
        Environment(name, map)
    }

    pub fn var_name(&self, name: &str) -> String {
        format!("{}_{}", self.0, name.to_ascii_uppercase().replace('-', "_"))
    }

    pub fn get_var(&self, name: &str) -> Option<String> {
        self.1
            .as_ref()
            .and_then(|internal_map| internal_map.get(name).map(|v| (*v).to_owned()))
            .or_else(|| env::var(name).ok())
    }
}

#[derive(Debug)]
pub struct Config {
    pub cargo: CargoConfig,
    pub cross: CrossConfig,
}

impl Config {
    pub fn new(cargo: CargoConfig, cross: CrossConfig) -> Config {
        Config { cargo, cross }
    }
}
