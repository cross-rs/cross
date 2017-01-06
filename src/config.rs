use std::path::{Path, PathBuf};
use std::borrow::Cow;
use toml::{Parser, Value};

use Target;
use file;

use errors::*;

pub struct Toml {
    table: Value,
}

impl Toml {
    /// Returns the `target.{}.image` part of `Cross.toml`
    pub fn image(&self, target: &Target) -> Option<&str> {
        self.table
            .lookup(&format!("target.{}.image", target.triple()))
            .map(|val| val.as_str())
            .unwrap_or(None)
    }

    /// Returns the `target.{}.tag` part of `Cross.toml`
    pub fn tag(&self, target: &Target) -> Option<&str> {
        self.table
            .lookup(&format!("target.{}.tag", target.triple()))
            .map(|val| val.as_str())
            .unwrap_or(None)
    }
}

pub fn get_image(target: &Target, root: &PathBuf) -> Result<String> {
    let toml = utils::toml(root)?;
    if let Some(image) = toml.as_ref().and_then(|toml| toml.image(target)) {
        return Ok(image.into());
    }
    let version = env!("CARGO_PKG_VERSION");
    let tag_inter = toml.as_ref().and_then(|toml| toml.tag(target));
    let tag = if version.ends_with("-dev") && tag_inter.is_none() {
        Cow::from("latest")
    } else if tag_inter.is_none() {
        Cow::from(format!("v{}", version))
    } else {
        Cow::from(tag_inter.unwrap())
    };
    Ok(format!("japaric/{}:{}", target.triple(), tag))
}


pub mod utils {
    use super::*;
    /// Parses `path` as TOML
    pub fn parse(path: &Path) -> Result<Value> {
        Ok(Value::Table(Parser::new(&file::read(path)?).parse()
            .ok_or_else(|| format!("{} is not valid TOML", path.display()))?))
    }

    pub fn toml(root: &PathBuf) -> Result<Option<Toml>> {
        let p = root.join("Cross.toml");

        if p.exists() {
            parse(&p).map(|t| Some(Toml { table: t }))
        } else {
            Ok(None)
        }
    }
}
