use std::cmp;
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

use crate::util::{project_dir, write_to_string};
use cross::shell::MessageInfo;
use cross::ToUtf8;

use chrono::{Datelike, Utc};
use clap::{Args, Subcommand};
use eyre::Context;
use serde::Deserialize;

pub fn changelog(args: Changelog, msg_info: &mut MessageInfo) -> cross::Result<()> {
    match args {
        Changelog::Build(args) => build_changelog(args, msg_info),
        Changelog::Validate(args) => validate_changelog(args, msg_info),
    }
}

#[derive(Subcommand, Debug)]
pub enum Changelog {
    /// Build the changelog.
    Build(BuildChangelog),
    /// Validate changelog entries.
    Validate(ValidateChangelog),
}

#[derive(Args, Debug)]
pub struct BuildChangelog {
    /// Build a release changelog.
    #[clap(long, env = "NEW_VERSION", required = true)]
    release: Option<String>,
    /// Whether we're doing a dry run or not.
    #[clap(long, env = "DRY_RUN")]
    dry_run: bool,
}

#[derive(Args, Debug)]
pub struct ValidateChangelog {
    /// List of changelog entries to validate.
    files: Vec<String>,
}

// the type for the identifier: if it's a PR, sort
// by the number, otherwise, sort as 0. the numbers
// should be sorted, and the `max(values) || 0` should
// be used
#[derive(Debug, Clone, PartialEq, Eq)]
enum IdType {
    PullRequest(Vec<u64>),
    Issue(Vec<u64>),
}

impl IdType {
    fn numbers(&self) -> &[u64] {
        match self {
            IdType::PullRequest(v) => v,
            IdType::Issue(v) => v,
        }
    }

    fn max_number(&self) -> u64 {
        self.numbers().iter().max().map_or_else(|| 0, |v| *v)
    }

    fn parse_stem(file_stem: &str) -> cross::Result<IdType> {
        let (is_issue, rest) = match file_stem.strip_prefix("issue") {
            Some(n) => (true, n),
            None => (false, file_stem),
        };
        let mut numbers = rest
            .split('-')
            .map(|x| x.parse::<u64>())
            .collect::<Result<Vec<u64>, _>>()?;
        numbers.sort_unstable();

        Ok(match is_issue {
            false => IdType::PullRequest(numbers),
            true => IdType::Issue(numbers),
        })
    }

    fn parse_changelog(prs: &str) -> cross::Result<IdType> {
        let mut numbers = prs
            .split(',')
            .map(|x| x.trim().parse::<u64>())
            .collect::<Result<Vec<u64>, _>>()?;
        numbers.sort_unstable();

        Ok(IdType::PullRequest(numbers))
    }
}

