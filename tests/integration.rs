use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;

/// Lazily-created temp HOME so tests never read the developer's real
/// `$HOME/.config/library-cli/config.toml`. A real config with a
/// `default_module` would flip `no_module_selected_is_usage_error` to exit 0,
/// and a malformed one would make every invocation exit 4.
/// A static (not `set_var`) keeps this race-free with parallel tests.
static HOME_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = std::env::temp_dir().join(format!("libcli-test-home-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create test HOME dir");
    dir
});

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_library-cli"));
    cmd.env("HOME", &*HOME_DIR);
    cmd
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

#[test]
fn install_force_removes_stale_files() {
    let mods_dir = std::env::temp_dir().join(format!("libcli-test-mods-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&mods_dir);
    let src = fixtures().join("installable");

    // First install into a fresh target dir.
    let out = bin()
        .env("LIBRARY_CLI_MODULES", &mods_dir)
        .args(["install", src.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    // Plant a stale file inside the installed module.
    let stale = mods_dir.join("installable/stale.txt");
    std::fs::write(&stale, "stale").unwrap();
    assert!(stale.exists());

    // Forced reinstall must mirror the source exactly: stale file gone.
    let out = bin()
        .env("LIBRARY_CLI_MODULES", &mods_dir)
        .args(["install", src.to_str().unwrap(), "--force"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(!stale.exists(), "stale.txt must be removed by --force reinstall");
    assert!(mods_dir.join("installable/module.py").exists());

    let _ = std::fs::remove_dir_all(&mods_dir);
}

#[test]
fn help_exits_zero() {
    let out = bin().arg("--help").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Usage"));
}

#[test]
fn version_exits_zero() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
}
