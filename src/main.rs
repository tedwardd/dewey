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
            require_capability(entry, "search")?;
            let host = host_for(entry);
            let result = host.call("search", json!({"query": query, "limit": limit}), 1)?;
            let resp: book::BooksResponse = serde_json::from_value(result)?;
            if json {
                print!("{}", output::render_books_json(&resp.books));
            } else {
                print!("{}", output::render_books_table(&resp.books));
                print!("{}", output::render_count_line(resp.total, resp.books.len()));
            }
        }
        cli::Command::Categories { module } => {
            let entry = resolve(&entries, module, &cfg)?;
            require_capability(entry, "categories")?;
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
            require_capability(entry, "list")?;
            let host = host_for(entry);
            let result = host.call("list", json!({"category": category, "limit": limit}), 1)?;
            let resp: book::BooksResponse = serde_json::from_value(result)?;
            print!("{}", output::render_books_table(&resp.books));
            print!("{}", output::render_count_line(resp.total, resp.books.len()));
        }
        cli::Command::Show { book_id, module } => {
            let entry = resolve(&entries, module, &cfg)?;
            require_capability(entry, "book")?;
            let host = host_for(entry);
            let result = host.call("book", json!({"id": book_id}), 1)?;
            let resp: book::BookResponse = serde_json::from_value(result)?;
            print!("{}", output::render_book(&resp.book));
        }
        cli::Command::Download { book_id, format, module, dir, force } => {
            let entry = resolve(&entries, module, &cfg)?;
            require_capability(entry, "book")?;
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

/// The host only surfaces verbs backed by a declared capability: refuse to
/// call a module for a method it does not advertise.
fn require_capability(entry: &discovery::ModuleEntry, method: &str) -> Result<(), CliError> {
    let m = entry.manifest.as_ref().expect("resolve_module guarantees manifest");
    if m.capabilities.iter().any(|c| c == method) {
        Ok(())
    } else {
        let caps = if m.capabilities.is_empty() {
            "(none)".to_string()
        } else {
            m.capabilities.join(", ")
        };
        Err(CliError::Usage(format!(
            "module {} does not support {method} (capabilities: {caps})",
            entry.name
        )))
    }
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
    if target.exists() {
        // force is implied here (non-force already returned above): clean the
        // target so the reinstall exactly mirrors the source, no stale files.
        std::fs::remove_dir_all(&target)
            .map_err(|e| CliError::Network(format!("cannot remove {}: {e}", target.display())))?;
    }
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
