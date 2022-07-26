use std::collections::BTreeSet;
use std::env;
use std::path::Path;

use crate::config::split_to_cloned_by_ws;
use crate::errors::*;
use crate::file;

type Table = toml::value::Table;
type Value = toml::value::Value;

// the strategy is to merge, with arrays merging together
// and the deeper the config file is, the higher its priority.
// arrays merge, numbers/strings get replaced, objects merge in.
// we don't want to make any assumptions about the cargo
// config data, in case we need to use it later.
#[derive(Debug, Clone, Default)]
pub struct CargoToml(Table);

impl CargoToml {
    fn parse(path: &Path) -> Result<CargoToml> {
        let contents = file::read(&path)
            .wrap_err_with(|| format!("could not read cargo config file at `{path:?}`"))?;
        Ok(CargoToml(toml::from_str(&contents)?))
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string(&self.0).map_err(Into::into)
    }

    // finding cargo config files actually runs from the
    // current working directory the command is invoked,
    // not from the project root. same is true with work
    // spaces: the project layout does not matter.
    pub fn read() -> Result<Option<CargoToml>> {
        // note: cargo supports both `config` and `config.toml`
        // `config` exists for compatibility reasons, but if
        // present, only it will be read.
        let read = |dir: &Path| -> Result<Option<CargoToml>> {
            let noext = dir.join("config");
            let ext = dir.join("config.toml");
            if noext.exists() {
                Ok(Some(CargoToml::parse(&noext)?))
            } else if ext.exists() {
                Ok(Some(CargoToml::parse(&ext)?))
            } else {
                Ok(None)
            }
        };

        let read_and_merge = |result: &mut Option<CargoToml>, dir: &Path| -> Result<()> {
            let parent = read(dir)?;
            // can't use a match, since there's a use-after-move issue
            match (result.as_mut(), parent) {
                (Some(r), Some(p)) => r.merge(&p)?,
                (None, Some(p)) => *result = Some(p),
                (Some(_), None) | (None, None) => (),
            }

            Ok(())
        };

        let mut result = None;
        let cwd = env::current_dir()?;
        let mut dir: &Path = &cwd;
        loop {
            read_and_merge(&mut result, &dir.join(".cargo"))?;
            let parent_dir = dir.parent();
            match parent_dir {
                Some(path) => dir = path,
                None => break,
            }
        }

        read_and_merge(&mut result, &home::cargo_home()?)?;

        Ok(result)
    }

    fn merge(&mut self, parent: &CargoToml) -> Result<()> {
        // can error on mismatched-data

        fn validate_types(x: &Value, y: &Value) -> Option<()> {
            match x.same_type(y) {
                true => Some(()),
                false => None,
            }
        }

        // merge 2 tables. x has precedence over y.
        fn merge_tables(x: &mut Table, y: &Table) -> Option<()> {
            // we need to iterate over both keys, so we need a full deduplication
            let keys: BTreeSet<String> = x.keys().chain(y.keys()).cloned().collect();
            for key in keys {
                let in_x = x.contains_key(&key);
                let in_y = y.contains_key(&key);
                match (in_x, in_y) {
                    (true, true) => {
                        // need to do our merge strategy
                        let xk = x.get_mut(&key)?;
                        let yk = y.get(&key)?;
                        validate_types(xk, yk)?;

                        // now we've filtered out missing keys and optional values
                        // all key/value pairs should be same type.
                        if xk.is_table() {
                            merge_tables(xk.as_table_mut()?, yk.as_table()?)?;
                        } else if xk.is_array() {
                            xk.as_array_mut()?.extend_from_slice(yk.as_array()?);
                        }
                    }
                    (false, true) => {
                        // key in y is not in x: copy it over
                        let yk = y[&key].clone();
                        x.insert(key, yk);
                    }
                    // key isn't present in y: can ignore it
                    (_, false) => (),
                }
            }

            Some(())
        }

        merge_tables(&mut self.0, &parent.0).ok_or_else(|| eyre::eyre!("could not merge"))
    }

