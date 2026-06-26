// TODO: remove when we're done.
#![allow(unused)]
#![deny(unreachable_pub)]

mod cli;
mod lyrics;
mod worker;

use std::env::home_dir;

use clap::Parser;

use crate::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let base_dirs = cross_xdg::BaseDirs::with_prefix(env!("CARGO_CRATE_NAME"))?;
	let cli = Cli::parse();

	let mut db_path = base_dirs.cache_home().to_owned();
	db_path.push("cache.db");

	worker::lurk_and_tag(cli.paths, db_path);

	Ok(())
}
