use std::fs;
use std::path::Path;
use std::str::FromStr;

use eyre::Context;

use crate::errors::Result;
use crate::file::PathExt;
use crate::shell::MessageInfo;

use super::engine::EngineType;

#[derive(Debug, Clone)]
pub struct DockerIgnoreRule {
    pub pattern: glob::Pattern,
    pub is_exception: bool,
    pub only_dir: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DockerIgnore {
    pub rules: Vec<DockerIgnoreRule>,
}

impl DockerIgnore {
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn from_dir(
        dir: &Path,
        engine_kind: EngineType,
        msg_info: &mut MessageInfo,
    ) -> Result<Self> {
        let containerignore = dir.join(".containerignore");
        let dockerignore = dir.join(".dockerignore");
        let (ignore_file, is_containerignore) =
            if engine_kind.is_podman() && containerignore.is_file() {
                (containerignore, true)
            } else if dockerignore.is_file() {
                (dockerignore, false)
            } else {
                (containerignore, true)
            };
        if ignore_file.is_file() {
            if engine_kind.is_podman() && !is_containerignore {
                msg_info.warn(format_args!(
                    "using `.dockerignore` with podman; consider renaming it to `.containerignore`"
                ))?;
            } else if !engine_kind.is_podman() && is_containerignore {
                msg_info.warn(format_args!(
                    "using `.containerignore` with docker; consider renaming it to `.dockerignore`"
                ))?;
            }
            Self::from_path(&ignore_file)
        } else {
            Ok(Self::empty())
        }
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .wrap_err_with(|| format!("when reading dockerignore file {path:?}"))?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self> {
        let content = content.strip_prefix('\u{feff}').unwrap_or(content);
        let mut rules = Vec::new();

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let (is_exception, pattern_str) = if let Some(rest) = trimmed.strip_prefix('!') {
                (true, rest.trim())
            } else {
                (false, trimmed)
            };

            if pattern_str.is_empty() {
                continue;
            }

            let mut pat = pattern_str.replace('\\', "/");

            while let Some(rest) = pat.strip_prefix("./") {
                pat = rest.to_owned();
            }

            while let Some(rest) = pat.strip_prefix('/') {
                pat = rest.to_owned();
            }

            while pat.contains("//") {
                pat = pat.replace("//", "/");
            }

            if pat.is_empty() {
                continue;
            }

            let only_dir = pat.ends_with('/');
            while pat.ends_with('/') && pat.len() > 1 {
                pat.pop();
            }

            if pat == "/" || pat.is_empty() {
                continue;
            }

            let compiled = glob::Pattern::new(&pat).wrap_err_with(|| {
                format!(
                    "invalid pattern on line {} in .dockerignore: {:?}",
                    line_idx + 1,
                    pattern_str
                )
            })?;

            rules.push(DockerIgnoreRule {
                pattern: compiled,
                is_exception,
                only_dir,
            });
        }

        Ok(Self { rules })
    }

    pub fn is_ignored(&self, path: &str, is_dir: bool) -> bool {
        if self.rules.is_empty() {
            return false;
        }

        let path = path.replace('\\', "/");
        let path = path.trim_matches('/');
        if path.is_empty() || path == "." {
            return false;
        }

        let mut parent_dirs = Vec::new();
        let mut current = path;
        while let Some((parent, _)) = current.rsplit_once('/') {
            if !parent.is_empty() {
                parent_dirs.push(parent);
                current = parent;
            } else {
                break;
            }
        }

        let match_opts = glob::MatchOptions {
            case_sensitive: true,
            require_literal_separator: true,
            require_literal_leading_dot: false,
        };

        let mut matched = false;

        for rule in &self.rules {
            if rule.is_exception != matched {
                continue;
            }

            let mut is_match = false;

            if (!rule.only_dir || is_dir) && rule.pattern.matches_with(path, match_opts) {
                is_match = true;
            }

            if !is_match {
                for parent in &parent_dirs {
                    if rule.pattern.matches_with(parent, match_opts) {
                        is_match = true;
                        break;
                    }
                }
            }

            if is_match {
                matched = !rule.is_exception;
            }
        }

        matched
    }

    pub fn is_dir_ignored(&self, dir_path: &str) -> bool {
        if !self.is_ignored(dir_path, true) {
            return false;
        }

        let dir_path = dir_path.replace('\\', "/");
        let dir_path = dir_path.trim_matches('/');
        let dir_parts: Vec<&str> = dir_path.split('/').filter(|s| !s.is_empty()).collect();
        if dir_parts.is_empty() {
            return false;
        }

        let has_child_exception = self.rules.iter().any(|rule| {
            if !rule.is_exception {
                return false;
            }

            let pat_str = rule.pattern.as_str();
            let pat_parts: Vec<&str> = pat_str.split('/').filter(|s| !s.is_empty()).collect();

            let mut could_match_child = true;
            for (i, dir_part) in dir_parts.iter().enumerate() {
                if i >= pat_parts.len() {
                    could_match_child = false;
                    break;
                }
                if pat_parts[i] == "**" {
                    could_match_child = true;
                    break;
                }
                let matches = glob::Pattern::new(pat_parts[i])
                    .map(|p| p.matches(dir_part))
                    .unwrap_or(false);
                if !matches {
                    could_match_child = false;
                    break;
                }
            }

            if could_match_child {
                pat_parts.len() > dir_parts.len() || pat_parts.contains(&"**")
            } else {
                false
            }
        });

        !has_child_exception
    }

    pub fn is_path_ignored(&self, path: &Path, is_dir: bool) -> Result<bool> {
        let posix = path.as_posix_relative()?;
        Ok(self.is_ignored(&posix, is_dir))
    }

