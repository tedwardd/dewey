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
        // Canonicalize so command args resolve absolutely regardless of how
        // the modules dir was spelled (a relative DEWEY_MODULES would
        // otherwise leave relative script paths that the child resolves
        // against its own cwd).
        let dir = fs::canonicalize(e.path()).unwrap_or_else(|_| e.path());
        match load_manifest(&dir) {
            Ok(m) => out.push(ModuleEntry {
                name,
                dir,
                manifest: Some(m),
                error: None,
            }),
            Err(err) => out.push(ModuleEntry {
                name,
                dir,
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
