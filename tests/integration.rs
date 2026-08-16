use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_library-cli"))
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn run_search_table() -> String {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["search", "fake", "--module", "fake", "--limit", "5"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn search_renders_table() {
    let text = run_search_table();
    assert!(text.contains("Fake Book"), "got: {text}");
    assert!(text.contains("Fake Author"), "got: {text}");
}

#[test]
fn search_json_output() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["search", "fake", "--module", "fake", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(parsed[0]["title"], "Fake Book");
    assert_eq!(parsed[0]["formats"][0]["format"], "epub");
}

#[test]
fn categories_renders() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["categories", "--module", "fake"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Category One"));
}

#[test]
fn show_renders_book() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["show", "1", "--module", "fake"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Fake Book"));
}

#[test]
fn list_requires_category() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["list", "--module", "fake"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn download_unreachable_url_is_network_error() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["download", "1", "--format", "epub", "--module", "fake"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn download_unknown_format_is_usage_error() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["download", "1", "--format", "pdf", "--module", "fake"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no format"));
}

#[test]
fn unknown_module_is_usage_error() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["search", "x", "--module", "nope"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn empty_query_is_usage_error() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["search", "", "--module", "fake"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn broken_module_is_config_error() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["search", "x", "--module", "broken"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
}

#[test]
fn no_module_selected_is_usage_error() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .args(["search", "x"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn modules_lists_entries_and_broken() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", fixtures())
        .arg("modules")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("fake v0.1.0"), "got: {text}");
    assert!(text.contains("BROKEN"), "got: {text}");
}