impl cmp::PartialOrd for IdType {
    fn partial_cmp(&self, other: &IdType) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for IdType {
    fn cmp(&self, other: &IdType) -> cmp::Ordering {
        self.max_number().cmp(&other.max_number())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ChangelogType {
    Added,
    Changed,
    Fixed,
    Removed,
    Internal,
}

impl ChangelogType {
    fn from_header(s: &str) -> cross::Result<Self> {
        Ok(match s {
            "Added" => Self::Added,
            "Changed" => Self::Changed,
            "Fixed" => Self::Fixed,
            "Removed" => Self::Removed,
            "Internal" => Self::Internal,
            _ => eyre::bail!("invalid header section, got {s}"),
        })
    }

    fn sort_by(&self) -> u32 {
        match self {
            ChangelogType::Added => 4,
            ChangelogType::Changed => 3,
            ChangelogType::Fixed => 2,
            ChangelogType::Removed => 1,
            ChangelogType::Internal => 0,
        }
    }
}

impl cmp::PartialOrd for ChangelogType {
    fn partial_cmp(&self, other: &ChangelogType) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for ChangelogType {
    fn cmp(&self, other: &ChangelogType) -> cmp::Ordering {
        self.sort_by().cmp(&other.sort_by())
    }
}

// internal type for a changelog, just containing the contents
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct ChangelogContents {
    description: String,
    #[serde(default)]
    issues: Vec<u64>,
    #[serde(default)]
    breaking: bool,
    #[serde(rename = "type")]
    kind: ChangelogType,
}

impl ChangelogContents {
    fn sort_by(&self) -> (&ChangelogType, &str, &bool) {
        (&self.kind, &self.description, &self.breaking)
    }
}

impl cmp::PartialOrd for ChangelogContents {
    fn partial_cmp(&self, other: &ChangelogContents) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for ChangelogContents {
    fn cmp(&self, other: &ChangelogContents) -> cmp::Ordering {
        self.sort_by().cmp(&other.sort_by())
    }
}

impl fmt::Display for ChangelogContents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.breaking {
            f.write_str("BREAKING: ")?;
        }
        f.write_str(&self.description)
    }
}

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
struct ChangelogEntry {
    id: IdType,
    contents: ChangelogContents,
}

impl ChangelogEntry {
    fn new(id: IdType, contents: ChangelogContents) -> Self {
        Self { id, contents }
    }

    fn parse(s: &str, kind: ChangelogType) -> cross::Result<Self> {
        let (id, rest) = match s.split_once('-') {
            Some((prefix, rest)) => match prefix.trim().strip_prefix('#') {
                Some(prs) => (IdType::parse_changelog(prs)?, rest),
                None => (IdType::Issue(vec![]), s),
            },
            None => (IdType::Issue(vec![]), s),
        };

        let trimmed = rest.trim();
        let (breaking, description) = match trimmed.strip_prefix("BREAKING: ") {
            Some(d) => (true, d.trim().to_owned()),
            None => (false, trimmed.to_owned()),
        };

        Ok(ChangelogEntry {
            id,
            contents: ChangelogContents {
                kind,
                breaking,
                description,
                issues: vec![],
            },
        })
    }

    fn from_object(id: IdType, value: serde_json::Value) -> cross::Result<Self> {
        Ok(Self::new(id, serde_json::value::from_value(value)?))
    }

    fn from_value(id: IdType, mut value: serde_json::Value) -> cross::Result<Vec<Self>> {
        let mut result = vec![];
        if value.is_array() {
            for item in value.as_array_mut().expect("must be array") {
                result.push(Self::from_object(id.clone(), item.take())?);
            }
        } else {
            result.push(Self::from_object(id, value)?);
        }

        Ok(result)
    }
}

impl fmt::Display for ChangelogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("-")?;
        match &self.id {
            IdType::PullRequest(prs) => f.write_fmt(format_args!(
                " #{} -",
                prs.iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>()
                    .join(",#")
            ))?,
            IdType::Issue(_) => (),
        }
        f.write_fmt(format_args!(" {}", self.contents))?;
        f.write_str("\n")
    }
}

// de-duplicate in place
fn deduplicate_entries(original: &mut Vec<ChangelogEntry>) {
    let mut result = Vec::with_capacity(original.len());
    let mut memo = BTreeSet::new();
    for item in original.iter() {
        if memo.insert(item.to_string()) {
            result.push(item.clone());
        }
    }

    *original = result;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Changes {
    added: Vec<ChangelogEntry>,
    changed: Vec<ChangelogEntry>,
    fixed: Vec<ChangelogEntry>,
    removed: Vec<ChangelogEntry>,
    internal: Vec<ChangelogEntry>,
}

impl Changes {
    fn sort_descending(&mut self) {
        self.added.sort_by(|x, y| y.cmp(x));
        self.changed.sort_by(|x, y| y.cmp(x));
        self.fixed.sort_by(|x, y| y.cmp(x));
        self.removed.sort_by(|x, y| y.cmp(x));
        self.internal.sort_by(|x, y| y.cmp(x));
    }

    fn deduplicate(&mut self) {
        deduplicate_entries(&mut self.added);
        deduplicate_entries(&mut self.changed);
        deduplicate_entries(&mut self.fixed);
        deduplicate_entries(&mut self.removed);
        deduplicate_entries(&mut self.internal);
    }

    fn merge(&mut self, other: &mut Self) {
        self.added.append(&mut other.added);
        self.changed.append(&mut other.changed);
        self.fixed.append(&mut other.fixed);
        self.removed.append(&mut other.removed);
        self.internal.append(&mut other.internal);
    }

    fn push(&mut self, entry: ChangelogEntry) {
        match entry.contents.kind {
            ChangelogType::Added => self.added.push(entry),
            ChangelogType::Changed => self.changed.push(entry),
            ChangelogType::Fixed => self.fixed.push(entry),
            ChangelogType::Removed => self.removed.push(entry),
            ChangelogType::Internal => self.internal.push(entry),
        }
    }
}

macro_rules! fmt_changelog_vec {
    ($self:ident, $fmt:ident, $field:ident, $header:literal) => {{
        if !$self.$field.is_empty() {
            $fmt.write_str(concat!("\n### ", $header, "\n\n"))?;
            for entry in &$self.$field {
                $fmt.write_fmt(format_args!("{}", entry))?;
            }
        }
    }};
}

impl fmt::Display for Changes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_changelog_vec!(self, f, added, "Added");
        fmt_changelog_vec!(self, f, changed, "Changed");
        fmt_changelog_vec!(self, f, fixed, "Fixed");
        fmt_changelog_vec!(self, f, removed, "Removed");
        fmt_changelog_vec!(self, f, internal, "Internal");

        Ok(())
    }
}

