//! Handle command-line arguments passed to the program.

use std::path::PathBuf;

use clap::Parser;
use clap::crate_name;
use clap::value_parser;

#[derive(Debug, Parser)]
#[command(name = crate_name!(), version, about)]
pub(crate) struct Cli {
	/// Paths to scan. Could be a mix files or directories. If it's a directory, this program will
	/// traverse it recursively and download lyrics, reporting any errors along the way.
	/// If it's a file, will download a corresponding lyrics for it and update that file.
	#[arg(required = true)]
	pub(crate) paths: Vec<PathBuf>,

	/// How many simultaneous network requests will occur at the same time.
	#[arg(
        short = 'j',
        long,
        default_value_t = 5,
        value_parser = value_parser!(u16).range(1..),
    )]
	pub(crate) download_jobs: u16,

	/// How many threads will be spawn to process the files.
	/// 1 is useful for HDDs.
	/// 0 will make the program automatically determine
	/// the number of available CPUs of the current system.
	#[arg(short = 'J', long, default_value_t = 0)]
	pub(crate) processing_jobs: u16,

	/// Proxy setting, supporting SOCKS5, SOCKS4 and HTTP proxies.
	#[arg(short, long, value_parser = proxy)]
	pub(crate) proxy: Option<reqwest::Proxy>,
}

fn proxy(s: &str) -> Result<reqwest::Proxy, String> {
	reqwest::Proxy::all(s).map_err(|_| format!("invalid proxy string: {}", s))
}
