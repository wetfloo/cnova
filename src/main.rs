// TODO: remove when we're done.
#![allow(unused)]

use cnova::IterExt;
use lofty::error::LoftyError;
use lofty::file::TaggedFile as LoftyTaggedFile;
use lofty::file::{AudioFile as _, TaggedFileExt as _};
use lofty::probe::Probe as LoftyProbe;
use lofty::tag::ItemKey as LoftyItemKey;
use lofty::tag::{Tag as LoftyTag, TagType as LoftyTagType};
use std::fs::FileType;
use std::fs::OpenOptions;
use std::iter::Inspect;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use std::{env::home_dir, sync::LazyLock};

use sqlite::ffi::sqlite3_stmt_status;
use walkdir::WalkDir;

trait LyricsHolder {
	fn lyrics(metadata: &Metadata) -> Option<Lyrics>;
}

trait LyricsResolver {
	async fn resolve_lyrics(metadata: &Metadata) -> Result<Lyrics, LyricsResolveError>;
}

struct Metadata {
	title: Option<String>,
	artist: Option<String>,
	album_artist: Option<String>,
	album: Option<String>,
	duration: Duration,
}

struct Lyrics {
	kind: LyricsKind,
	data: String,
}

enum LyricsKind {
	Synced,
	Unsynced,
}

struct LyricsResolveError;

fn main() {
	traverse();
}

fn traverse() {
	let mut path = home_dir().unwrap();
	path.push("Music/Experiment");

	for dir_entry in WalkDir::new(&path)
		.into_iter()
		.inspect_err(|err| {
			// TODO::logging
			dbg!(err);
		})
		.discard_err()
		.filter(|dir_entry| {
			dir_entry.file_type().is_file()
				&& dir_entry
					.path()
					.extension()
					.is_some_and(|ext| ext == "flac")
		}) {
		let entry_path = dir_entry.path();
		match handle_file_guessing(entry_path) {
			Ok(mut tagged_file) => {
				for tt in tagged_file.tag_types_to_write() {
					// TODO::logging
					dbg!(&tt);
					if let Some(tag) = tagged_file.tag_mut(tt) {
						// TODO::logging
						dbg!(tag.insert_text(
							LoftyItemKey::Lyrics,
							"I've been here before!".to_owned(),
						));
					}
				}

				let mut file = OpenOptions::new()
					.read(true)
					.write(true)
					.open(entry_path)
					// TODO::unwrap
					.unwrap();
				tagged_file
					// TODO::unwrap
					.save_to(&mut file, Default::default())
					.unwrap();
			},

			Err(err) => {
				// TODO::logging
				dbg!(&err);
			},
		}
	}
}

fn handle_file_guessing<P>(path: P) -> Result<LoftyTaggedFile, LoftyError>
where
	P: AsRef<Path>,
{
	// TODO::config: add a way to make lofty guess (or not) track's filetype.
	LoftyProbe::open(path)?
		.guess_file_type()?
		.read()
}

struct TagTypesToWrite {
	tt: LoftyTagType,
	primary_shown: bool,
}

impl TagTypesToWrite {
	fn new(tt: LoftyTagType) -> Self {
		Self {
			tt,
			primary_shown: false,
		}
	}
}

impl Iterator for TagTypesToWrite {
	type Item = LoftyTagType;

	fn next(&mut self) -> Option<Self::Item> {
		match (self.primary_shown, self.tt) {
			(false, tt) => {
				self.primary_shown = true;
				Some(tt)
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