fn file_stem(path: &Path) -> cross::Result<&str> {
    path.file_stem()
        .ok_or(eyre::eyre!("unable to get file stem {path:?}"))?
        .to_utf8()
}

fn read_changes(changes_dir: &Path) -> cross::Result<Changes> {
    let mut changes = Changes::default();
    for entry in fs::read_dir(changes_dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let file_name = entry.file_name();
        let path = entry.path();
        let ext = path.extension();
        if file_type.is_file() && ext.map_or(false, |v| v == "json") {
            let stem = file_stem(&path)?;
            let id = IdType::parse_stem(stem)?;
            let contents = fs::read_to_string(path)?;
            let value = serde_json::from_str(&contents)
                .wrap_err_with(|| format!("unable to parse JSON for {file_name:?}"))?;
            let new_entries = ChangelogEntry::from_value(id, value)
                .wrap_err_with(|| format!("unable to extract changelog from {file_name:?}"))?;
            for change in new_entries {
                match change.contents.kind {
                    ChangelogType::Added => changes.added.push(change),
                    ChangelogType::Changed => changes.changed.push(change),
                    ChangelogType::Fixed => changes.fixed.push(change),
                    ChangelogType::Removed => changes.removed.push(change),
                    ChangelogType::Internal => changes.internal.push(change),
                }
            }
        }
    }

    Ok(changes)
}

fn read_changelog(root: &Path) -> cross::Result<(String, Changes, String)> {
    let lines: Vec<String> = fs::read_to_string(root.join("CHANGELOG.md"))?
        .lines()
        .map(ToOwned::to_owned)
        .collect();

    let next_index = lines
        .iter()
        .position(|x| x.trim().starts_with("## [Unreleased]"))
        .ok_or(eyre::eyre!("could not find unreleased section"))?;
    let (header, rest) = lines.split_at(next_index);

    // need to skip the first index since it's previously
    // matched, and then just increment our split by 1.
    let last_index = 1 + rest[1..]
        .iter()
        .position(|x| x.trim().starts_with("## "))
        .ok_or(eyre::eyre!("could not find the next release section"))?;
    let (section, footer) = rest.split_at(last_index);

    // the unreleased should have the format:
    //  ## [Unreleased] - ReleaseDate
    //
    //  ### Added
    //
    //  - #905 - ...
    let mut kind = None;
    let mut changes = Changes::default();
    for line in section {
        let line = line.trim();
        if let Some(header) = line.strip_prefix("### ") {
            kind = Some(ChangelogType::from_header(header)?);
        } else if let Some(entry) = line.strip_prefix("- ") {
            match kind {
                Some(kind) => changes.push(ChangelogEntry::parse(entry, kind)?),
                None => eyre::bail!("changelog entry \"{line}\" without header"),
            }
        } else if !(line.is_empty() || line == "## [Unreleased] - ReleaseDate") {
            eyre::bail!("invalid changelog entry, got \"{line}\"");
        }
    }

    Ok((header.join("\n"), changes, footer.join("\n")))
}

fn delete_changes(root: &Path) -> cross::Result<()> {
    // move all files to the denoted version release
    for entry in fs::read_dir(root.join(".changes"))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let srcpath = entry.path();
        let ext = srcpath.extension();
        if file_type.is_file() && ext.map_or(false, |v| v == "json") {
            fs::remove_file(srcpath)?;
        }
    }

    Ok(())
}

/// Get the date as a year/month/day tuple.
pub fn get_current_date() -> String {
    let utc = Utc::now();
    let date = utc.date_naive();

    format!("{}-{:0>2}-{}", date.year(), date.month(), date.day())
}

