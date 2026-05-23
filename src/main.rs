// TODO: remove when we're done.
#![allow(unused)]

use std::borrow::Cow;
use std::error::Error;
use std::fs::FileType;
use std::fs::OpenOptions;
use std::io;
use std::iter::Inspect;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use std::{env::home_dir, sync::LazyLock};

use lofty::error::LoftyError;
use lofty::file::{AudioFile as _, TaggedFileExt as _};
use lofty::file::{FileType as LoftyFileType, TaggedFile as LoftyTaggedFile};
use lofty::probe::Probe as LoftyProbe;
use lofty::tag::ItemKey as LoftyItemKey;
use lofty::tag::{Tag as LoftyTag, TagType as LoftyTagType};
use sqlite::ffi::sqlite3_stmt_status;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel as tokio_unbounded_channel;
use tokio::task;
use tokio::task::JoinSet;
use walkdir::WalkDir;
use wetutil::prelude::*;

type EntryTx = UnboundedSender<PathBuf>;
type EntryRx = UnboundedReceiver<PathBuf>;

#[tokio::main]
async fn main() {
	let (tx, mut rx) = tokio_unbounded_channel();

	let mut paths = Vec::new();
	let mut path = home_dir().unwrap();
	path.push("Music/Experiment");
	paths.push(path);

	// Walk the file structure in a separate thread,
	// without blocking tokio's executors for async tasks.
	let tx_join_handle = task::spawn_blocking(move || traverse_v2(&tx, paths));
	// Get the results from a single async task,
	// that's able to spawn a task for each entry.
	let rx_join_handle = task::spawn(async move {
		handle_entries(&mut rx).await;
	});
	tx_join_handle.await;
	rx_join_handle.await;
}

/// Traverse `paths` recursively,
/// sending any file (not a directory!) to `tx`.
fn traverse_v2<I, P>(tx: &EntryTx, paths: I)
where
	I: IntoIterator<Item = P>,
	P: AsRef<Path>,
{
	for path in paths {
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
			// TODO::error_handling: remove unwrap,
			// (replace with `expect` that the channel will never be closed?)
			tx.send(entry_path).unwrap();
		}
	}
}

// TODO::doc appropriate by who?
// Add a semaphore description here when it's added
/// Get the contents of `rx`,
/// spawning at max as many tasks as deemed appropriate
async fn handle_entries(rx: &mut EntryRx) {
	let mut join_set = JoinSet::new();
	let mut abort_handles = Vec::new();

	while let Some(p) = rx.recv().await {
		let abort_handle = join_set.spawn(tag_entry(Cow::Owned(p)));
		abort_handles.push(abort_handle);
	}

	join_set.join_all().await;
}

async fn tag_entry<'a>(path: Cow<'a, Path>) {
	// TODO::perf don't await, just pipeline all the steps
	// (reading file tags, network requests, writing tags)
	let path = path.into_owned();
	let tagged_file = task::spawn_blocking(move || handle_file_guessing(path)).await;
	let network_req = todo!("add a network request here");
	let tags_write_handle = todo!("add the ability to write tags here");
}

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

	for tag_type in tagged_file.tag_types_to_write() {
		// TODO::logging
		dbg!(&tag_type);

		if let Some(tag) = tagged_file.tag_mut(tag_type) {
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
