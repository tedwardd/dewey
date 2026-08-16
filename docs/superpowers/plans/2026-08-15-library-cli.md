# library-cli Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `library-cli`, a Rust CLI framework that accesses open ebook libraries through standalone modules speaking JSON-RPC over stdio, with working Project Gutenberg and Standard Ebooks modules.

**Architecture:** A single Rust binary discovers module directories, spawns a module process per CLI verb, exchanges one JSON-RPC request/response over newline-delimited JSON, and normalizes results onto a shared `Book` record. Reference modules are Python 3 stdlib-only programs; downloads are host-owned (modules return direct URLs).

**Tech Stack:** Rust (edition 2021), clap 4 (derive), serde/serde_json, toml 0.8, ureq 2 (blocking HTTP), indicatif 0.17 (progress). Modules: Python 3 stdlib only (`urllib`, `json`, `xml.etree.ElementTree`, `html.parser`). Tests: `cargo test` unit + integration tests that spawn real module processes against recorded fixtures.

**Spec:** `docs/superpowers/specs/2026-08-15-library-cli-design.md` (approved).

## Global Constraints

Apply to every task; copied verbatim from the spec:

- **Protocol:** JSON-RPC 2.0 over NDJSON — one compact JSON object per line, LF-terminated, UTF-8. No non-protocol bytes on module stdout; diagnostics go to stderr. Unknown/extra JSON fields are ignored.
- **One-shot:** host sends exactly one request per module process; module exits 0 after responding; 30 s exchange timeout.
- **Methods/capabilities:** `search`, `categories`, `list`, `book`. Manifest `capabilities` is a non-empty subset of these; the host surfaces only declared capabilities.
- **Book contract:** `id`/`title`/`formats` required; `authors`, `languages`, `published` (int year), `description`, `categories` optional. Format tags: `epub`, `azw3`, `mobi`, `kepub`, `txt`, `html`. Download URLs are `http(s)` only.
- **Manifest:** `name` matches `^[a-z0-9-]+$` and equals the dir name; `version` non-empty; `command` non-empty array (first element = PATH lookup, or relative file in the module dir; other args resolved relative to the module dir).
- **Exit codes:** 0 success, 1 usage, 2 module/protocol, 3 network/IO, 4 config/discovery.
- **Config:** `~/.config/library-cli/config.toml`, keys `default_module`, `download_dir` (tilde-expanded). Missing file OK; invalid file → exit 4.
- **Discovery:** `LIBRARY_CLI_MODULES` env var overrides the default modules dir `~/.config/library-cli/modules`.
- **Download:** progress bar (only when stderr is a TTY); single retry on transient failure (HTTP 5xx, connection-level errors); refuse overwrite unless `--force`; filename `Title - Author.ext` (authors joined `, `; empty → `Title.ext`); sanitize illegal filename chars → `-`, collapse whitespace, trim.
- **Module selection:** `--module` flag > `default_module` config > error (exit 1) listing installed modules.
- **install:** copy module dir into `<modules-dir>/<name>/`; refuse overwrite unless `--force`.
- **Output:** aligned table by default; `--json` emits normalized Book records.
- **Modules:** Python 3 stdlib only. When `LIBRARY_CLI_FIXTURE=<dir>` is set, modules MUST answer from fixture files and never touch the network. Fixture filenames: `<method>.json` (no params) or `<method>-<slug>.json` where slug = lowercase, non-alphanumerics → `-` (URLs keyed by last path segment).
- **Reference APIs (verify live during implementation; shapes may require small fixes):** gutendex `https://gutendex.com/books/?search=<q>&page=<n>` and `/books/<id>`; Standard Ebooks OPDS root `https://standardebooks.org/feeds/opds/all`, acquisition feeds, search template `https://standardebooks.org/feeds/opds/search?query={searchTerms}`; MIME map: `application/epub+zip`→epub, `application/vnd.amazon.ebook`→azw3, `application/x-kepub+zip`→kepub.
- **Environment:** `python3` (≥ 3.8) must be on PATH. Do NOT run formatters or linters; run only the commands listed in each step.

---

### Task 1: Crate scaffold + error type

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/errors.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `CliError` enum with variants `Usage(String)`, `Module(String)`, `Network(String)`, `Config(String)`; methods `exit_code() -> i32` (1/2/3/4), `Display`; `From<std::io::Error>` (Network), `From<ureq::Error>` (Network), `From<serde_json::Error>` (Module), `From<toml::de::Error>` (Config). Later tasks construct these via `CliError::Usage(format!(...))` etc.

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "library-cli"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4", features = ["derive"] }
indicatif = "0.17"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
ureq = "2"
```

- [ ] **Step 2: Create `src/main.rs`**

```rust
mod errors;

fn main() {
    println!("library-cli");
}
```

(A `dead_code` warning for the unused `errors` module is expected and disappears as later tasks use it.)

- [ ] **Step 3: Create `src/errors.rs`**

```rust
use std::fmt;

