// TODO: remove when we're done.
#![allow(unused)]

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::fs::FileType;
use std::fs::OpenOptions;
use std::io;
use std::io::BufReader;
use std::io::Read;
use std::iter::Inspect;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::Receiver as StdUnboundedRx;
use std::sync::mpsc::Sender as StdUnboundedTx;
use std::sync::mpsc::channel as std_unbounded_channel;
use std::thread;
use std::time::Duration;
use std::{env::home_dir, sync::LazyLock};

use lofty::error::LoftyError;
use lofty::file::{AudioFile as _, TaggedFileExt as _};
use lofty::file::{FileType as LoftyFileType, TaggedFile as LoftyTaggedFile};
use lofty::probe::Probe as LoftyProbe;
use lofty::tag;
use lofty::tag::ItemKey as LoftyItemKey;
use lofty::tag::{Tag as LoftyTag, TagType as LoftyTagType};
use sqlite::ffi::sqlite3_stmt_status;
use tokio::sync::mpsc::UnboundedReceiver as TokioUnboundedRx;
use tokio::sync::mpsc::UnboundedSender as TokioUnboundedTx;
use tokio::sync::mpsc::unbounded_channel as tokio_unbounded_channel;
use tokio::task;
use tokio::task::JoinSet;
use walkdir::WalkDir;
use wetutil::prelude::*;

#[tokio::main]
async fn main() {
	let (untagged_tx, mut untagged_rx) = tokio_unbounded_channel();
	let (tagged_tx, mut tagged_rx) = tokio_unbounded_channel();
	let (lrc_tx, mut lrc_rx) = tokio_unbounded_channel();

	let mut paths = Vec::new();
	let mut path = home_dir().unwrap();
	path.push("Music/Experiment");
	paths.push(path);

	let mut join_set = JoinSet::new();

	// Step 1: discover all the files.
	//
	// Walk the file structure in a separate task,
	// without blocking tokio's executors for async tasks.
	join_set.spawn_blocking(move || traverse_v2(&untagged_tx, paths));

	// Step 2: read file tags, when possible.
	join_set.spawn(async move {
		let mut tagging_worker_handles = JoinSet::new();
		while let Some(dir_entry) = untagged_rx.recv().await {
			let tagged_tx = tagged_tx.clone();
			// TODO::perf consider using rayon's thread pool
			// instead of spawning a task for every file.
			tagging_worker_handles.spawn_blocking(move || {
				let path = dir_entry.into_path();
				match handle_file_guessing(&path) {
					Ok(tagged_file) => {
						tagged_tx.send((tagged_file, path));
					},
					Err(guess_err) => {
						// TODO::logging
						dbg!(guess_err);
					},
				}
			});
		}

		tagging_worker_handles.join_all().await;
	});

	// Step 3: use file tags to request lyrics
	join_set.spawn(async move {
		let mut networking_worker_handles = JoinSet::new();
		while let Some((tagged_file, path)) = tagged_rx.recv().await {
			let lrc_tx = lrc_tx.clone();
			networking_worker_handles.spawn(async move {
				// TODO: some networking here.
				// TODO: better lyrics type here than a plain `String`.
				lrc_tx.send((
					"some lyrics here".to_owned(),
					tagged_file,
					path,
				));
			});
		}

		networking_worker_handles
			.join_all()
			.await;
	});

	// Step 4: write lyrics tags back to files.
	join_set.spawn(async move {
		let mut writing_worker_handles = JoinSet::new();
		while let Some((lyrics, tagged_file, path)) = lrc_rx.recv().await {
			writing_worker_handles.spawn_blocking(|| {
				// TODO: write tags back to files.
			});
		}

		writing_worker_handles.join_all().await;
	});

	join_set.join_all().await;
}

/// Traverse `paths` recursively,
/// sending any file (not a directory!) to `tx`.
fn traverse_v2<TX, I, P>(tx: &TX, paths: I)
where
	TX: UnboundedTx<Item = walkdir::DirEntry>,
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
		{
			// TODO::error_handling: remove unwrap,
			// (replace with `expect` that the channel will never be closed?)
			tx.send(entry_path).unwrap();
		}
	}
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

trait UnboundedTx {
	type Item;
	type Err: SendError<Self::Item>;

	fn send(&self, message: Self::Item) -> Result<(), Self::Err>;
}

impl<T> UnboundedTx for tokio::sync::mpsc::UnboundedSender<T> {
	type Item = T;
	type Err = tokio::sync::mpsc::error::SendError<Self::Item>;

	fn send(&self, message: Self::Item) -> Result<(), Self::Err> {
		self.send(message)
	}
}

impl<T> UnboundedTx for std::sync::mpsc::Sender<T> {
	type Item = T;
	type Err = std::sync::mpsc::SendError<Self::Item>;

	fn send(&self, message: Self::Item) -> Result<(), Self::Err> {
		self.send(message)
	}
}

trait SendError<T>: fmt::Debug + fmt::Display + Error {}

impl<T> SendError<T> for tokio::sync::mpsc::error::SendError<T> {}

impl<T> SendError<T> for std::sync::mpsc::SendError<T> {}
