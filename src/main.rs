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
use const_format::formatcp;
use tokio::fs::create_dir;

use crate::cli::Cli;
use crate::lyrics::db::DbCache;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::try_init()?;

	let base_dirs = cross_xdg::BaseDirs::with_prefix(env!("CARGO_CRATE_NAME"))?;
	log::debug!("got base dirs");
	let mut cli = Cli::parse();
	log::debug!("parsed cli args: {:?}", cli);

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
	let db_cache = sqlite::Connection::open(&db_path).and_then(DbCache::new)?;

	let mut client_builder = reqwest::ClientBuilder::new().user_agent(formatcp!(
		"{} v{} ({})",
		env!("CARGO_CRATE_NAME"),
		env!("CARGO_PKG_VERSION"),
		env!("CARGO_PKG_HOMEPAGE"),
	));
	// Moving this value out of Option is fine,
	// because we don't use it anywhere else.
	if let Some(proxy) = cli.proxy.take() {
		client_builder = client_builder.proxy(proxy);
	}
	let http_client = client_builder.build()?;

	log::trace!("successfully initialized all the basics, ready to lurk (and tag)");

	worker::lurk_and_tag(cli.paths, db_cache, http_client).await?;

	Ok(())
}