/// Application error carrying its CLI exit code.
#[derive(Debug)]
pub enum CliError {
    Usage(String),
    Module(String),
    Network(String),
    Config(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Usage(_) => 1,
            CliError::Module(_) => 2,
            CliError::Network(_) => 3,
            CliError::Config(_) => 4,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Usage(m) => write!(f, "{m}"),
            CliError::Module(m) => write!(f, "{m}"),
            CliError::Network(m) => write!(f, "{m}"),
            CliError::Config(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Network(format!("io: {e}"))
    }
}

impl From<ureq::Error> for CliError {
    fn from(e: ureq::Error) -> Self {
        CliError::Network(format!("http: {e}"))
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        CliError::Module(format!("module protocol: {e}"))
    }
}

impl From<toml::de::Error> for CliError {
    fn from(e: toml::de::Error) -> Self {
        CliError::Config(format!("config: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::CliError;

    #[test]
    fn exit_codes_map_to_spec_values() {
        assert_eq!(CliError::Usage("u".into()).exit_code(), 1);
        assert_eq!(CliError::Module("m".into()).exit_code(), 2);
        assert_eq!(CliError::Network("n".into()).exit_code(), 3);
        assert_eq!(CliError::Config("c".into()).exit_code(), 4);
    }

    #[test]
    fn display_prints_message() {
        assert_eq!(CliError::Module("boom".into()).to_string(), "boom");
    }

    #[test]
    fn error_trait_implemented() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<CliError>();
    }
}
```

- [ ] **Step 4: Build and run tests**

Run: `cargo test`
Expected: 3 tests pass; one `dead_code` warning for `errors` (accepted).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/main.rs src/errors.rs
git commit -m "chore: scaffold crate with error types and exit codes"
```

---

### Task 2: Book data model

**Files:**
- Create: `src/book.rs`
- Modify: `src/main.rs` (add `mod book;`)

**Interfaces:**
- Consumes: nothing (pure types).
- Produces: `Book` (serde Serialize+Deserialize; fields `id: String`, `title: String`, `authors: Vec<String>` [default], `languages: Vec<String>` [default], `published: Option<i64>` [default], `description: Option<String>` [default], `categories: Vec<String>` [default], `formats: Vec<Format>` [default]), `Format { format: String, url: String, size: Option<u64> }`, `Category { id: String, title: String }`, response wrappers `BooksResponse { books: Vec<Book>, total: Option<u64> }`, `CategoriesResponse { categories: Vec<Category> }`, `BookResponse { book: Book }`; function `extension_for(format: &str) -> &'static str` mapping `epub/azw3/mobi/kepub/txt/html` to themselves, unknown tags passthrough.

- [ ] **Step 1: Write the failing test in `src/book.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Book {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub published: Option<i64>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub formats: Vec<Format>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Format {
    pub format: String,
    pub url: String,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Category {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct BooksResponse {
    pub books: Vec<Book>,
    #[serde(default)]
    pub total: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CategoriesResponse {
    pub categories: Vec<Category>,
}

#[derive(Debug, Deserialize)]
pub struct BookResponse {
    pub book: Book,
}

pub fn extension_for(format: &str) -> &'static str {
    match format {
        "epub" | "azw3" | "mobi" | "kepub" | "txt" | "html" => format,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip_minimal_book() {
        let json = r#"{"id":"1","title":"T","formats":[]}"#;
        let book: Book = serde_json::from_str(json).unwrap();
        assert_eq!(book.id, "1");
        assert_eq!(book.title, "T");
        assert!(book.authors.is_empty());
        assert!(serde_json::to_string(&book).unwrap().contains("\"authors\":[]"));
    }

    #[test]
    fn serde_roundtrip_full_book() {
        let json = r#"{"id":"2","title":"T","authors":["A"],"languages":["en"],
            "published":1851,"description":"d","categories":["c"],
            "formats":[{"format":"epub","url":"https://x/y.epub","size":10}]}"#;
        let book: Book = serde_json::from_str(json).unwrap();
        assert_eq!(book.published, Some(1851));
        assert_eq!(book.formats[0].size, Some(10));
    }

    #[test]
    fn extension_for_known_and_unknown() {
        assert_eq!(extension_for("epub"), "epub");
        assert_eq!(extension_for("txt"), "txt");
        assert_eq!(extension_for("weird"), "weird");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test book`
Expected: FAIL — `book` module not found (no such module).

- [ ] **Step 3: Add `mod book;` to `src/main.rs`**

```rust
mod book;
mod errors;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test book`
Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/book.rs src/main.rs
git commit -m "feat: add Book/Category data model and format tags"
```

---

### Task 3: JSON-RPC framing

**Files:**
- Create: `src/module/jsonrpc.rs`
- Create: `src/module/mod.rs` (only `pub mod jsonrpc;` for now)
- Modify: `src/main.rs` (add `mod module;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `Request { jsonrpc: String, id: u64, method: String, params: Option<serde_json::Value> }`, `Response { jsonrpc: String, id: u64, result: Option<Value>, error: Option<RpcError> }`, `RpcError { code: i64, message: String }` (all serde Serialize+Deserialize); `encode_request(id: u64, method: &str, params: &Value) -> String` (single JSON object + `\n`); `decode_response(line: &str) -> Result<Response, serde_json::Error>`.

- [ ] **Step 1: Write the failing test in `src/module/jsonrpc.rs`**

```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

pub fn encode_request(id: u64, method: &str, params: &Value) -> String {
    let req = Request {
        jsonrpc: "2.0".into(),
        id,
        method: method.into(),
        params: Some(params.clone()),
    };
    format!("{}\n", serde_json::to_string(&req).expect("request serializes"))
}

pub fn decode_response(line: &str) -> Result<Response, serde_json::Error> {
    serde_json::from_str(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_request_is_single_ndjson_line() {
        let line = encode_request(7, "search", &json!({"query": "dune"}));
        assert!(line.ends_with('\n'));
        let req: Request = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, 7);
        assert_eq!(req.method, "search");
        assert_eq!(req.params, Some(json!({"query": "dune"})));
    }

    #[test]
    fn decode_response_result() {
        let resp = decode_response(r#"{"jsonrpc":"2.0","id":7,"result":{"ok":true}}"#).unwrap();
        assert_eq!(resp.id, 7);
        assert_eq!(resp.result, Some(json!({"ok": true})));
        assert!(resp.error.is_none());
    }

    #[test]
    fn decode_response_error() {
        let resp = decode_response(r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32000,"message":"boom"}}"#).unwrap();
        assert_eq!(resp.error.unwrap().code, -32000);
        assert!(resp.result.is_none());
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode_response("not json\n").is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test jsonrpc`
Expected: FAIL — module not found.

- [ ] **Step 3: Create `src/module/mod.rs`**

```rust
pub mod jsonrpc;
```

- [ ] **Step 4: Add `mod module;` to `src/main.rs` and run tests**

```rust
mod book;
mod errors;
mod module;
```

Run: `cargo test jsonrpc`
Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/module/jsonrpc.rs src/module/mod.rs src/main.rs
git commit -m "feat: add JSON-RPC request/response framing"
```

---

### Task 4: Config loading

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs` (add `mod config;`)

**Interfaces:**
- Consumes: `CliError`.
- Produces: `Config { default_module: Option<String>, download_dir: Option<String> }` (Deserialize, Default), `Config::expanded_download_dir() -> Option<String>`; `config_dir() -> Result<PathBuf, CliError>` (`$HOME/.config/library-cli`, `HOME` unset → Config error); `modules_dir() -> Result<PathBuf, CliError>` (`LIBRARY_CLI_MODULES` env override, empty value → Config error; else `config_dir()/modules`); `load_config() -> Result<Config, CliError>` (missing file → default; parse/validation errors → Config); `load_config_from(dir: &Path) -> Result<Config, CliError>` (testable variant reading `dir/config.toml`); `expand_tilde(s: &str) -> String`.

- [ ] **Step 1: Write the failing test in `src/config.rs`**

```rust
use crate::errors::CliError;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_DIR_NAME: &str = "library-cli";

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub default_module: Option<String>,
    pub download_dir: Option<String>,
}

impl Config {
    pub fn expanded_download_dir(&self) -> Option<String> {
        self.download_dir.as_ref().map(|d| expand_tilde(d))
    }
}

pub fn config_dir() -> Result<PathBuf, CliError> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| CliError::Config("HOME not set; cannot locate config directory".into()))?;
    Ok(PathBuf::from(home).join(".config").join(CONFIG_DIR_NAME))
}

pub fn modules_dir() -> Result<PathBuf, CliError> {
    if let Some(dir) = std::env::var_os("LIBRARY_CLI_MODULES") {
        if dir.is_empty() {
            return Err(CliError::Config("LIBRARY_CLI_MODULES is empty".into()));
        }
        return Ok(PathBuf::from(dir));
    }
    Ok(config_dir()?.join("modules"))
}

pub fn load_config() -> Result<Config, CliError> {
    load_config_from(&config_dir()?)
}

pub fn load_config_from(dir: &Path) -> Result<Config, CliError> {
    let path = dir.join("config.toml");
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| CliError::Config(format!("cannot read {}: {e}", path.display())))?;
    let cfg: Config = toml::from_str(&text)
        .map_err(|e| CliError::Config(format!("invalid {}: {e}", path.display())))?;
    if let Some(m) = &cfg.default_module {
        if m.trim().is_empty() {
            return Err(CliError::Config(
                "config.toml: default_module must not be empty".into(),
            ));
        }
    }
    Ok(cfg)
}

pub fn expand_tilde(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "libcli-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_config_file_returns_defaults() {
        let dir = temp_dir();
        let cfg = load_config_from(&dir).unwrap();
        assert!(cfg.default_module.is_none());
        assert!(cfg.download_dir.is_none());
    }

    #[test]
    fn valid_config_parses() {
        let dir = temp_dir();
        fs::write(
            dir.join("config.toml"),
            "default_module = \"gutenberg\"\ndownload_dir = \"~/Downloads\"\n",
        )
        .unwrap();
        let cfg = load_config_from(&dir).unwrap();
        assert_eq!(cfg.default_module.as_deref(), Some("gutenberg"));
        assert_eq!(
            cfg.expanded_download_dir().as_deref(),
            Some(&format!("{}/Downloads", std::env::var("HOME").unwrap()))
        );
    }

    #[test]
    fn invalid_toml_is_config_error() {
        let dir = temp_dir();
        fs::write(dir.join("config.toml"), "not = = toml").unwrap();
        let err = load_config_from(&dir).unwrap_err();
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn empty_default_module_is_config_error() {
        let dir = temp_dir();
        fs::write(dir.join("config.toml"), "default_module = \"\"\n").unwrap();
        let err = load_config_from(&dir).unwrap_err();
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn expand_tilde_replaces_home() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand_tilde("~/x"), format!("{home}/x"));
        assert_eq!(expand_tilde("/abs/path"), "/abs/path");
        assert_eq!(expand_tilde("relative"), "relative");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test config`
Expected: FAIL — module not found.

- [ ] **Step 3: Add `mod config;` to `src/main.rs` and run tests**

```rust
mod book;
mod config;
mod errors;
mod module;
```

Run: `cargo test config`
Expected: 5 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: add config loading and module path resolution"
```

---

### Task 5: Module discovery and manifest validation

**Files:**
- Create: `src/discovery.rs`
- Create: `tests/fixtures/fake/manifest.toml`
- Create: `tests/fixtures/fake/module.py`
- Create: `tests/fixtures/broken/manifest.toml`
- Create: `tests/fixtures/installable/manifest.toml`
- Create: `tests/fixtures/installable/module.py`
- Modify: `src/main.rs` (add `mod discovery;`)

**Interfaces:**
- Consumes: `CliError`.
- Produces: `pub const CAPABILITIES: [&str; 4] = ["search", "categories", "list", "book"]`; `Manifest { name: String, version: String, description: Option<String>, command: Vec<String>, capabilities: Vec<String> }` (Deserialize); `ModuleEntry { name: String, dir: PathBuf, manifest: Option<Manifest>, error: Option<String> }`; `discover(dir: &Path) -> Vec<ModuleEntry>` (reads subdirs sorted by name; valid → `manifest: Some`, broken → `error: Some`); `load_manifest(dir: &Path) -> Result<Manifest, String>`; `resolve_module<'a>(entries: &'a [ModuleEntry], name: &str) -> Result<&'a ModuleEntry, CliError>` (unknown → `Usage` listing installed; broken → `Config`).

- [ ] **Step 1: Write the failing test in `src/discovery.rs`**

```rust
use crate::errors::CliError;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const CAPABILITIES: [&str; 4] = ["search", "categories", "list", "book"];

#[derive(Debug, Deserialize, Clone)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub command: Vec<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug)]
pub struct ModuleEntry {
    pub name: String,
    pub dir: PathBuf,
    pub manifest: Option<Manifest>,
    pub error: Option<String>,
}

pub fn discover(dir: &Path) -> Vec<ModuleEntry> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        match load_manifest(&e.path()) {
            Ok(m) => out.push(ModuleEntry {
                name,
                dir: e.path(),
                manifest: Some(m),
                error: None,
            }),
            Err(err) => out.push(ModuleEntry {
                name,
                dir: e.path(),
                manifest: None,
                error: Some(err),
            }),
        }
    }
    out
}

pub fn load_manifest(dir: &Path) -> Result<Manifest, String> {
    let path = dir.join("manifest.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("cannot read manifest.toml: {e}"))?;
    let m: Manifest = toml::from_str(&text).map_err(|e| format!("invalid manifest.toml: {e}"))?;
    validate(&m, dir)?;
    Ok(m)
}

fn validate(m: &Manifest, dir: &Path) -> Result<(), String> {
    let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let name_ok = !m.name.is_empty()
        && m.name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !name_ok {
        return Err(format!("name must match ^[a-z0-9-]+$, got {:?}", m.name));
    }
    if m.name != dir_name {
        return Err(format!(
            "manifest name {:?} does not match directory {dir_name:?}",
            m.name
        ));
    }
    if m.version.trim().is_empty() {
        return Err("version must be non-empty".into());
    }
    if m.command.is_empty() {
        return Err("command must be non-empty".into());
    }
    if m.capabilities.is_empty() {
        return Err("capabilities must be non-empty".into());
    }
    for c in &m.capabilities {
        if !CAPABILITIES.contains(&c.as_str()) {
            return Err(format!(
                "unknown capability {c:?} (allowed: {})",
                CAPABILITIES.join(", ")
            ));
        }
    }
    Ok(())
}

pub fn resolve_module<'a>(entries: &'a [ModuleEntry], name: &str) -> Result<&'a ModuleEntry, CliError> {
    let Some(e) = entries.iter().find(|e| e.name == name) else {
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        let list = if names.is_empty() {
            "(none)".to_string()
        } else {
            names.join(", ")
        };
        return Err(CliError::Usage(format!(
            "unknown module {name:?}; installed: {list}"
        )));
    };
    if e.manifest.is_none() {
        return Err(CliError::Config(format!(
            "module {name:?} is broken: {}",
            e.error.as_deref().unwrap_or("invalid manifest")
        )));
    }
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn discover_classifies_valid_and_broken() {
        let entries = discover(&fixtures());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"fake"));
        assert!(names.contains(&"broken"));
        let fake = entries.iter().find(|e| e.name == "fake").unwrap();
        assert!(fake.manifest.is_some());
        assert!(fake.error.is_none());
        let broken = entries.iter().find(|e| e.name == "broken").unwrap();
        assert!(broken.manifest.is_none());
        assert!(broken.error.is_some());
    }

    #[test]
    fn discover_on_missing_dir_is_empty() {
        assert!(discover(&fixtures().join("nope")).is_empty());
    }

    #[test]
    fn resolve_unknown_is_usage_error() {
        let entries = discover(&fixtures());
        let err = resolve_module(&entries, "nope").unwrap_err();
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn resolve_broken_is_config_error() {
        let entries = discover(&fixtures());
        let err = resolve_module(&entries, "broken").unwrap_err();
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn resolve_valid_returns_entry() {
        let entries = discover(&fixtures());
        let e = resolve_module(&entries, "fake").unwrap();
        assert_eq!(
            e.manifest.as_ref().unwrap().capabilities,
            vec!["search", "categories", "list", "book"]
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test discovery`
Expected: FAIL — module not found.

- [ ] **Step 3: Create the test fixture modules**

`tests/fixtures/fake/manifest.toml`:

```toml
name = "fake"
version = "0.1.0"
description = "Deterministic fake module for tests"
command = ["python3", "module.py"]
capabilities = ["search", "categories", "list", "book"]
```

`tests/fixtures/fake/module.py` (used by CLI integration tests in Task 9; answers from inline data, never the network):

```python
#!/usr/bin/env python3
import json
import sys

line = sys.stdin.readline()
if not line:
    sys.exit(1)
req = json.loads(line)
method = req["method"]
if method == "search":
    result = {"books": [{"id": "1", "title": "Fake Book", "authors": ["Fake Author"],
                         "formats": [{"format": "epub", "url": "http://127.0.0.1:1/x.epub"}]}]}
elif method == "categories":
    result = {"categories": [{"id": "cat1", "title": "Category One"}]}
elif method == "list":
    result = {"books": [{"id": "2", "title": "Listed Book", "formats": []}]}
elif method == "book":
    result = {"book": {"id": "1", "title": "Fake Book", "authors": ["Fake Author"],
                       "formats": [{"format": "epub", "url": "http://127.0.0.1:1/x.epub"}]}}
else:
    sys.stderr.write("fake: unknown method\n")
    sys.exit(1)
sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": result}) + "\n")
```

`tests/fixtures/broken/manifest.toml` (invalid: empty capabilities):

```toml
name = "broken"
version = "0.1.0"
command = ["python3", "module.py"]
capabilities = []
```

`tests/fixtures/installable/manifest.toml` (used by the install test in Task 10):

```toml
name = "installable"
version = "0.1.0"
description = "Module used to test install"
command = ["python3", "module.py"]
capabilities = ["search"]
```

`tests/fixtures/installable/module.py`:

```python
#!/usr/bin/env python3
import json
import sys

req = json.loads(sys.stdin.readline())
sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {"books": []}}) + "\n")
```

- [ ] **Step 4: Add `mod discovery;` to `src/main.rs` and run tests**

```rust
mod book;
mod config;
mod discovery;
mod errors;
mod module;
```

Run: `cargo test discovery`
Expected: 5 tests PASS. (The `broken` dir has no `module.py` — fine, discovery only reads manifests.)

- [ ] **Step 5: Commit**

```bash
git add src/discovery.rs src/main.rs tests/fixtures
git commit -m "feat: add module discovery and manifest validation"
```

---

### Task 6: Module host — spawn, one-shot exchange, timeout

**Files:**
- Create: `src/module/mod.rs` (extend with host; keep `pub mod jsonrpc;`)
- Create: `tests/fixtures/fake-ok.py`
- Create: `tests/fixtures/fake-error.py`
- Create: `tests/fixtures/fake-crash.py`
- Create: `tests/fixtures/fake-sleep.py`
- Create: `tests/fixtures/fake-badjson.py`

**Interfaces:**
- Consumes: `CliError`, `module::jsonrpc::{encode_request, decode_response}`.
- Produces: `pub const EXCHANGE_TIMEOUT: Duration` (= 30 s); `ModuleHost { pub name: String, command: Vec<String>, cwd: PathBuf }` with `ModuleHost::new(name: String, manifest_command: &[String], cwd: PathBuf) -> ModuleHost` (resolves command per Global Constraints), `call(&self, method: &str, params: Value, id: u64) -> Result<Value, CliError>` (30 s timeout), `call_with_timeout(&self, method: &str, params: Value, id: u64, timeout: Duration) -> Result<Value, CliError>`. Error mapping: spawn/write/read failure, timeout, exit-without-response → `CliError::Module` with `module <name>: <detail>`; module JSON-RPC error → `CliError::Module` with `module <name>: <message> (code <code>)`.

- [ ] **Step 1: Write the failing tests in `src/module/mod.rs`**

```rust
pub mod jsonrpc;

use crate::errors::CliError;
use crate::module::jsonrpc::{decode_response, encode_request, Response};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ModuleHost {
    pub name: String,
    command: Vec<String>,
    cwd: PathBuf,
}

impl ModuleHost {
    pub fn new(name: String, manifest_command: &[String], cwd: PathBuf) -> ModuleHost {
        let command = resolve_command(manifest_command, &cwd);
        ModuleHost {
            name,
            command,
            cwd,
        }
    }

    pub fn call(&self, method: &str, params: Value, id: u64) -> Result<Value, CliError> {
        self.call_with_timeout(method, params, id, EXCHANGE_TIMEOUT)
    }

    pub fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        id: u64,
        timeout: Duration,
    ) -> Result<Value, CliError> {
        let mut child = Command::new(&self.command[0])
            .args(&self.command[1..])
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                CliError::Module(format!(
                    "module {}: cannot spawn {:?}: {e}",
                    self.name, self.command[0]
                ))
            })?;

        let mut stdin = child.stdin.take().expect("stdin piped");
        let line = encode_request(id, method, &params);
        if let Err(e) = stdin.write_all(line.as_bytes()) {
            let _ = child.kill();
            return Err(CliError::Module(format!(
                "module {}: failed to send request: {e}",
                self.name
            )));
        }
        drop(stdin); // EOF so the module sees a complete request

        let stdout = child.stdout.take().expect("stdout piped");
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let n = reader.read_line(&mut line);
            let _ = tx.send(n.map(|n| if n == 0 { None } else { Some(line) }));
        });

        let outcome = match rx.recv_timeout(timeout) {
            Ok(Ok(Some(line))) => {
                let resp: Response = decode_response(line.trim_end())?;
                if let Some(err) = resp.error {
                    return Err(CliError::Module(format!(
                        "module {}: {} (code {})",
                        self.name, err.message, err.code
                    )));
                }
                Ok(resp.result.unwrap_or(Value::Null))
            }
            Ok(Ok(None)) => Err(CliError::Module(format!(
                "module {}: exited without a response",
                self.name
            ))),
            Ok(Err(e)) => Err(CliError::Module(format!(
                "module {}: read error: {e}",
                self.name
            ))),
            Err(_) => Err(CliError::Module(format!(
                "module {}: timed out after {}s",
                self.name,
                timeout.as_secs()
            ))),
        };

        let _ = handle.join();
        match outcome {
            Ok(v) => {
                let _ = finish(child, Duration::from_secs(2));
                Ok(v)
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(e)
            }
        }
    }
}

fn resolve_command(cmd: &[String], dir: &Path) -> Vec<String> {
    let program = if Path::new(&cmd[0]).components().count() > 1 || dir.join(&cmd[0]).is_file() {
        dir.join(&cmd[0]).to_string_lossy().into_owned()
    } else {
        cmd[0].clone()
    };
    let mut out = vec![program];
    for arg in &cmd[1..] {
        let p = Path::new(arg);
        out.push(if p.is_absolute() {
            arg.clone()
        } else {
            dir.join(arg).to_string_lossy().into_owned()
        });
    }
    out
}

fn finish(mut child: Child, grace: Duration) -> std::io::Result<()> {
    let deadline = std::time::Instant::now() + grace;
    loop {
        if let Some(status) = child.try_wait()? {
            let _ = status;
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(script: &str) -> ModuleHost {
        ModuleHost::new(
            "fake".into(),
            &["python3".into(), script.into()],
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
        )
    }

    #[test]
    fn returns_result_value() {
        let v = host("fake-ok.py").call("ping", json!({}), 1).unwrap();
        assert_eq!(v, json!({"ok": true}));
    }

    #[test]
    fn surfaces_module_error() {
        let err = host("fake-error.py").call("ping", json!({}), 1).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("boom"), "got: {err}");
    }

    #[test]
    fn crash_without_response_is_module_error() {
        let err = host("fake-crash.py").call("ping", json!({}), 1).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("exited without a response"), "got: {err}");
    }

    #[test]
    fn timeout_kills_module() {
        let err = host("fake-sleep.py")
            .call_with_timeout("ping", json!({}), 1, Duration::from_secs(1))
            .unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("timed out"), "got: {err}");
    }

    #[test]
    fn invalid_response_is_protocol_error() {
        let err = host("fake-badjson.py").call("ping", json!({}), 1).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("module protocol"), "got: {err}");
    }
}
```

- [ ] **Step 2: Create the fake module scripts in `tests/fixtures/`** (the test file above is the full final content of `src/module/mod.rs`)

`tests/fixtures/fake-ok.py`:

```python
import json
import sys

req = json.loads(sys.stdin.readline())
sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {"ok": True}}) + "\n")
```

`tests/fixtures/fake-error.py`:

```python
import json
import sys

req = json.loads(sys.stdin.readline())
sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "error": {"code": -32000, "message": "boom"}}) + "\n")
```

`tests/fixtures/fake-crash.py`:

```python
import sys

sys.exit(1)
```

`tests/fixtures/fake-sleep.py`:

```python
import json
import sys
import time

time.sleep(10)
req = json.loads(sys.stdin.readline())
sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {}}) + "\n")
```

`tests/fixtures/fake-badjson.py`:

```python
import sys

sys.stdout.write("not json\n")
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test module::tests`
Expected: 5 tests PASS (takes ~1 s for the timeout test).

- [ ] **Step 4: Commit**

```bash
git add src/module/mod.rs tests/fixtures/fake-ok.py tests/fixtures/fake-error.py tests/fixtures/fake-crash.py tests/fixtures/fake-sleep.py tests/fixtures/fake-badjson.py
git commit -m "feat: add module host with one-shot exchange and timeout"
```

---

### Task 7: Downloader — URL fetch, retry, filenames, overwrite

**Files:**
- Create: `src/download.rs`
- Modify: `src/main.rs` (add `mod download;`)

**Interfaces:**
- Consumes: `CliError`, `book::{Book, extension_for}`.
- Produces: `sanitize_component(s: &str) -> String`; `download_filename(book: &Book, format: &str) -> String` (`Title - Author.ext`, `Author` = authors joined `, `, dropped when empty; ext from `extension_for`); `resolve_dest(dir: &Path, filename: &str, force: bool) -> Result<PathBuf, CliError>` (existing file without `--force` → `CliError::Network`); `fetch_to_file(url: &str, dest: &Path) -> Result<u64, CliError>` (bytes written; one retry on transient failures: HTTP 5xx or connection-level errors; progress bar only when stderr is a TTY; final errors → `CliError::Network`).

- [ ] **Step 1: Write the failing test in `src/download.rs`**

```rust
use crate::book::{extension_for, Book};
use crate::errors::CliError;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{IsTerminal, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

pub fn sanitize_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
            }
            prev_space = true;
            continue;
        }
        prev_space = false;
        match ch {
            '/' | '\\' | '\0' | '<' | '>' | ':' | '"' | '|' | '?' | '*' => out.push('-'),
            _ => out.push(ch),
        }
    }
    out.trim().to_string()
}

pub fn download_filename(book: &Book, format: &str) -> String {
    let author = if book.authors.is_empty() {
        String::new()
    } else {
        format!(" - {}", book.authors.join(", "))
    };
    format!(
        "{}{}.{}",
        sanitize_component(&book.title),
        author,
        extension_for(format)
    )
}

pub fn resolve_dest(dir: &Path, filename: &str, force: bool) -> Result<PathBuf, CliError> {
    let dest = dir.join(filename);
    if dest.exists() && !force {
        return Err(CliError::Network(format!(
            "{} already exists; use --force to overwrite",
            dest.display()
        )));
    }
    Ok(dest)
}

enum FetchError {
    Status(u16),
    Transport(String),
    Io(std::io::Error),
}

fn fetch_once(url: &str, dest: &Path) -> Result<u64, FetchError> {
    let resp = ureq::get(url).call().map_err(|e| match e {
        ureq::Error::Status(code, _) => FetchError::Status(code),
        ureq::Error::Transport(t) => FetchError::Transport(t.to_string()),
    })?;
    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok());
    let mut reader = resp.into_reader();
    let mut file = File::create(dest).map_err(FetchError::Io)?;
    let show_progress = std::io::stderr().is_terminal();
    let pb = match total {
        Some(t) => ProgressBar::new(t),
        None => ProgressBar::new_spinner(),
    };
    if show_progress {
        pb.set_style(ProgressStyle::with_template("{bar:40} {bytes}/{total_bytes} {msg}").unwrap());
    }
    let mut buf = [0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let n = reader.read(&mut buf).map_err(FetchError::Io)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(FetchError::Io)?;
        written += n as u64;
        if show_progress {
            pb.set_position(written);
        }
    }
    if show_progress {
        pb.finish_and_clear();
    }
    Ok(written)
}

pub fn fetch_to_file(url: &str, dest: &Path) -> Result<u64, CliError> {
    let mut last: Option<CliError> = None;
    for attempt in 0..2 {
        match fetch_once(url, dest) {
            Ok(n) => return Ok(n),
            Err(FetchError::Status(code)) if code >= 500 && attempt == 0 => {
                last = Some(CliError::Network(format!("http status {code} (retrying)")));
            }
            Err(FetchError::Status(code)) => {
                return Err(CliError::Network(format!("http status {code}")));
            }
            Err(FetchError::Transport(t)) if attempt == 0 => {
                last = Some(CliError::Network(format!("transport error (retrying): {t}")));
            }
            Err(FetchError::Transport(t)) => {
                return Err(CliError::Network(format!("transport error: {t}")));
            }
            Err(FetchError::Io(e)) => return Err(CliError::Network(format!("write error: {e}"))),
        }
    }
    Err(last.unwrap_or_else(|| CliError::Network("download failed".into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "libcli-dl-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn spawn_http(responses: Vec<(&'static str, &'static [u8])>) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/b.epub", listener.local_addr().unwrap().port());
        let handle = thread::spawn(move || {
            for (status_line, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let head = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body).unwrap();
            }
        });
        (url, handle)
    }

    fn book(title: &str, authors: &[&str]) -> Book {
        Book {
            id: "1".into(),
            title: title.into(),
            authors: authors.iter().map(|a| a.to_string()).collect(),
            languages: vec![],
            published: None,
            description: None,
            categories: vec![],
            formats: vec![],
        }
    }

    #[test]
    fn fetch_success_writes_file() {
        let dir = temp_dir();
        let (url, handle) = spawn_http(vec![("200 OK", b"hello epub")]);
        let dest = dir.join("out.epub");
        let n = fetch_to_file(&url, &dest).unwrap();
        assert_eq!(n, 10);
        assert_eq!(fs::read(&dest).unwrap(), b"hello epub");
        handle.join().unwrap();
    }

    #[test]
    fn fetch_retries_once_on_5xx() {
        let dir = temp_dir();
        let (url, handle) = spawn_http(vec![("503 Service Unavailable", b""), ("200 OK", b"data")]);
        let dest = dir.join("out.epub");
        let n = fetch_to_file(&url, &dest).unwrap();
        assert_eq!(n, 4);
        assert_eq!(fs::read(&dest).unwrap(), b"data");
        handle.join().unwrap();
    }

    #[test]
    fn fetch_does_not_retry_404() {
        let dir = temp_dir();
        let (url, handle) = spawn_http(vec![("404 Not Found", b"")]);
        let dest = dir.join("out.epub");
        let err = fetch_to_file(&url, &dest).unwrap_err();
        assert_eq!(err.exit_code(), 3);
        assert!(err.to_string().contains("404"), "got: {err}");
        handle.join().unwrap();
    }

    #[test]
    fn connection_refused_is_network_error() {
        let dest = temp_dir().join("out.epub");
        let err = fetch_to_file("http://127.0.0.1:1/x.epub", &dest).unwrap_err();
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn overwrite_refused_without_force() {
        let dir = temp_dir();
        let f = dir.join("x.epub");
        File::create(&f).unwrap();
        let err = resolve_dest(&dir, "x.epub", false).unwrap_err();
        assert_eq!(err.exit_code(), 3);
        assert!(err.to_string().contains("--force"), "got: {err}");
        assert!(resolve_dest(&dir, "x.epub", true).is_ok());
    }

    #[test]
    fn filename_joins_title_and_authors() {
        assert_eq!(
            download_filename(&book("Moby Dick", &["Herman Melville"]), "epub"),
            "Moby Dick - Herman Melville.epub"
        );
        assert_eq!(
            download_filename(&book("Moby Dick", &["A", "B"]), "txt"),
            "Moby Dick - A, B.txt"
        );
        assert_eq!(download_filename(&book("Moby Dick", &[]), "txt"), "Moby Dick.txt");
    }

    #[test]
    fn sanitize_replaces_illegal_chars_and_collapses_space() {
        assert_eq!(sanitize_component("A/B:C*"), "A-B-C-");
        assert_eq!(sanitize_component("  a   b  "), "a b");
        assert_eq!(sanitize_component("Moby Dick; Or, The Whale"), "Moby Dick; Or, The Whale");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test download`
Expected: FAIL — module not found.

- [ ] **Step 3: Add `mod download;` to `src/main.rs` and run tests**

```rust
mod book;
mod config;
mod discovery;
mod download;
mod errors;
mod module;
```

Run: `cargo test download`
Expected: 7 tests PASS (retry and refused tests take ~1 s combined).

- [ ] **Step 4: Commit**

```bash
git add src/download.rs src/main.rs
git commit -m "feat: add host-owned downloader with retry and naming"
```

---

### Task 8: Output rendering

**Files:**
- Create: `src/output.rs`
- Modify: `src/main.rs` (add `mod output;`)

**Interfaces:**
- Consumes: `book::{Book, Category}`.
- Produces: `render_books_table(&[Book]) -> String` (columns TITLE / AUTHOR(S) / ID / FORMATS, aligned, header + `-----` separator, trailing newline; `"no results\n"` when empty); `render_books_json(&[Book]) -> String` (pretty JSON + newline); `render_book(&Book) -> String` (title, `by <authors>`, `published <year>`, `formats:` lines `  <tag> - <url>`); `render_categories(&[Category]) -> String` (`<title> - <id>` per line; `"no categories\n"` when empty).

- [ ] **Step 1: Write the failing test in `src/output.rs`**

```rust
use crate::book::{Book, Category};

pub fn render_books_table(books: &[Book]) -> String {
    if books.is_empty() {
        return "no results\n".to_string();
    }
    let cols = ["TITLE", "AUTHOR(S)", "ID", "FORMATS"];
    let rows: Vec<[String; 4]> = books
        .iter()
        .map(|b| {
            [
                b.title.clone(),
                b.authors.join(", "),
                b.id.clone(),
                b.formats.iter().map(|f| f.format.clone()).collect::<Vec<_>>().join(", "),
            ]
        })
        .collect();
    let mut widths = [0usize; 4];
    for i in 0..4 {
        widths[i] = cols[i].len();
    }
    for r in &rows {
        for i in 0..4 {
            widths[i] = widths[i].max(r[i].len());
        }
    }
    let row = |cells: &[String; 4]| -> String {
        (0..4)
            .map(|i| format!("{:<w$}", cells[i], w = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
            + "\n"
    };
    let mut out = String::new();
    out.push_str(&row(&[
        "TITLE".into(),
        "AUTHOR(S)".into(),
        "ID".into(),
        "FORMATS".into(),
    ]));
    out.push_str(&row(&[
        "-----".into(),
        "---------".into(),
        "--".into(),
        "-------".into(),
    ]));
    for r in &rows {
        out.push_str(&row(r));
    }
    out
}

pub fn render_books_json(books: &[Book]) -> String {
    serde_json::to_string_pretty(books).unwrap() + "\n"
}

pub fn render_book(book: &Book) -> String {
    let mut out = format!("{}\n", book.title);
    if !book.authors.is_empty() {
        out.push_str(&format!("by {}\n", book.authors.join(", ")));
    }
    if let Some(p) = book.published {
        out.push_str(&format!("published {p}\n"));
    }
    out.push_str("formats:\n");
    if book.formats.is_empty() {
        out.push_str("  (none)\n");
    }
    for f in &book.formats {
        out.push_str(&format!("  {} - {}\n", f.format, f.url));
    }
    out
}

pub fn render_categories(cats: &[Category]) -> String {
    if cats.is_empty() {
        return "no categories\n".to_string();
    }
    let mut out = String::new();
    for c in cats {
        out.push_str(&format!("{} - {}\n", c.title, c.id));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> Book {
        Book {
            id: "1".into(),
            title: "Moby Dick".into(),
            authors: vec!["Herman Melville".into()],
            languages: vec![],
            published: Some(1851),
            description: None,
            categories: vec![],
            formats: vec![
                crate::book::Format { format: "epub".into(), url: "https://x/1.epub".into(), size: None },
                crate::book::Format { format: "txt".into(), url: "https://x/1.txt".into(), size: None },
            ],
        }
    }

    #[test]
    fn table_has_header_and_rows() {
        let out = render_books_table(&[book()]);
        assert!(out.contains("TITLE"));
        assert!(out.contains("Moby Dick"));
        assert!(out.contains("Herman Melville"));
        assert!(out.contains("epub, txt"));
    }

    #[test]
    fn empty_table_says_no_results() {
        assert_eq!(render_books_table(&[]), "no results\n");
    }

    #[test]
    fn json_output_is_parseable() {
        let out = render_books_json(&[book()]);
        let parsed: Vec<Book> = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed[0].title, "Moby Dick");
    }

    #[test]
    fn book_details_include_formats() {
        let out = render_book(&book());
        assert!(out.contains("Moby Dick"));
        assert!(out.contains("by Herman Melville"));
        assert!(out.contains("published 1851"));
        assert!(out.contains("  epub - https://x/1.epub"));
    }

    #[test]
    fn categories_render_title_and_id() {
        let out = render_categories(&[Category { id: "c1".into(), title: "New Releases".into() }]);
        assert!(out.contains("New Releases - c1"));
        assert_eq!(render_categories(&[]), "no categories\n");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test output`
Expected: FAIL — module not found.

- [ ] **Step 3: Add `mod output;` to `src/main.rs` and run tests**

```rust
mod book;
mod config;
mod discovery;
mod download;
mod errors;
mod module;
mod output;
```

Run: `cargo test output`
Expected: 5 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/output.rs src/main.rs
git commit -m "feat: add table and JSON output rendering"
```

---

### Task 9: CLI wiring — verbs, module resolution, exit codes

**Files:**
- Create: `src/cli.rs`
- Rewrite: `src/main.rs`
- Create: `tests/integration.rs`

**Interfaces:**
- Consumes: all prior modules. Uses `ModuleHost::new(entry.name.clone(), &m.command, entry.dir.clone())` where `m = entry.manifest.as_ref().unwrap()` (safe after `resolve_module`).
- Produces: `cli::Cli` (clap Parser) + `cli::Command` (Subcommand) with verbs `Modules`, `Install { module_dir: PathBuf, force: bool }`, `Search { query, module: Option<String>, limit: u16, json: bool }`, `Categories { module }`, `List { category, module, limit }`, `Show { book_id, module }`, `Download { book_id, format, module, dir: Option<PathBuf>, force }`. `main()` maps clap errors (usage → 1, help/version → 0), runs dispatch, prints `error: <msg>` to stderr and exits with `CliError::exit_code()`. Dispatch: `Search` validates non-empty query and limit 1..=200 (else Usage), resolves module, calls `search` with `{"query": q, "limit": n}`, deserializes `BooksResponse`, renders table or `--json`. `Categories` → `categories` → `CategoriesResponse` → `render_categories`. `List` → validates `--category` non-empty and limit, calls `list` with `{"category": c, "limit": n}` → table. `Show` → `book` → `BookResponse` → `render_book`. `Download` → `book` → find format (else Usage listing available), dest dir = `--dir` > config `download_dir` (expanded) > `.`, `download_filename` + `resolve_dest(force)` + `fetch_to_file`, print `downloaded <n> bytes to <path>`.

- [ ] **Step 1: Create `src/cli.rs`**

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "library-cli", version, about = "Access open ebook libraries through pluggable modules")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// List installed modules and their capabilities
    Modules,
    /// Install a module from a directory into the discovery path
    Install {
        /// Path to the module directory (must contain manifest.toml)
        module_dir: PathBuf,
        /// Overwrite an existing module with the same name
        #[arg(long)]
        force: bool,
    },
    /// Search a library for books
    Search {
        query: String,
        #[arg(long)]
        module: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: u16,
        /// Emit normalized Book records as JSON
        #[arg(long)]
        json: bool,
    },
    /// List browsable categories/collections of a library
    Categories {
        #[arg(long)]
        module: Option<String>,
    },
    /// Browse a library's category
    List {
        #[arg(long)]
        category: String,
        #[arg(long)]
        module: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: u16,
    },
    /// Show details and available formats for a book
    Show {
        book_id: String,
        #[arg(long)]
        module: Option<String>,
    },
    /// Download a book in a given format
    Download {
        book_id: String,
        #[arg(long)]
        format: String,
        #[arg(long)]
        module: Option<String>,
        /// Destination directory (default: config download_dir or .)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Overwrite an existing file
        #[arg(long)]
        force: bool,
    },
}
```

- [ ] **Step 2: Rewrite `src/main.rs`**

```rust
mod book;
mod cli;
mod config;
mod discovery;
mod download;
mod errors;
mod module;
mod output;

use clap::Parser;
use errors::CliError;
use serde_json::json;

fn main() {
    let cli = match cli::Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let code = if e.use_stderr() { 1 } else { 0 };
            let _ = e.print();
            std::process::exit(code);
        }
    };
    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(e.exit_code());
    }
}

fn run(cli: cli::Cli) -> Result<(), CliError> {
    let cfg = config::load_config()?;
    let mods_dir = config::modules_dir()?;
    let entries = discovery::discover(&mods_dir);
    match cli.cmd {
        cli::Command::Modules => print_entries(&entries),
        cli::Command::Install { module_dir, force } => {
            install_module(&module_dir, &mods_dir, force)?
        }
        cli::Command::Search { query, module, limit, json } => {
            if query.trim().is_empty() {
                return Err(CliError::Usage("search query must not be empty".into()));
            }
            let limit = validate_limit(limit)?;
            let entry = resolve(&entries, module, &cfg)?;
            let host = host_for(entry);
            let result = host.call("search", json!({"query": query, "limit": limit}), 1)?;
            let resp: book::BooksResponse = serde_json::from_value(result)?;
            if json {
                print!("{}", output::render_books_json(&resp.books));
            } else {
                print!("{}", output::render_books_table(&resp.books));
            }
        }
        cli::Command::Categories { module } => {
            let entry = resolve(&entries, module, &cfg)?;
            let host = host_for(entry);
            let result = host.call("categories", json!({}), 1)?;
            let resp: book::CategoriesResponse = serde_json::from_value(result)?;
            print!("{}", output::render_categories(&resp.categories));
        }
        cli::Command::List { category, module, limit } => {
            if category.trim().is_empty() {
                return Err(CliError::Usage("--category must not be empty".into()));
            }
            let limit = validate_limit(limit)?;
            let entry = resolve(&entries, module, &cfg)?;
            let host = host_for(entry);
            let result = host.call("list", json!({"category": category, "limit": limit}), 1)?;
            let resp: book::BooksResponse = serde_json::from_value(result)?;
            print!("{}", output::render_books_table(&resp.books));
        }
        cli::Command::Show { book_id, module } => {
            let entry = resolve(&entries, module, &cfg)?;
            let host = host_for(entry);
            let result = host.call("book", json!({"id": book_id}), 1)?;
            let resp: book::BookResponse = serde_json::from_value(result)?;
            print!("{}", output::render_book(&resp.book));
        }
        cli::Command::Download { book_id, format, module, dir, force } => {
            let entry = resolve(&entries, module, &cfg)?;
            let host = host_for(entry);
            let result = host.call("book", json!({"id": book_id}), 1)?;
            let resp: book::BookResponse = serde_json::from_value(result)?;
            let book = resp.book;
            let fmt = book.formats.iter().find(|f| f.format == format).ok_or_else(|| {
                let avail: Vec<&str> = book.formats.iter().map(|f| f.format.as_str()).collect();
                CliError::Usage(format!(
                    "book {} has no format {format:?}; available: {}",
                    book.id,
                    if avail.is_empty() { "(none)".to_string() } else { avail.join(", ") }
                ))
            })?;
            let dest_dir = dir.unwrap_or_else(|| {
                cfg.expanded_download_dir()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
            });
            let filename = download::download_filename(&book, &format);
            let dest = download::resolve_dest(&dest_dir, &filename, force)?;
            let bytes = download::fetch_to_file(&fmt.url, &dest)?;
            println!("downloaded {bytes} bytes to {}", dest.display());
        }
    }
    Ok(())
}

fn validate_limit(limit: u16) -> Result<u16, CliError> {
    if limit == 0 || limit > 200 {
        return Err(CliError::Usage("--limit must be between 1 and 200".into()));
    }
    Ok(limit)
}

fn resolve<'a>(
    entries: &'a [discovery::ModuleEntry],
    flag: Option<String>,
    cfg: &config::Config,
) -> Result<&'a discovery::ModuleEntry, CliError> {
    let name = match flag {
        Some(n) => n,
        None => match &cfg.default_module {
            Some(n) => n.clone(),
            None => {
                let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
                let list = if names.is_empty() {
                    "(none)".to_string()
                } else {
                    names.join(", ")
                };
                return Err(CliError::Usage(format!(
                    "no module selected; pass --module or set default_module in config (installed: {list})"
                )));
            }
        },
    };
    discovery::resolve_module(entries, &name)
}

fn host_for(entry: &discovery::ModuleEntry) -> module::ModuleHost {
    let m = entry.manifest.as_ref().expect("resolve_module guarantees manifest");
    module::ModuleHost::new(entry.name.clone(), &m.command, entry.dir.clone())
}

fn print_entries(entries: &[discovery::ModuleEntry]) {
    if entries.is_empty() {
        println!("no modules installed");
        return;
    }
    for e in entries {
        match &e.manifest {
            Some(m) => println!(
                "{} v{} - {} [{}]",
                m.name,
                m.version,
                m.description.as_deref().unwrap_or(""),
                m.capabilities.join(", ")
            ),
            None => println!(
                "{} - BROKEN: {}",
                e.name,
                e.error.as_deref().unwrap_or("invalid manifest")
            ),
        }
    }
}

fn install_module(module_dir: &std::path::Path, mods_dir: &std::path::Path, force: bool) -> Result<(), CliError> {
    let manifest = discovery::load_manifest(module_dir)
        .map_err(|e| CliError::Config(format!("cannot install: {e}")))?;
    let target = mods_dir.join(&manifest.name);
    if target.exists() && !force {
        return Err(CliError::Usage(format!(
            "module {} already installed; use --force to overwrite",
            manifest.name
        )));
    }
    std::fs::create_dir_all(mods_dir)
        .map_err(|e| CliError::Network(format!("cannot create {}: {e}", mods_dir.display())))?;
    copy_dir(module_dir, &target)?;
    println!("installed module {} from {}", manifest.name, module_dir.display());
    Ok(())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<(), CliError> {
    std::fs::create_dir_all(dst)
        .map_err(|e| CliError::Network(format!("copy: {e}")))?;
    for e in std::fs::read_dir(src).map_err(|e| CliError::Network(format!("copy: {e}")))? {
        let e = e.map_err(|e| CliError::Network(format!("copy: {e}")))?;
        let src_p = e.path();
        let dst_p = dst.join(e.file_name());
        let ft = e.file_type().map_err(|e| CliError::Network(format!("copy: {e}")))?;
        if ft.is_dir() {
            copy_dir(&src_p, &dst_p)?;
        } else {
            std::fs::copy(&src_p, &dst_p)
                .map_err(|e| CliError::Network(format!("copy: {e}")))?;
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Create `tests/integration.rs`** (fake-module flows; `Install` verb tests arrive in Task 10)

```rust
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
```

- [ ] **Step 4: Run the full suite**

Run: `cargo test`
Expected: all unit tests + 12 integration tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs tests/integration.rs
git commit -m "feat: wire CLI verbs, module resolution, and exit codes"
```

---

### Task 10: `install` verb end-to-end

**Files:**
- Modify: `tests/integration.rs` (append tests)
- No new source: `install_module` already exists in `src/main.rs` (Task 9).

**Interfaces:**
- Consumes: `install_module` from Task 9; fixture module at `tests/fixtures/installable/`.
- Produces: nothing new. Tests assert copy semantics and `--force` behavior.

- [ ] **Step 1: Append the failing tests to `tests/integration.rs`**

```rust
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
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --test integration install_`
Expected: 2 tests PASS.

- [ ] **Step 3: Run the full suite**

Run: `cargo test`
Expected: everything PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/integration.rs
git commit -m "feat: verify install verb copies and guards overwrites"
```

---

### Task 11: Gutenberg module

**Files:**
- Create: `modules/gutenberg/manifest.toml`
- Create: `modules/gutenberg/module.py`
- Create: `modules/gutenberg/fixtures/search-moby-dick.json`
- Create: `modules/gutenberg/fixtures/book-2701.json`
- Modify: `tests/integration.rs` (append tests)

**Interfaces:**
- Consumes: host protocol from Task 6; none of the Rust modules directly.
- Produces: a module dir consumable by `discover`/`ModuleHost`: capabilities `["search", "book"]`; `search` maps gutendex `results` → Books (`id` = stringified numeric id, `authors` from `authors[].name`, `languages`, `categories` = `subjects`, formats via MIME prefix map `text/plain`→txt, `application/epub+zip`→epub, `application/x-mobipocket-ebook`→mobi, `text/html`→html; `total` = `count`); `book` maps a single gutendex result. Fixture mode honors `LIBRARY_CLI_FIXTURE` per Global Constraints.

- [ ] **Step 1: Create `modules/gutenberg/manifest.toml`**

```toml
name = "gutenberg"
version = "0.1.0"
description = "Project Gutenberg via gutendex"
command = ["python3", "module.py"]
capabilities = ["search", "book"]
```

- [ ] **Step 2: Create `modules/gutenberg/module.py`**

```python
#!/usr/bin/env python3
"""Project Gutenberg module for library-cli.

Speaks JSON-RPC 2.0 (one JSON object per line) over stdio.
When LIBRARY_CLI_FIXTURE is set, answers from recorded gutendex JSON files
(<fixture>/search-<slug>.json, <fixture>/book-<id>.json) instead of the network.
"""
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

API = "https://gutendex.com/books"

MIME_PREFIXES = [
    ("text/plain", "txt"),
    ("application/epub+zip", "epub"),
    ("application/x-mobipocket-ebook", "mobi"),
    ("text/html", "html"),
]


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def key_for(s):
    return s.rsplit("/", 1)[-1]


def fixture_path(method, key):
    root = os.environ.get("LIBRARY_CLI_FIXTURE")
    if not root:
        return None
    k = slug(key) if key else ""
    return os.path.join(root, f"{method}{'-' + k if k else ''}.json")


def load(method, key, url):
    path = fixture_path(method, key)
    if path:
        if not os.path.isfile(path):
            raise RuntimeError(f"fixture not found: {path}")
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"gutendex http {e.code}: {url}") from e
    except urllib.error.URLError as e:
        raise RuntimeError(f"gutendex network error: {e.reason}") from e


def to_book(g):
    formats = []
    for mime, url in (g.get("formats") or {}).items():
        for prefix, tag in MIME_PREFIXES:
            if mime.startswith(prefix):
                formats.append({"format": tag, "url": url})
                break
    return {
        "id": str(g["id"]),
        "title": g.get("title", ""),
        "authors": [a.get("name", "") for a in g.get("authors", []) if a.get("name")],
        "languages": g.get("languages", []),
        "categories": g.get("subjects", []),
        "formats": formats,
    }


def search(params):
    query = params.get("query", "")
    if not query:
        raise RuntimeError("missing query param")
    page = params.get("page", 1)
    limit = params.get("limit", 20)
    url = f"{API}/?search={urllib.parse.quote(query)}&page={page}"
    data = load("search", query, url)
    books = [to_book(g) for g in data.get("results", [])][:limit]
    return {"books": books, "total": data.get("count")}


def book(params):
    bid = params.get("id", "")
    data = load("book", key_for(bid), f"{API}/{urllib.parse.quote(bid)}")
    return {"book": to_book(data)}


def main():
    line = sys.stdin.readline()
    if not line:
        sys.stderr.write("gutenberg: no request on stdin\n")
        sys.exit(1)
    try:
        req = json.loads(line)
        method = req["method"]
        params = req.get("params") or {}
        if method == "search":
            result = search(params)
        elif method == "book":
            result = book(params)
        else:
            sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"],
                "error": {"code": -32601, "message": f"method not found: {method}"}}) + "\n")
            return
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": result}) + "\n")
    except RuntimeError as e:
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req.get("id", 0),
            "error": {"code": -32000, "message": str(e)}}) + "\n")
        sys.exit(0)
    except (json.JSONDecodeError, KeyError, TypeError, ValueError) as e:
        sys.stderr.write(f"gutenberg: bad request: {e}\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: Create the fixtures**

`modules/gutenberg/fixtures/search-moby-dick.json` (recorded gutendex shape; exact values refreshed in Step 6):

```json
{
  "count": 1,
  "next": null,
  "previous": null,
  "results": [
    {
      "id": 2701,
      "title": "Moby Dick; Or, The Whale",
      "authors": [{"name": "Melville, Herman"}],
      "translators": [],
      "subjects": ["Whaling -- Fiction", "Sea stories", "Psychological fiction"],
      "bookshelves": ["Best Books Ever Listing"],
      "languages": ["en"],
      "copyright": false,
      "media_type": "Text",
      "formats": {
        "text/plain; charset=us-ascii": "https://www.gutenberg.org/ebooks/2701.txt.utf-8",
        "application/epub+zip": "https://www.gutenberg.org/ebooks/2701.epub.noimages",
        "application/x-mobipocket-ebook": "https://www.gutenberg.org/ebooks/2701.kf8.images",
        "text/html; charset=utf-8": "https://www.gutenberg.org/ebooks/2701-h/2701-h.htm"
      },
      "download_count": 32000
    }
  ]
}
```

`modules/gutenberg/fixtures/book-2701.json` (single gutendex result):

```json
{
  "id": 2701,
  "title": "Moby Dick; Or, The Whale",
  "authors": [{"name": "Melville, Herman"}],
  "translators": [],
  "subjects": ["Whaling -- Fiction", "Sea stories", "Psychological fiction"],
  "bookshelves": ["Best Books Ever Listing"],
  "languages": ["en"],
  "copyright": false,
  "media_type": "Text",
  "formats": {
    "text/plain; charset=us-ascii": "https://www.gutenberg.org/ebooks/2701.txt.utf-8",
    "application/epub+zip": "https://www.gutenberg.org/ebooks/2701.epub.noimages",
    "application/x-mobipocket-ebook": "https://www.gutenberg.org/ebooks/2701.kf8.images",
    "text/html; charset=utf-8": "https://www.gutenberg.org/ebooks/2701-h/2701-h.htm"
  },
  "download_count": 32000
}
```

- [ ] **Step 4: Add the failing integration tests to `tests/integration.rs`**

```rust
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
```

- [ ] **Step 5: Run the tests**

Run: `cargo test --test integration gutenberg_`
Expected: 3 tests PASS against the committed fixtures.

- [ ] **Step 6: Verify against the live API and refresh fixtures if needed**

Run:
```bash
curl -s "https://gutendex.com/books/?search=moby%20dick" | head -c 3000
curl -s "https://gutendex.com/books/2701" | head -c 3000
```

Compare the shapes with the fixtures. The `module.py` mapping logic is the source of truth; if gutendex changed field names/MIME keys, update `module.py` (and its tests) to match, and refresh the fixture files from the live response. Then re-run `cargo test --test integration gutenberg_`. If the network is unavailable, skip this step (fixtures remain the test contract) and note it in the commit message.

Also verify the module standalone (documented self-test one-liner):

Run: `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"search","params":{"query":"moby dick","limit":5}}' | LIBRARY_CLI_FIXTURE=modules/gutenberg/fixtures python3 modules/gutenberg/module.py`
Expected: one line of JSON with `"result"` containing `"Moby Dick; Or, The Whale"`.

- [ ] **Step 7: Commit**

```bash
git add modules/gutenberg tests/integration.rs
git commit -m "feat: add gutenberg module with fixtures and tests"
```

---

### Task 12: Standard Ebooks module

**Files:**
- Create: `modules/standard-ebooks/manifest.toml`
- Create: `modules/standard-ebooks/module.py`
- Create: `modules/standard-ebooks/fixtures/categories.json`
- Create: `modules/standard-ebooks/fixtures/list-new-releases.json`
- Create: `modules/standard-ebooks/fixtures/search-moby-dick.json`
- Create: `modules/standard-ebooks/fixtures/book-frankenstein.json`
- Modify: `tests/integration.rs` (append tests)

**Interfaces:**
- Consumes: host protocol; OPDS feeds via stdlib `xml.etree.ElementTree`, `html.parser` for the HTML fallback.
- Produces: module dir with capabilities `["search", "categories", "list", "book"]`. `categories` → nav entries (rel `subsection`) of `https://standardebooks.org/feeds/opds/all`, `{id: href, title}`. `list {category}` → fetch the category href (an acquisition feed), entries → Books. `search` → `https://standardebooks.org/feeds/opds/search?query={searchTerms}` acquisition feed. `book {id}` → fetch the id URL (id = entry permalink) with `Accept: application/atom+xml`; parse entry if the response is OPDS, else scan HTML `<link rel="alternate">` tags for download links. MIME map: `application/epub+zip`→epub, `application/vnd.amazon.ebook`→azw3, `application/x-kepub+zip`→kepub. Book `published` = year from `dcterms:issued`.

- [ ] **Step 1: Create `modules/standard-ebooks/manifest.toml`**

```toml
name = "standard-ebooks"
version = "0.1.0"
description = "Standard Ebooks via OPDS catalog"
command = ["python3", "module.py"]
capabilities = ["search", "categories", "list", "book"]
```

- [ ] **Step 2: Create `modules/standard-ebooks/module.py`**

```python
#!/usr/bin/env python3
"""Standard Ebooks module for library-cli.

Speaks JSON-RPC 2.0 (one JSON object per line) over stdio.
When LIBRARY_CLI_FIXTURE is set, answers from recorded files
(<fixture>/<method>-<slug>.json) instead of the network.
"""
import html.parser
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

ATOM = "{http://www.w3.org/2005/Atom}"
DC = "{http://purl.org/dc/terms/}"
ROOT = "https://standardebooks.org/feeds/opds"
SEARCH_TEMPLATE = ROOT + "/search?query={searchTerms}"

MIME_TO_FORMAT = {
    "application/epub+zip": "epub",
    "application/vnd.amazon.ebook": "azw3",
    "application/x-kepub+zip": "kepub",
}


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def key_for(s):
    return s.rsplit("/", 1)[-1]


def fixture_path(method, key):
    root = os.environ.get("LIBRARY_CLI_FIXTURE")
    if not root:
        return None
    k = slug(key) if key else ""
    return os.path.join(root, f"{method}{'-' + k if k else ''}.json")


def http_bytes(url, accept=None):
    req = urllib.request.Request(url, headers={"Accept": accept} if accept else {})
    with urllib.request.urlopen(req, timeout=30) as resp:
        return resp.read()


def load_xml(method, key, url):
    path = fixture_path(method, key)
    if path:
        if not os.path.isfile(path):
            raise RuntimeError(f"fixture not found: {path}")
        with open(path, "rb") as f:
            return ET.fromstring(f.read())
    try:
        return ET.fromstring(http_bytes(url, "application/atom+xml"))
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"standard-ebooks http {e.code}: {url}") from e
    except urllib.error.URLError as e:
        raise RuntimeError(f"standard-ebooks network error: {e.reason}") from e


def text_of(parent, tag):
    el = parent.find(tag)
    return el.text if el is not None and el.text else ""


def to_book(entry):
    formats = []
    for link in entry.findall(ATOM + "link"):
        if "acquisition" not in link.get("rel", ""):
            continue
        tag = MIME_TO_FORMAT.get(link.get("type", ""))
        if tag:
            formats.append({"format": tag, "url": link.get("href", "")})
    issued = text_of(entry, DC + "issued")
    published = int(issued[:4]) if issued[:4].isdigit() else None
    return {
        "id": text_of(entry, ATOM + "id"),
        "title": text_of(entry, ATOM + "title"),
        "authors": [a.text for a in entry.findall(ATOM + "author/" + ATOM + "name") if a.text],
        "languages": [text_of(entry, DC + "language")] if text_of(entry, DC + "language") else [],
        "published": published,
        "description": text_of(entry, DC + "description") or None,
        "categories": [c.get("term") for c in entry.findall(ATOM + "category") if c.get("term")],
        "formats": formats,
    }


def categories():
    feed = load_xml("categories", "", ROOT + "/all")
    cats = []
    for entry in feed.findall(ATOM + "entry"):
        title = text_of(entry, ATOM + "title")
        for link in entry.findall(ATOM + "link"):
            if link.get("rel") == "subsection":
                cats.append({"id": link.get("href", ""), "title": title})
    return {"categories": cats}


def list_books(params):
    category = params.get("category", "")
    if not category:
        raise RuntimeError("missing category param")
    limit = params.get("limit", 20)
    feed = load_xml("list", key_for(category), category)
    books = [to_book(e) for e in feed.findall(ATOM + "entry")][:limit]
    return {"books": books}


def search(params):
    query = params.get("query", "")
    if not query:
        raise RuntimeError("missing query param")
    limit = params.get("limit", 20)
    url = SEARCH_TEMPLATE.replace("{searchTerms}", urllib.parse.quote(query))
    feed = load_xml("search", query, url)
    books = [to_book(e) for e in feed.findall(ATOM + "entry")][:limit]
    return {"books": books}


class _LinkParser(html.parser.HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []

    def handle_starttag(self, tag, attrs):
        if tag == "link":
            d = dict(attrs)
            if d.get("rel") == "alternate" and d.get("type") in MIME_TO_FORMAT:
                self.links.append((d["type"], d.get("href", "")))


def book(params):
    bid = params.get("id", "")
    if not (bid.startswith("http://") or bid.startswith("https://")):
        raise RuntimeError(f"invalid book id: {bid}")
    path = fixture_path("book", key_for(bid))
    if path:
        with open(path, "rb") as f:
            raw = f.read()
    else:
        try:
            raw = http_bytes(bid, "application/atom+xml")
        except urllib.error.HTTPError as e:
            raise RuntimeError(f"standard-ebooks http {e.code}: {bid}") from e
        except urllib.error.URLError as e:
            raise RuntimeError(f"standard-ebooks network error: {e.reason}") from e
    if b"<entry" in raw:
        feed = ET.fromstring(raw)
        entries = feed.findall(ATOM + "entry")
        if entries:
            return {"book": to_book(entries[0])}
    parser = _LinkParser()
    parser.feed(raw.decode("utf-8", errors="replace"))
    formats = [{"format": MIME_TO_FORMAT[t], "url": h} for t, h in parser.links]
    return {"book": {"id": bid, "title": key_for(bid), "authors": [], "formats": formats}}


def main():
    line = sys.stdin.readline()
    if not line:
        sys.stderr.write("standard-ebooks: no request on stdin\n")
        sys.exit(1)
    try:
        req = json.loads(line)
        method = req["method"]
        params = req.get("params") or {}
        if method == "search":
            result = search(params)
        elif method == "categories":
            result = categories()
        elif method == "list":
            result = list_books(params)
        elif method == "book":
            result = book(params)
        else:
            sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"],
                "error": {"code": -32601, "message": f"method not found: {method}"}}) + "\n")
            return
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": result}) + "\n")
    except RuntimeError as e:
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req.get("id", 0),
            "error": {"code": -32000, "message": str(e)}}) + "\n")
        sys.exit(0)
    except (json.JSONDecodeError, KeyError, TypeError, ValueError) as e:
        sys.stderr.write(f"standard-ebooks: bad request: {e}\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: Create the fixtures**

`modules/standard-ebooks/fixtures/categories.json` (nav feed; refresh from live in Step 6):

```xml
<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <title>Standard Ebooks</title>
  <id>https://standardebooks.org/feeds/opds/all</id>
  <updated>2026-01-01T00:00:00Z</updated>
  <entry>
    <title>New Releases</title>
    <id>https://standardebooks.org/feeds/opds/new-releases</id>
    <updated>2026-01-01T00:00:00Z</updated>
    <link rel="subsection" type="application/atom+xml;profile=opds-catalog;kind=acquisition" href="https://standardebooks.org/feeds/opds/new-releases"/>
  </entry>
  <entry>
    <title>Science Fiction</title>
    <id>https://standardebooks.org/feeds/opds/science-fiction</id>
    <updated>2026-01-01T00:00:00Z</updated>
    <link rel="subsection" type="application/atom+xml;profile=opds-catalog;kind=acquisition" href="https://standardebooks.org/feeds/opds/science-fiction"/>
  </entry>
</feed>
```

`modules/standard-ebooks/fixtures/list-new-releases.json` (acquisition feed):

```xml
<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:opds="http://opds-spec.org/2010/catalog">
  <title>New Releases</title>
  <id>https://standardebooks.org/feeds/opds/new-releases</id>
  <updated>2026-01-01T00:00:00Z</updated>
  <entry>
    <title>Frankenstein; Or, The Modern Prometheus</title>
    <id>https://standardebooks.org/ebooks/mary-shelley/frankenstein</id>
    <updated>2026-01-01T00:00:00Z</updated>
    <author><name>Mary Wollstonecraft Shelley</name></author>
    <dcterms:language>en-US</dcterms:language>
    <dcterms:issued>2015-06-29</dcterms:issued>
    <category term="Science Fiction" label="Science Fiction"/>
    <link rel="alternate" type="text/html" href="https://standardebooks.org/ebooks/mary-shelley/frankenstein"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/epub+zip" href="https://standardebooks.org/ebooks/mary-shelley/frankenstein/downloads/mary-shelley_frankenstein.epub"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/vnd.amazon.ebook" href="https://standardebooks.org/ebooks/mary-shelley/frankenstein/downloads/mary-shelley_frankenstein.azw3"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/x-kepub+zip" href="https://standardebooks.org/ebooks/mary-shelley/frankenstein/downloads/mary-shelley_frankenstein.kepub.epub"/>
  </entry>
</feed>
```

`modules/standard-ebooks/fixtures/search-moby-dick.json` (search results acquisition feed):

```xml
<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:opds="http://opds-spec.org/2010/catalog">
  <title>Search: moby dick</title>
  <id>https://standardebooks.org/feeds/opds/search?query=moby%20dick</id>
  <updated>2026-01-01T00:00:00Z</updated>
  <entry>
    <title>Moby-Dick; or, The Whale</title>
    <id>https://standardebooks.org/ebooks/herman-melville/moby-dick</id>
    <updated>2026-01-01T00:00:00Z</updated>
    <author><name>Herman Melville</name></author>
    <dcterms:language>en-US</dcterms:language>
    <dcterms:issued>2013-06-14</dcterms:issued>
    <category term="Adventure" label="Adventure"/>
    <link rel="alternate" type="text/html" href="https://standardebooks.org/ebooks/herman-melville/moby-dick"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/epub+zip" href="https://standardebooks.org/ebooks/herman-melville/moby-dick/downloads/herman-melville_moby-dick.epub"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/vnd.amazon.ebook" href="https://standardebooks.org/ebooks/herman-melville/moby-dick/downloads/herman-melville_moby-dick.azw3"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/x-kepub+zip" href="https://standardebooks.org/ebooks/herman-melville/moby-dick/downloads/herman-melville_moby-dick.kepub.epub"/>
  </entry>
</feed>
```

`modules/standard-ebooks/fixtures/book-frankenstein.json` (single-entry feed; content negotiation for `book`):

```xml
<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:opds="http://opds-spec.org/2010/catalog">
  <title>Frankenstein; Or, The Modern Prometheus</title>
  <id>https://standardebooks.org/ebooks/mary-shelley/frankenstein</id>
  <updated>2026-01-01T00:00:00Z</updated>
  <entry>
    <title>Frankenstein; Or, The Modern Prometheus</title>
    <id>https://standardebooks.org/ebooks/mary-shelley/frankenstein</id>
    <updated>2026-01-01T00:00:00Z</updated>
    <author><name>Mary Wollstonecraft Shelley</name></author>
    <dcterms:language>en-US</dcterms:language>
    <dcterms:issued>2015-06-29</dcterms:issued>
    <category term="Science Fiction" label="Science Fiction"/>
    <link rel="alternate" type="text/html" href="https://standardebooks.org/ebooks/mary-shelley/frankenstein"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/epub+zip" href="https://standardebooks.org/ebooks/mary-shelley/frankenstein/downloads/mary-shelley_frankenstein.epub"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/vnd.amazon.ebook" href="https://standardebooks.org/ebooks/mary-shelley/frankenstein/downloads/mary-shelley_frankenstein.azw3"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/x-kepub+zip" href="https://standardebooks.org/ebooks/mary-shelley/frankenstein/downloads/mary-shelley_frankenstein.kepub.epub"/>
  </entry>
</feed>
```

- [ ] **Step 4: Add the failing integration tests to `tests/integration.rs`**

```rust
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
    assert_eq!(parsed[0]["title"], "Moby-Dick; or, The Whale");
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
```

- [ ] **Step 5: Run the tests**

Run: `cargo test --test integration standard_ebooks_`
Expected: 4 tests PASS against committed fixtures.

- [ ] **Step 6: Verify against the live API and refresh fixtures if needed**

Run:
```bash
curl -s https://standardebooks.org/feeds/opds/all | head -c 3000
curl -s "https://standardebooks.org/feeds/opds/search?query=moby%20dick" | head -c 3000
```

Compare with the fixtures; if the live feed shapes, MIME types, or nav `rel` values differ, update `module.py` (and its tests) and refresh the fixture files. Re-run `cargo test --test integration standard_ebooks_`. If the network is unavailable, skip and note it in the commit.

Also verify standalone:

Run: `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"categories","params":{}}' | LIBRARY_CLI_FIXTURE=modules/standard-ebooks/fixtures python3 modules/standard-ebooks/module.py`
Expected: one line of JSON with `"result"` containing `"New Releases"`.

