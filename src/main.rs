// TODO: remove when we're done.
#![allow(unused)]
#![deny(unreachable_pub)]

mod cli;
mod lyrics;
mod worker;

use std::env::home_dir;
use std::fs::create_dir_all;

use anyhow::anyhow;
use clap::Parser;
use tokio::fs::create_dir;

use crate::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::try_init()?;

	let base_dirs = cross_xdg::BaseDirs::with_prefix(env!("CARGO_CRATE_NAME"))?;
	log::debug!("got base dirs");
	let cli = Cli::parse();
	log::debug!("parsed cli args: {:?}", cli);

	log::trace!("successfully initialized all the basics");

	let mut db_path = base_dirs.cache_home().to_owned();
	create_dir_all(&db_path)?;
	log::debug!(
		"initialized database cache base dir {:?}",
		db_path,
	);
	db_path.push("cache.db");
	log::debug!(
		"initialized database path: {:?}",
		db_path,
	);

	worker::lurk_and_tag(cli.paths, db_path).await?;

	Ok(())
}
