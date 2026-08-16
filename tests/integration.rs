use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

static INSTALL_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique_target_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "libcli-install-{}-{}",
        std::process::id(),
        INSTALL_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

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
fn install_copies_module_into_discovery_path() {
    let target = unique_target_dir();
    let source = fixtures().join("installable");
    let out = bin()
        .env("LIBRARY_CLI_MODULES", &target)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(target.join("installable/manifest.toml").exists());
    assert!(target.join("installable/module.py").exists());
    // Installed module is discoverable and runnable.
    let search = bin()
        .env("LIBRARY_CLI_MODULES", &target)
        .args(["search", "x", "--module", "installable"])
        .output()
        .unwrap();
    assert!(search.status.success());
}

#[test]
fn install_refuses_overwrite_without_force() {
    let target = unique_target_dir();
    let source = fixtures().join("installable");
    let first = bin()
        .env("LIBRARY_CLI_MODULES", &target)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(first.status.success());
    let second = bin()
        .env("LIBRARY_CLI_MODULES", &target)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&second.stderr).contains("--force"));
    let third = bin()
        .env("LIBRARY_CLI_MODULES", &target)
        .args(["install", "--force", source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(third.status.success());
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

fn repo_modules() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("modules")
}

fn gutenberg_fixtures() -> PathBuf {
    repo_modules().join("gutenberg").join("fixtures")
}

#[test]
fn gutenberg_search_json_against_fixtures() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", repo_modules())
        .env("LIBRARY_CLI_FIXTURE", gutenberg_fixtures())
        .args(["search", "moby dick", "--module", "gutenberg", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let book = &parsed[0];
    assert_eq!(book["title"], "Moby Dick; Or, The Whale");
    assert_eq!(book["id"], "2701");
    let formats: Vec<String> = book["formats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["format"].as_str().unwrap().to_string())
        .collect();
    assert!(formats.contains(&"epub".to_string()), "got: {formats:?}");
    assert!(formats.contains(&"txt".to_string()), "got: {formats:?}");
}

#[test]
fn gutenberg_search_table_against_fixtures() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", repo_modules())
        .env("LIBRARY_CLI_FIXTURE", gutenberg_fixtures())
        .args(["search", "moby dick", "--module", "gutenberg"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Moby Dick; Or, The Whale"), "got: {text}");
    assert!(text.contains("2701"), "got: {text}");
}

#[test]
fn gutenberg_show_against_fixtures() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", repo_modules())
        .env("LIBRARY_CLI_FIXTURE", gutenberg_fixtures())
        .args(["show", "2701", "--module", "gutenberg"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Moby Dick; Or, The Whale"));
}

fn se_fixtures() -> PathBuf {
    repo_modules().join("standard-ebooks").join("fixtures")
}

fn se_bin() -> Command {
    let mut c = bin();
    c.env("LIBRARY_CLI_MODULES", repo_modules());
    c.env("LIBRARY_CLI_FIXTURE", se_fixtures());
    c
}

#[test]
fn standard_ebooks_categories() {
    let out = se_bin().args(["categories", "--module", "standard-ebooks"]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("New Releases"), "got: {text}");
    assert!(text.contains("Science Fiction"), "got: {text}");
}

#[test]
fn standard_ebooks_list_category() {
    let out = se_bin()
        .args(["list", "--category", "https://standardebooks.org/feeds/opds/new-releases", "--module", "standard-ebooks"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Frankenstein; Or, The Modern Prometheus"), "got: {text}");
    assert!(text.contains("Mary Wollstonecraft Shelley"), "got: {text}");
}

#[test]
fn standard_ebooks_search() {
    let out = se_bin()
        .args(["search", "moby dick", "--module", "standard-ebooks", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(parsed[0]["title"], "Moby Dick");
    let formats: Vec<String> = parsed[0]["formats"].as_array().unwrap().iter()
        .map(|f| f["format"].as_str().unwrap().to_string()).collect();
    assert!(formats.contains(&"azw3".to_string()), "got: {formats:?}");
}

#[test]
fn standard_ebooks_show_book() {
    let out = se_bin()
        .args(["show", "https://standardebooks.org/ebooks/mary-shelley/frankenstein", "--module", "standard-ebooks"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Mary Wollstonecraft Shelley"), "got: {text}");
    assert!(text.contains("kepub"), "got: {text}");
}

#[test]
fn standard_ebooks_show_book_html_fallback() {
    let id = "https://standardebooks.org/ebooks/herman-melville/moby-dick";
    // The book page is XHTML (no OPDS <entry>), so module.py must fall back to
    // parsing the <a property="schema:contentUrl" class="epub|amazon|kobo">
    // download anchors and absolutize relative hrefs against the page URL.
    let out = se_bin()
        .args(["show", id, "--module", "standard-ebooks"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    // Fallback title is the id's last path segment.
    assert!(text.contains("moby-dick"), "got: {text}");
    assert!(text.contains("epub"), "got: {text}");
    // Every format URL must be absolute, and the relative href must be joined
    // to the page URL.
    let urls: Vec<&str> = text
        .lines()
        .filter_map(|l| l.trim().split_once(" - ").map(|(_, url)| url.trim()))
        .collect();
    assert!(!urls.is_empty(), "no format URLs in: {text}");
    for url in &urls {
        assert!(
            url.starts_with("http://") || url.starts_with("https://"),
            "format URL not absolute: {url}"
        );
    }
    assert!(
        urls.contains(&"https://standardebooks.org/ebooks/herman-melville/moby-dick/downloads/herman-melville_moby-dick.epub"),
        "relative href not absolutized against the page URL; got: {urls:?}"
    );
    // The `show` renderer omits the book id, so pin the JSON-RPC contract
    // directly: the result must echo the requested id and absolutize hrefs.
    use std::io::Write;
    let mut child = Command::new("python3")
        .arg(repo_modules().join("standard-ebooks/module.py"))
        .current_dir(repo_modules().join("standard-ebooks"))
        .env("LIBRARY_CLI_FIXTURE", se_fixtures())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"book\",\"params\":{{\"id\":\"{id}\"}}}}\n").as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let book = &parsed["result"]["book"];
    assert_eq!(book["id"], id);
    let formats = book["formats"].as_array().unwrap();
    assert!(!formats.is_empty(), "no formats in {book}");
    for f in formats {
        let url = f["url"].as_str().unwrap();
        assert!(
            url.starts_with("http://") || url.starts_with("https://"),
            "format URL not absolute: {url}"
        );
    }
    let epub = formats.iter().find(|f| f["format"].as_str() == Some("epub")).expect("epub format");
    assert_eq!(
        epub["url"],
        "https://standardebooks.org/ebooks/herman-melville/moby-dick/downloads/herman-melville_moby-dick.epub"
    );
}