// used for internal testing
fn build_changelog_from_dir(
    root: &Path,
    changes_dir: &Path,
    release: Option<&str>,
) -> cross::Result<String> {
    use std::fmt::Write;

    let mut new = read_changes(changes_dir)?;
    let (header, mut existing, footer) = read_changelog(root)?;
    new.merge(&mut existing);
    new.deduplicate();
    new.sort_descending();

    let mut output = header;
    output.push_str("\n## [Unreleased] - ReleaseDate\n");
    if let Some(release) = release {
        let version = semver::Version::parse(release)?;
        if version.pre.is_empty() {
            let date = get_current_date();
            writeln!(&mut output, "\n## [v{release}] - {date}")?;
        }
    }
    output.push_str(&new.to_string());
    output.push('\n');
    output.push_str(&footer);

    Ok(output)
}

pub fn build_changelog(
    BuildChangelog {
        dry_run, release, ..
    }: BuildChangelog,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    msg_info.info("Building the changelog.")?;
    msg_info.debug(format_args!(
        "Running with dry-run set the {dry_run} and with release {release:?}"
    ))?;

    let root = project_dir(msg_info)?;
    let changes_dir = root.join(".changes");
    let output = build_changelog_from_dir(&root, &changes_dir, release.as_deref())?;

    let filename = match !dry_run && release.is_some() {
        true => {
            delete_changes(&root)?;
            "CHANGELOG.md"
        }
        false => "CHANGELOG.md.draft",
    };
    let path = root.join(filename);
    write_to_string(&path, &output)?;
    #[allow(clippy::disallowed_methods)]
    msg_info.info(format_args!("Changelog written to `{}`", path.display()))?;

    Ok(())
}

