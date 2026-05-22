// TODO: remove when we're done.
#![allow(unused)]

use std::error::Error;
use std::fs::FileType;
use std::fs::OpenOptions;
use std::io;
use std::iter::Inspect;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use std::{env::home_dir, sync::LazyLock};

use cnova::IterExt as _;
use cnova::result::ResultBothInto as _;
use cnova::result::ResultErrInto as _;
use lofty::error::LoftyError;
use lofty::file::{AudioFile as _, TaggedFileExt as _};
use lofty::file::{FileType as LoftyFileType, TaggedFile as LoftyTaggedFile};
use lofty::probe::Probe as LoftyProbe;
use lofty::tag::ItemKey as LoftyItemKey;
use lofty::tag::{Tag as LoftyTag, TagType as LoftyTagType};
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

	for entry_path in WalkDir::new(&path)
		.into_iter()
		.inspect_err(|err| {
			// TODO::logging
			dbg!(err);
		})
		.discard_err()
		.filter(|dir_entry| dir_entry.file_type().is_file())
		.map(|dir_entry| dir_entry.into_path())
	{
		update_file_tags(&entry_path);
	}
}

// TODO::error_handling: change the return type to not box explicitly.
fn update_file_tags<P>(path: P) -> Result<(), Box<dyn Error>>
where
	P: AsRef<Path>,
{
	let mut tagged_file = handle_file_guessing(path.as_ref())?;

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

	let mut dest_file = OpenOptions::new()
		.read(true)
		.write(true)
		.open(path.as_ref())?;
	tagged_file.save_to(&mut dest_file, Default::default())?;

	Ok(())
}

fn handle_file_guessing<P>(path: P) -> Result<LoftyTaggedFile, GuessFileError>
where
	P: AsRef<Path>,
{
	// TODO::config: add a way to make lofty guess (or not) track's filetype.
	LoftyProbe::open(path)?
		.guess_file_type()?
		.read()
		.err_into()
		.and_then(|tagged_file| {
			match tagged_file.file_type() {
				// Do not support custom file types, since we wouldn't be able to write
				// their tags anyway. Also, it gets rid of "non-music" file problem
				// (.jpg, .png, .lrc, etc.).
				LoftyFileType::Custom(ft) => Err(GuessFileError::InvalidFileType(ft)),
				_ => Ok(tagged_file),
			}
		})
}

#[derive(Debug, thiserror::Error)]
enum GuessFileError {
	#[error("Unsupported file type: {0}")]
	InvalidFileType(&'static str),
	#[error(transparent)]
	Lofty(#[from] LoftyError),
	#[error(transparent)]
	Io(#[from] io::Error),
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
