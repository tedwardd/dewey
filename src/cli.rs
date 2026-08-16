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
        #[arg(short = 'o', long)]
        dir: Option<PathBuf>,
        /// Overwrite an existing file
        #[arg(long)]
        force: bool,
    },
}
