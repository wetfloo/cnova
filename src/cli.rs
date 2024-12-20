use std::path::PathBuf;

use clap::{crate_name, Parser, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = crate_name!(), version, about)]
pub struct Cli {
    #[arg()]
    pub paths: Vec<PathBuf>,
    #[arg(short = 'i', long, default_value_t = true)]
    pub ignore_hidden: bool,
    #[arg(short = 's', long, default_value_t = true)]
    pub follow_symlinks: bool,
    #[arg(long, default_value_t = false)]
    pub overwrite_lrc_files: bool,
    #[arg(long, value_enum, default_value_t = FileMatchStrictness::FilterByExt)]
    pub strictness: FileMatchStrictness,
    #[arg(short, long)]
    pub proxy: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum FileMatchStrictness {
    TrustyGuesser,
    FilterByExt,
    Paranoid,
}
