fn main() {
    let p = std::path::PathBuf::from("src/main.rs");
    assert!(p.exists());
    let mut cmd = std::process::Command::new("cp");
    cmd.arg("src/main.rs")
        .arg("lib.rs")
        .output().unwrap();
}
