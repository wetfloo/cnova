use std::path::PathBuf;

use clap::{crate_name, Parser, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = crate_name!(), version, about)]
pub struct Cli {
    /// Paths to scan. Could be a mix files or directories. If it's a directory, this program will
    /// traverse it recursively and download LRC files, reporting any errors along the way. If it's
    /// a file, will download a corresponding LRC file for it
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
    /// Ignore hidden files and directories
    #[arg(short = 'i', long, default_value_t = true)]
    pub ignore_hidden: bool,
    /// Follow symlinks
    #[arg(short = 's', long, default_value_t = true)]
    pub follow_symlinks: bool,
    /// If true, will attempt to re-download an existing LRC file
    #[arg(long, default_value_t = false)]
    pub overwrite_lrc_files: bool,
    /// File matching strictness level
    #[arg(long, value_enum, default_value_t = FileMatchStrictness::FilterByExt)]
    pub strictness: FileMatchStrictness,
    /// Proxy setting, supports SOCKS5, SOCKS4 and HTTP proxies
    #[arg(short, long)]
    pub proxy: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum FileMatchStrictness {
    /// Try to probe any file by it's extension, even if it doesn't match. Not recommended
    TrustyGuesser,
    /// Filter music files by extensions, trust the extensions
    FilterByExt,
    /// Don't trust file extensions, read directly into them. Might take process to read files
    Paranoid,
}
