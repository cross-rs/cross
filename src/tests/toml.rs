use std::io::Read;

use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};

use crate::ToUtf8;

static TOML_REGEX: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"```toml(.*?)\n(.*?)```"#)
        .multi_line(true)
        .dot_matches_new_line(true)
        .build()
        .expect("regex should be valid")
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
        |p| p == Some("md".as_ref()),
    );

    for dir_entry in walk {
        let dir_entry = dir_entry?;
        if dir_entry.file_type().map_or(true, |f| f.is_dir()) {
            continue;
        }
        eprintln!("File: {:?}", dir_entry.path());
        let mut file = std::fs::File::open(dir_entry.path()).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        for matches in TOML_REGEX.captures_iter(&contents) {
            let cargo = {
                let t = matches.get(1).unwrap().as_str();
                if t.is_empty() {
                    false
                } else if t == ",cargo" {
                    true
                } else {
                    println!("skipping {t}");
                    continue;
                }
            };
            let fence = matches.get(2).unwrap();
            let fence_content = fence
                .as_str()
                .replace("$TARGET", "x86_64-unknown-linux-gnu")
                .replace("${target}", "x86_64-unknown-linux-gnu");

            eprintln!(
                "testing snippet at: {}:{:?}",
                dir_entry.path().to_utf8()?,
                text_line_no(&contents, fence.range().start),
            );
            let mut msg_info = crate::shell::MessageInfo::default();
            let toml = if !cargo {
                crate::cross_toml::CrossToml::parse_from_cross_str(
                    &fence_content,
                    None,
                    &mut msg_info,
                )?
            } else {
                crate::cross_toml::CrossToml::parse_from_cargo_package_str(
                    &fence_content,
                    &mut msg_info,
                )?
                .unwrap_or_default()
            };
            assert!(toml.1.is_empty());

            // TODO: Add serde_path_to_error
            // Check if roundtrip works, needed for merging Cross.toml and Cargo.toml
            serde_json::from_value::<crate::cross_toml::CrossToml>(serde_json::to_value(toml.0)?)?;
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
