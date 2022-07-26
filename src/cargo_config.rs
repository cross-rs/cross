use std::collections::HashMap;

use crate::cargo_toml::CargoToml;
use crate::config::{split_to_cloned_by_ws, Environment};
use crate::errors::*;

pub const CARGO_NO_PREFIX_ENVVARS: &[&str] = &[
    "http_proxy",
    "TERM",
    "RUSTDOCFLAGS",
    "RUSTFLAGS",
    "BROWSER",
    "HTTPS_PROXY",
    "HTTP_TIMEOUT",
    "https_proxy",
];

#[derive(Debug)]
struct CargoEnvironment(Environment);

impl CargoEnvironment {
    fn new(map: Option<HashMap<&'static str, &'static str>>) -> Self {
        CargoEnvironment(Environment::new("CARGO", map))
    }

    pub fn alias(&self, name: &str) -> Option<Vec<String>> {
        let key = format!("ALIAS_{name}");
        self.0
            .get_var(&self.0.var_name(&key))
            .map(|x| split_to_cloned_by_ws(&x))
    }
}

#[derive(Debug)]
pub struct CargoConfig {
    toml: Option<CargoToml>,
    env: CargoEnvironment,
}

impl CargoConfig {
    pub fn new(toml: Option<CargoToml>) -> Self {
        CargoConfig {
            toml,
            env: CargoEnvironment::new(None),
        }
    }

    pub fn alias(&self, name: &str) -> Result<Option<Vec<String>>> {
        match self.env.alias(name) {
            Some(alias) => Ok(Some(alias)),
            None => match self.toml.as_ref() {
                Some(t) => t.alias(name),
                None => Ok(None),
            },
        }
    }

    pub fn to_toml(&self) -> Result<Option<String>> {
        match self.toml.as_ref() {
            Some(t) => Ok(Some(t.to_toml()?)),
            None => Ok(None),
        }
    }
}
