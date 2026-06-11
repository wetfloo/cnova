// TODO: remove when we're done.
#![allow(unused)]

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
}
struct TagTypesToWrite {
	tag_type: LoftyTagType,
	primary_shown: bool,
}

impl TagTypesToWrite {
	fn new(tag_type: LoftyTagType) -> Self {
		Self {
			tag_type,
			primary_shown: false,
		}
	}
}

impl Iterator for TagTypesToWrite {
	type Item = LoftyTagType;

	fn next(&mut self) -> Option<Self::Item> {
		match (self.primary_shown, self.tag_type) {
			(false, tag_type) => {
				self.primary_shown = true;
				Some(tag_type)
			},
			(true, LoftyTagType::Id3v2) => Some(LoftyTagType::Id3v1),
			(true, _) => None,
		}
	}
}

trait TagTypesToWriteExt {
	type Iter: Iterator<Item = LoftyTagType>;

	fn tag_types_to_write(&self) -> Self::Iter;
}

impl TagTypesToWriteExt for LoftyTaggedFile {
	type Iter = TagTypesToWrite;

	fn tag_types_to_write(&self) -> Self::Iter {
		TagTypesToWrite::new(self.primary_tag_type())
	}
}
