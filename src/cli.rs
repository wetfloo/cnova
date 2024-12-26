use std::path::PathBuf;

use clap::{crate_name, value_parser, Parser, ValueEnum};
use reqwest::Proxy;

#[derive(Debug, Parser)]
#[command(name = crate_name!(), version, about)]
pub struct Cli {
    /// Paths to scan. Could be a mix files or directories. If it's a directory, this program will
    /// traverse it recursively and download .lrc files, reporting any errors along the way. If it's
    /// a file, will download a corresponding .lrc file for it
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,

    /// Don't ignore hidden files and directories
    #[arg(short = 'i', long)]
    pub no_ignore_hidden: bool,

    /// Don't follow symlinks
    #[arg(short = 's', long)]
    pub no_follow_symlinks: bool,

    /// .lrc file acquisition behavior
    #[arg(short = 'l', long, value_enum, default_value_t = LrcAcquireBehavior::LrcMissing)]
    pub lrc_acquire_behavior: LrcAcquireBehavior,

    /// Allows the program to create .nolrc files, in order to prevent requesting lyrics from the
    /// same songs in the future, making the process faster if you keep a large library. As a
    /// downside, you get a lot of empty .nolrc files
    #[arg(long)]
    pub deny_nolrc: bool,

    /// File matching strictness level
    #[arg(long, value_enum, default_value_t = FileMatchStrictness::FilterByExt)]
    pub strictness: FileMatchStrictness,

    /// How many simultaneous downloads will occur at the same time. The default value is selected
    /// to not, hopefully, overwhelm the website with traffic
    #[arg(
        short = 'j',
        long,
        default_value_t = 5,
        value_parser = value_parser!(u16).range(1..),
    )]
    pub download_jobs: u16,

    /// How many threads will be spawn to process the files. 0 corresponds to the amount of
    /// available system threads
    #[arg(short = 'J', long, default_value_t = 0)]
    pub traversal_jobs: u16,

    /// Proxy setting, supports SOCKS5, SOCKS4 and HTTP proxies
    #[arg(short, long, value_parser = proxy)]
    pub proxy: Option<reqwest::Proxy>,
}

fn proxy(s: &str) -> Result<Proxy, String> {
    Proxy::all(s).map_err(|_| "invalid proxy string".to_string())
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

impl Default for FileMatchStrictness {
    fn default() -> Self {
        Self::FilterByExt
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LrcAcquireBehavior {
    /// Attempt to download lyrics for every track, even if a corresponding .lrc or .nolrc is present
    All,
    /// Download for all tracks that have a corresponding .lrc file, excluding tracks that
    /// have a corresponding .nolrc file
    OverwriteExceptNolrc,
    /// Download for all tracks that don't have a corresponding .lrc file, including tracks that
    /// have a corresponding .nolrc file
    LrcMissingAll,
    /// Download for all tracks that don't have a corresponding .lrc file or .nolrc file
    LrcMissing,
}