    pub fn is_path_dir_ignored(&self, path: &Path) -> Result<bool> {
        let posix = path.as_posix_relative()?;
        Ok(self.is_dir_ignored(&posix))
    }
}

impl FromStr for DockerIgnore {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dockerignore_parsing() {
        let content = concat!(
            "\u{feff}",
            r#"# Comment line

  # Indented comment
  target/  
/src//*.rs
!/src/main.rs
./build/
"#
        );
        let di = DockerIgnore::parse(content).unwrap();
        assert_eq!(di.rules.len(), 4);

        assert_eq!(di.rules[0].pattern.as_str(), "target");
        assert!(di.rules[0].only_dir);
        assert!(!di.rules[0].is_exception);

        assert_eq!(di.rules[1].pattern.as_str(), "src/*.rs");
        assert!(!di.rules[1].only_dir);
        assert!(!di.rules[1].is_exception);

        assert_eq!(di.rules[2].pattern.as_str(), "src/main.rs");
        assert!(!di.rules[2].only_dir);
        assert!(di.rules[2].is_exception);

        assert_eq!(di.rules[3].pattern.as_str(), "build");
        assert!(di.rules[3].only_dir);
        assert!(!di.rules[3].is_exception);
    }

    #[test]
    fn test_dockerignore_matching_basic() {
        let content = r#"target
*.md
**/*.log
*/temp*
temp?
"#;
        let di = DockerIgnore::parse(content).unwrap();

        // target matches at root
        assert!(di.is_ignored("target", true));
        assert!(di.is_ignored("target", false));
        assert!(di.is_ignored("target/debug/app", false));
        assert!(di.is_ignored("target/debug", true));
        // target does not match in subdirectories (literal separator)
        assert!(!di.is_ignored("src/target", true));
        assert!(!di.is_ignored("src/target/foo", false));

        // *.md only matches at root
        assert!(di.is_ignored("README.md", false));
        assert!(!di.is_ignored("docs/README.md", false));

        // **/*.log matches at any depth
        assert!(di.is_ignored("app.log", false));
        assert!(di.is_ignored("logs/app.log", false));
        assert!(di.is_ignored("a/b/c/app.log", false));

        // */temp* matches in immediate subdirectories
        assert!(!di.is_ignored("temp", false));
        assert!(di.is_ignored("sub/temp", false));
        assert!(di.is_ignored("sub/temporary.txt", false));
        assert!(!di.is_ignored("a/b/temp", false));

        // temp? matches 1 char extension at root
        assert!(di.is_ignored("tempa", false));
        assert!(di.is_ignored("temp1", false));
        assert!(!di.is_ignored("temp", false));
        assert!(!di.is_ignored("temporary", false));
    }

    #[test]
    fn test_dockerignore_dir_only() {
        let content = r#"logs/
"#;
        let di = DockerIgnore::parse(content).unwrap();

        // logs/ should match directory, but not file named logs
        assert!(!di.is_ignored("logs", false));
        assert!(di.is_ignored("logs", true));
        // children of logs directory are inside an excluded directory
        assert!(di.is_ignored("logs/app.log", false));
        assert!(di.is_ignored("logs/sub/app.log", false));
    }

    #[test]
    fn test_dockerignore_exceptions() {
        let content = r#"*.md
!README*.md
README-secret.md
"#;
        let di = DockerIgnore::parse(content).unwrap();

        assert!(!di.is_ignored("README.md", false));
        assert!(!di.is_ignored("README-test.md", false));
        assert!(di.is_ignored("README-secret.md", false));
        assert!(di.is_ignored("other.md", false));
        assert!(!di.is_ignored("other.txt", false));
    }

    #[test]
    fn test_dockerignore_nested_exceptions() {
        // Moby test case
        let content = r#"**
!util/docker/web
"#;
        let di = DockerIgnore::parse(content).unwrap();
        assert!(!di.is_ignored("util/docker/web/foo", false));

        let content2 = r#"**
!util/docker/web
util/docker/web/foo
"#;
        let di2 = DockerIgnore::parse(content2).unwrap();
        assert!(di2.is_ignored("util/docker/web/foo", false));

        // Directory recursion exception
        let content3 = r#"target/**
!target/keep.txt
"#;
        let di3 = DockerIgnore::parse(content3).unwrap();
        assert!(!di3.is_ignored("target", true));
        assert!(!di3.is_ignored("target/keep.txt", false));
        assert!(di3.is_ignored("target/delete.txt", false));
    }

    #[test]
    fn test_dockerignore_dir_exception() {
        let content = r#"target/
!target/keep.txt
logs/
!logs/**/*.log
build/
"#;
        let di = DockerIgnore::parse(content).unwrap();

        // target/ is ignored by default rule
        assert!(di.is_ignored("target", true));
        // but is_dir_ignored returns false because of child exception !target/keep.txt
        assert!(!di.is_dir_ignored("target"));
        assert!(!di.is_ignored("target/keep.txt", false));
        assert!(di.is_ignored("target/secret.txt", false));

        // logs/ has child exception !logs/**/*.log
        assert!(di.is_ignored("logs", true));
        assert!(!di.is_dir_ignored("logs"));
        assert!(!di.is_dir_ignored("logs/sub"));
        assert!(!di.is_ignored("logs/sub/app.log", false));
        assert!(di.is_ignored("logs/sub/app.txt", false));

        // build/ has NO child exception
        assert!(di.is_ignored("build", true));
        assert!(di.is_dir_ignored("build"));
        assert!(di.is_ignored("build/app", false));
    }
}