- [ ] **Step 7: Commit**

```bash
git add modules/standard-ebooks tests/integration.rs
git commit -m "feat: add standard-ebooks module with fixtures and tests"
```

---

### Task 13: End-to-end download, live tests, README

**Files:**
- Modify: `tests/integration.rs` (append tests)
- Create: `README.md`

**Interfaces:**
- Consumes: everything from Tasks 1-12.
- Produces: hermetic end-to-end download proof (CLI → gutenberg module fixture → local HTTP server → file on disk), `#[ignore]`-gated live tests, and the README documenting usage, the module contract, and per-module self-test one-liners.

- [ ] **Step 1: Append the failing tests to `tests/integration.rs`**

```rust
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

// End-to-end: gutenberg module (fixture) returns a download URL pointing at
// a local server; the host must fetch and write the file.
#[test]
fn gutenberg_download_end_to_end() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = b"PK\x03\x04 fake epub bytes".to_vec();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
    });

    // Fixture override: point the book's txt URL at the local server.
    let tmp = std::env::temp_dir().join(format!(
        "libcli-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut fixture: serde_json::Value =
        serde_json::from_str(include_str!("../modules/gutenberg/fixtures/book-2701.json")).unwrap();
    fixture["formats"]["text/plain; charset=us-ascii"] =
        serde_json::Value::String(format!("http://127.0.0.1:{port}/2701.txt.utf-8"));
    std::fs::write(
        tmp.join("book-2701.json"),
        serde_json::to_string(&fixture).unwrap(),
    )
    .unwrap();

    let out_dir = tmp.join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = bin()
        .env("LIBRARY_CLI_MODULES", repo_modules())
        .env("LIBRARY_CLI_FIXTURE", &tmp)
        .args([
            "download", "2701", "--format", "txt", "--module", "gutenberg",
            "--dir", out_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("downloaded 20 bytes"), "got: {text}");
    let saved = out_dir.join("Moby Dick; Or, The Whale - Melville, Herman.txt");
    assert!(saved.exists(), "missing {saved:?}");
    assert_eq!(std::fs::read(&saved).unwrap(), body);
    handle.join().unwrap();
}

// Live tests: opt in with `cargo test -- --ignored`. Requires network.
#[test]
#[ignore = "live network test"]
fn gutenberg_search_live() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", repo_modules())
        .env_remove("LIBRARY_CLI_FIXTURE")
        .args(["search", "moby dick", "--module", "gutenberg", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(parsed.as_array().unwrap().iter().any(|b| b["title"].as_str().unwrap_or("").contains("Moby Dick")));
}

#[test]
#[ignore = "live network test"]
fn standard_ebooks_categories_live() {
    let out = bin()
        .env("LIBRARY_CLI_MODULES", repo_modules())
        .env_remove("LIBRARY_CLI_FIXTURE")
        .args(["categories", "--module", "standard-ebooks"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("New Releases"));
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test --test integration gutenberg_download_end_to_end`
Expected: 1 test PASS.

