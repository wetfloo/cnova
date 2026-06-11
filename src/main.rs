// TODO: remove when we're done.
#![allow(unused)]
#![deny(unreachable_pub)]

mod lyrics;
mod worker;

use std::env::home_dir;
use std::error::Error;
use std::fmt;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;

use lofty::error::LoftyError;
use lofty::file::AudioFile as _;
use lofty::file::FileType as LoftyFileType;
use lofty::file::TaggedFile as LoftyTaggedFile;
use lofty::file::TaggedFileExt as _;
use lofty::probe::Probe as LoftyProbe;
use lofty::tag::ItemKey as LoftyItemKey;
use lofty::tag::TagType as LoftyTagType;
use tokio::task::JoinSet;
use walkdir::WalkDir;
use wetutil::prelude::*;

#[tokio::main]
async fn main() {
	let mut paths = Vec::new();

	let mut path = home_dir().unwrap();
	path.push("Music/Experiment");

	paths.push(path);

	worker::lurk_and_tag(paths);
}