    pub fn alias(&self, name: &str) -> Result<Option<Vec<String>>> {
        let parse_alias = |value: &Value| -> Result<Vec<String>> {
            if let Some(s) = value.as_str() {
                Ok(split_to_cloned_by_ws(s))
            } else if let Some(a) = value.as_array() {
                a.iter()
                    .map(|i| {
                        i.as_str()
                            .map(ToOwned::to_owned)
                            .ok_or_else(|| eyre::eyre!("invalid alias type, got {value}"))
                    })
                    .collect()
            } else {
                eyre::bail!("invalid alias type, got {}", value.type_str());
            }
        };

        let alias = match self.0.get("alias") {
            Some(a) => a,
            None => return Ok(None),
        };
        let table = match alias.as_table() {
            Some(t) => t,
            None => eyre::bail!("cargo config aliases must be a table"),
        };

        match table.get(name) {
            Some(v) => Ok(Some(parse_alias(v)?)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! s {
        ($s:literal) => {
            $s.to_owned()
        };
    }

    #[test]
    fn test_parse() -> Result<()> {
        let config1 = CargoToml(toml::from_str(CARGO_TOML1)?);
        let config2 = CargoToml(toml::from_str(CARGO_TOML2)?);
        assert_eq!(config1.alias("foo")?, Some(vec![s!("build"), s!("foo")]));
        assert_eq!(config1.alias("bar")?, Some(vec![s!("check"), s!("bar")]));
        assert_eq!(config2.alias("baz")?, Some(vec![s!("test"), s!("baz")]));
        assert_eq!(config2.alias("bar")?, Some(vec![s!("init"), s!("bar")]));
        assert_eq!(config1.alias("far")?, None);
        assert_eq!(config2.alias("far")?, None);

        let mut merged = config1;
        merged.merge(&config2)?;
        assert_eq!(merged.alias("foo")?, Some(vec![s!("build"), s!("foo")]));
        assert_eq!(merged.alias("baz")?, Some(vec![s!("test"), s!("baz")]));
        assert_eq!(merged.alias("bar")?, Some(vec![s!("check"), s!("bar")]));

        // check our merge went well, with arrays, etc.
        assert_eq!(
            merged
                .0
                .get("build")
                .and_then(|x| x.get("jobs"))
                .and_then(|x| x.as_integer()),
            Some(2),
        );
        assert_eq!(
            merged
                .0
                .get("build")
                .and_then(|x| x.get("rustflags"))
                .and_then(|x| x.as_array())
                .and_then(|x| x.iter().map(|i| i.as_str()).collect()),
            Some(vec!["-C lto", "-Zbuild-std", "-Zdoctest-xcompile"]),
        );

        Ok(())
    }

    #[test]
    fn test_read() -> Result<()> {
        let config = CargoToml::read()?.expect("cross must have cargo config.");
        assert_eq!(
            config.alias("build-docker-image")?,
            Some(vec![s!("xtask"), s!("build-docker-image")])
        );
        assert_eq!(
            config.alias("xtask")?,
            Some(vec![s!("run"), s!("-p"), s!("xtask"), s!("--")])
        );

        Ok(())
    }

    const CARGO_TOML1: &str = r#"
[alias]
foo = "build foo"
bar = "check bar"

[build]
jobs = 2
rustc-wrapper = "sccache"
target = "x86_64-unknown-linux-gnu"
rustflags = ["-C lto", "-Zbuild-std"]
incremental = true

[doc]
browser = "firefox"

[env]
VAR1 = "VAL1"
VAR2 = { value = "VAL2", force = true }
VAR3 = { value = "relative/path", relative = true }
"#;

    const CARGO_TOML2: &str = r#"
# want to check tables merge
# want to check arrays concat
# want to check rest override
[alias]
baz = "test baz"
bar = "init bar"

[build]
jobs = 4
rustc-wrapper = "sccache"
target = "x86_64-unknown-linux-gnu"
rustflags = ["-Zdoctest-xcompile"]
incremental = true

[doc]
browser = "chromium"

[env]
VAR1 = "NEW1"
VAR2 = { value = "VAL2", force = false }
"#;
}