#[allow(clippy::disallowed_methods)]
pub fn validate_changelog(
    ValidateChangelog { mut files, .. }: ValidateChangelog,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let root = project_dir(msg_info)?;
    let changes_dir = root.join(".changes");
    if files.is_empty() {
        files = fs::read_dir(&changes_dir)?
            .filter_map(|x| x.ok())
            .filter(|x| x.file_type().map_or(false, |v| v.is_file()))
            .filter_map(|x| {
                if x.path()
                    .extension()
                    .and_then(|s: &std::ffi::OsStr| s.to_str())
                    .unwrap_or_default()
                    == "json"
                {
                    Some(x.file_name().to_utf8().unwrap().to_owned())
                } else {
                    None
                }
            })
            .collect();
    }
    let mut errors = vec![];
    for file in files {
        let file_name = Path::new(&file);
        let path = changes_dir.join(file_name);
        let stem = file_stem(&path)?;
        let contents = fs::read_to_string(&path)
            .wrap_err_with(|| eyre::eyre!("cannot find file {}", path.display()))?;

        let id = match IdType::parse_stem(stem)
            .wrap_err_with(|| format!("unable to parse file stem for \"{}\"", path.display()))
        {
            Ok(id) => id,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };

        let value = match serde_json::from_str(&contents)
            .wrap_err_with(|| format!("unable to parse JSON for \"{}\"", path.display()))
        {
            Ok(value) => value,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };

        let res = ChangelogEntry::from_value(id, value)
            .wrap_err_with(|| format!("unable to extract changelog from \"{}\"", path.display()))
            .map(|_| ());
        errors.extend(res.err());
    }

    if !errors.is_empty() {
        return Err(crate::util::with_section_reports(
            eyre::eyre!("some files were not validated"),
            errors,
        ));
    }
    // also need to validate the existing changelog
    let _ = read_changelog(&root)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! s {
        ($x:literal) => {
            $x.to_owned()
        };
    }

    #[test]
    fn test_id_type_parse_stem() -> cross::Result<()> {
        assert_eq!(IdType::parse_stem("645")?, IdType::PullRequest(vec![645]));
        assert_eq!(
            IdType::parse_stem("640-645")?,
            IdType::PullRequest(vec![640, 645])
        );
        assert_eq!(
            IdType::parse_stem("issue640-645")?,
            IdType::Issue(vec![640, 645])
        );

        Ok(())
    }

    #[test]
    fn test_id_type_parse_changelog() -> cross::Result<()> {
        assert_eq!(
            IdType::parse_changelog("645")?,
            IdType::PullRequest(vec![645])
        );
        assert_eq!(
            IdType::parse_changelog("640,645")?,
            IdType::PullRequest(vec![640, 645])
        );

        Ok(())
    }

    #[test]
    fn changelog_type_sort() {
        assert!(ChangelogType::Added > ChangelogType::Changed);
        assert!(ChangelogType::Changed > ChangelogType::Fixed);
    }

    #[test]
    fn change_log_type_from_header() -> cross::Result<()> {
        assert_eq!(ChangelogType::from_header("Added")?, ChangelogType::Added);

        Ok(())
    }

    #[test]
    fn changelog_contents_deserialize() -> cross::Result<()> {
        let actual: ChangelogContents = serde_json::from_str(CHANGES_OBJECT)?;
        let expected = ChangelogContents {
            description: s!("sample description for a PR adding one CHANGELOG entry."),
            issues: vec![437],
            breaking: false,
            kind: ChangelogType::Fixed,
        };
        assert_eq!(actual, expected);

        let actual: Vec<ChangelogContents> = serde_json::from_str(CHANGES_ARRAY)?;
        let expected = vec![
            ChangelogContents {
                description: s!("this is one added entry."),
                issues: vec![630],
                breaking: false,
                kind: ChangelogType::Added,
            },
            ChangelogContents {
                description: s!("this is another added entry."),
                issues: vec![642],
                breaking: false,
                kind: ChangelogType::Added,
            },
            ChangelogContents {
                description: s!("this is a fixed entry that has no attached issue."),
                issues: vec![],
                breaking: false,
                kind: ChangelogType::Fixed,
            },
            ChangelogContents {
                description: s!("this is a breaking change."),
                issues: vec![679],
                breaking: true,
                kind: ChangelogType::Changed,
            },
        ];
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn changelog_entry_display() {
        let mut entry = ChangelogEntry::new(
            IdType::PullRequest(vec![637]),
            ChangelogContents {
                description: s!("this is one added entry."),
                issues: vec![630],
                breaking: false,
                kind: ChangelogType::Added,
            },
        );
        assert_eq!(entry.to_string(), s!("- #637 - this is one added entry.\n"));

        entry.contents.breaking = true;
        assert_eq!(
            entry.to_string(),
            s!("- #637 - BREAKING: this is one added entry.\n")
        );

        entry.id = IdType::Issue(vec![640]);
        assert_eq!(
            entry.to_string(),
            s!("- BREAKING: this is one added entry.\n")
        );

        entry.contents.breaking = false;
        assert_eq!(entry.to_string(), s!("- this is one added entry.\n"));
    }

    #[test]
    fn read_template_changes() -> cross::Result<()> {
        let mut msg_info = MessageInfo::default();
        let root = project_dir(&mut msg_info)?;

        let mut actual = read_changes(&root.join(".changes").join("template"))?;
        actual.sort_descending();
        let expected = Changes {
            added: vec![
                ChangelogEntry::new(
                    IdType::PullRequest(vec![979, 981]),
                    ChangelogContents {
                        description: s!("this has 2 PRs associated."),
                        issues: vec![441],
                        breaking: false,
                        kind: ChangelogType::Added,
                    },
                ),
                ChangelogEntry::new(
                    IdType::PullRequest(vec![940]),
                    ChangelogContents {
                        description: s!("this is one added entry."),
                        issues: vec![630],
                        breaking: false,
                        kind: ChangelogType::Added,
                    },
                ),
                ChangelogEntry::new(
                    IdType::PullRequest(vec![940]),
                    ChangelogContents {
                        description: s!("this is another added entry."),
                        issues: vec![642],
                        breaking: false,
                        kind: ChangelogType::Added,
                    },
                ),
            ],
            changed: vec![ChangelogEntry::new(
                IdType::PullRequest(vec![940]),
                ChangelogContents {
                    description: s!("this is a breaking change."),
                    issues: vec![679],
                    breaking: true,
                    kind: ChangelogType::Changed,
                },
            )],
            fixed: vec![
                ChangelogEntry::new(
                    IdType::PullRequest(vec![978]),
                    ChangelogContents {
                        description: s!("sample description for a PR adding one CHANGELOG entry."),
                        issues: vec![437],
                        breaking: false,
                        kind: ChangelogType::Fixed,
                    },
                ),
                ChangelogEntry::new(
                    IdType::PullRequest(vec![940]),
                    ChangelogContents {
                        description: s!("this is a fixed entry that has no attached issue."),
                        issues: vec![],
                        breaking: false,
                        kind: ChangelogType::Fixed,
                    },
                ),
                ChangelogEntry::new(
                    IdType::Issue(vec![440]),
                    ChangelogContents {
                        description: s!("no associated PR."),
                        issues: vec![440],
                        breaking: false,
                        kind: ChangelogType::Fixed,
                    },
                ),
            ],
            removed: vec![],
            internal: vec![],
        };
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn read_template_changelog() -> cross::Result<()> {
        let mut msg_info = MessageInfo::default();
        let root = project_dir(&mut msg_info)?;

        let (_, mut actual, _) = read_changelog(&root.join(".changes").join("template"))?;
        actual.sort_descending();
        let expected = ChangelogEntry::new(
            IdType::PullRequest(vec![905]),
            ChangelogContents {
                description: s!("added qemu emulation to `i586-unknown-linux-gnu`, `i686-unknown-linux-musl`, and `i586-unknown-linux-gnu`, so they can run on an `x86` CPU, rather than an `x86_64` CPU."),
                issues: vec![],
                breaking: false,
                kind: ChangelogType::Added,
            },
        );
        assert_eq!(actual.added[0], expected);

        let expected = ChangelogEntry::new(
            IdType::PullRequest(vec![869]),
            ChangelogContents {
                description: s!("ensure cargo configuration environment variable flags are passed to the docker container."),
                issues: vec![],
                breaking: false,
                kind: ChangelogType::Changed,
            },
        );
        assert_eq!(actual.changed[0], expected);

        let expected = ChangelogEntry::new(
            IdType::PullRequest(vec![905]),
            ChangelogContents {
                description: s!("fixed running dynamically-linked libraries for all musl targets except `x86_64-unknown-linux-musl`."),
                issues: vec![],
                breaking: false,
                kind: ChangelogType::Fixed,
            },
        );
        assert_eq!(actual.fixed[0], expected);
        assert_eq!(actual.removed.len(), 0);
        assert_eq!(actual.internal.len(), 0);

        Ok(())
    }

    fn build_changelog_test(release: Option<&str>) -> cross::Result<String> {
        let mut msg_info = MessageInfo::default();
        let root = project_dir(&mut msg_info)?;
        let changes_dir = root.join(".changes").join("template");

        build_changelog_from_dir(&changes_dir, &changes_dir, release)
    }

    #[test]
    fn test_build_changelog_no_release() -> cross::Result<()> {
        let output = build_changelog_test(None)?;
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines[10], "- #979,#981 - this has 2 PRs associated.");
        assert_eq!(lines[11], "- #940 - this is one added entry.");
        assert_eq!(
            lines[36],
            "- #885 - handle symlinks when using remote docker."
        );
        assert_eq!(lines[39], "- no associated PR.");
        assert_eq!(
            &lines[6..12],
            &[
                "## [Unreleased] - ReleaseDate",
                "",
                "### Added",
                "",
                "- #979,#981 - this has 2 PRs associated.",
                "- #940 - this is one added entry.",
            ]
        );

        Ok(())
    }

    #[test]
    fn test_build_changelog_dev_release() -> cross::Result<()> {
        let output = build_changelog_test(Some("0.2.4-alpha"))?;
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(
            &lines[6..12],
            &[
                "## [Unreleased] - ReleaseDate",
                "",
                "### Added",
                "",
                "- #979,#981 - this has 2 PRs associated.",
                "- #940 - this is one added entry.",
            ]
        );

        Ok(())
    }

    #[test]
    fn test_build_changelog_release() -> cross::Result<()> {
        let output = build_changelog_test(Some("0.2.4"))?;
        let lines: Vec<&str> = output.lines().collect();
        let date = get_current_date();

        assert_eq!(
            &lines[6..14],
            &[
                "## [Unreleased] - ReleaseDate",
                "",
                &format!("## [v0.2.4] - {date}"),
                "",
                "### Added",
                "",
                "- #979,#981 - this has 2 PRs associated.",
                "- #940 - this is one added entry.",
            ]
        );

        Ok(())
    }

    static CHANGES_OBJECT: &str = r#"
    {
        "description": "sample description for a PR adding one CHANGELOG entry.",
        "issues": [437],
        "type": "fixed"
    }
    "#;

    static CHANGES_ARRAY: &str = r#"
    [
        {
            "description": "this is one added entry.",
            "issues": [630],
            "type": "added"
        },
        {
            "description": "this is another added entry.",
            "issues": [642],
            "type": "added"
        },
        {
            "description": "this is a fixed entry that has no attached issue.",
            "type": "fixed"
        },
        {
            "description": "this is a breaking change.",
            "issues": [679],
            "breaking": true,
            "type": "changed"
        }
    ]
    "#;
}
