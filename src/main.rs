// TODO: remove when we're done.
#![allow(unused)]
#![deny(unreachable_pub)]

mod lyrics;
mod worker;

use std::env::home_dir;


#[tokio::main]
async fn main() {
	let mut paths = Vec::new();

	let mut path = home_dir().unwrap();
	path.push("Music/Experiment");

	paths.push(path);

	worker::lurk_and_tag(paths);
}