Run: `cargo test --test integration live`
Expected: 0 tests run (both live tests are `#[ignore]`-gated and skipped by default).

- [ ] **Step 3: Run the full suite**

Run: `cargo test`
Expected: all unit tests and all integration tests PASS. Run live tests once manually with network: `cargo test -- --ignored` (both should pass against real gutendex/OPDS; if a live shape differs, fix per Task 11/12 Step 6 discipline).

- [ ] **Step 4: Create `README.md`**

```markdown
# library-cli

Access open ebook libraries from the command line. The core is a small Rust
binary; each library is a *module* — a standalone program (any language) that
speaks JSON-RPC over stdio and returns normalized book records.

## Build & run

Requires Rust and `python3` (>= 3.8).

```bash
cargo build --release
```

Modules live in `~/.config/library-cli/modules/<name>/` (override with the
`LIBRARY_CLI_MODULES` env var). Install from a directory:

```bash
library-cli install ./modules/gutenberg
library-cli install ./modules/standard-ebooks
```

## Usage

```bash
library-cli modules                       # list installed modules
library-cli search "moby dick" --module gutenberg
library-cli categories --module standard-ebooks
library-cli list --category <feed-url> --module standard-ebooks
library-cli show 2701 --module gutenberg
library-cli download 2701 --format epub --module gutenberg
library-cli download <book-id> --format epub --dir ~/Downloads
```

Default module and download dir go in `~/.config/library-cli/config.toml`:

```toml
default_module = "gutenberg"
download_dir = "~/Downloads"
```

## Module contract (short version)

- Manifest `manifest.toml` in a module dir: `name` (== dir name), `version`,
  `command` (argv; first element resolved relative to the dir if it names a
  file there), `capabilities` (subset of `search`, `categories`, `list`,
  `book`).
- One JSON-RPC 2.0 request object per line on stdin; one response object per
  line on stdout; exit 0. Nothing else on stdout (log to stderr).
- Methods: `search {query, page?, limit?}` → `{books, total?}`;
  `categories` → `{categories: [{id, title}]}`;
  `list {category, page?, limit?}` → `{books, total?}`;
  `book {id}` → `{book}`.
- Book: `{id, title, authors[], languages[], published?, description?,
  categories[], formats: [{format, url, size?}]}` with format tags
  `epub azw3 mobi kepub txt html`.
- Test a module by hand:
  `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"search","params":{"query":"moby dick"}}' | python3 modules/gutenberg/module.py`
- Fixture mode: set `LIBRARY_CLI_FIXTURE=<dir>` to serve recorded responses
  (`<method>-<slug>.json`) instead of the network.

Exit codes: 0 ok, 1 usage, 2 module/protocol, 3 network/IO, 4 config/discovery.
```

- [ ] **Step 5: Commit**

```bash
git add tests/integration.rs README.md
git commit -m "feat: end-to-end download proof, live tests, README"
```




