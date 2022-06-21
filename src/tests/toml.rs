use std::io::Read;

use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};

static TOML_REGEX: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"```toml\n(.*?)```"#)
        .multi_line(true)
        .dot_matches_new_line(true)
        .build()
        .unwrap()
});

#[test]
fn toml_check() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = super::get_cargo_workspace();
    let walk = super::walk_dir(
        workspace_root,
        &[
            "target",
            ".git",
            "src",
            "CODE_OF_CONDUCT.md",
            "CHANGELOG.md",
        ],
    );

    for dir_entry in walk {
        let dir_entry = dir_entry?;
        if dir_entry.file_type().is_dir() {
            continue;
        }
        eprintln!("File: {:?}", dir_entry.path());
        let mut file = std::fs::File::open(dir_entry.path()).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        for matches in TOML_REGEX.captures_iter(&contents) {
            let fence = matches.get(1).unwrap();
            eprintln!(
                "testing snippet at: {:?}:{:?}",
                dir_entry.path(),
                text_line_no(&contents, fence.range().start),
            );
            assert!(crate::cross_toml::CrossToml::parse(fence.as_str())?
                .1
                .is_empty());
        }
    }
    Ok(())
}

pub fn text_line_no(text: &str, index: usize) -> usize {
    let mut line_no = 0;
    let mut count = 0;
    for line in text.split('\n') {
        line_no += 1;
        count += line.as_bytes().len() + 1;
        if count >= index {
            break;
        }
    }
    line_no
}
