mod tag_types;

use std::error::Error;
use std::fmt;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use lofty::error::LoftyError;
use lofty::file::AudioFile as _;
use lofty::file::TaggedFileExt as _;
use lofty::io::FileLike;
use lofty::tag::ItemKey;
use sqlite::Connection;
use sqlite::ConnectionThreadSafe;
use tokio::sync::mpsc::unbounded_channel as tokio_unbounded_channel;
use tokio::task::JoinSet;
use walkdir::WalkDir;
use wetutil::prelude::*;

use crate::lyrics::Lyrics;
use crate::lyrics::fetcher::LyricsFetcherBuilder;
use crate::lyrics::service::DbLyricsFetchService;
use crate::lyrics::service::LrclibLyricsFetchService;
use crate::worker::tag_types::TagTypesToWriteExt as _;

type StdFile = std::fs::File;
type TaggedFile = lofty::file::BoundTaggedFile<StdFile>;

type ChanUntagged = walkdir::DirEntry;
type ChanTagged = TaggedFile;
type ChanTaggedWithLyrics = (Lyrics, TaggedFile);

pub(super) trait UnboundedTx {
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

pub(super) trait SendError<T>: fmt::Debug + fmt::Display + Error {}

impl<T> SendError<T> for tokio::sync::mpsc::error::SendError<T> {}

impl<T> SendError<T> for std::sync::mpsc::SendError<T> {}

#[derive(Debug, thiserror::Error)]
pub(super) enum GuessFileError {
	#[error("Unsupported file type: {}", .0)]
	InvalidFileType(&'static str),
	#[error(transparent)]
	Lofty(#[from] LoftyError),
	#[error(transparent)]
	Io(#[from] std::io::Error),
}

pub(super) async fn lurk_and_tag<I, P>(paths: I)
where
	I: IntoIterator<Item = P> + Send + 'static,
	P: AsRef<Path>,
{
	let (untagged_tx, mut untagged_rx) = tokio_unbounded_channel::<ChanUntagged>();
	let (tagged_tx, mut tagged_rx) = tokio_unbounded_channel::<ChanTagged>();
	let (lrc_tx, mut lrc_rx) = tokio_unbounded_channel::<ChanTaggedWithLyrics>();

	let http_client = Arc::new(reqwest::Client::new());
	let db_conn = Connection::open_thread_safe(":memory:").unwrap();

	let lrclib_service = LrclibLyricsFetchService::new(http_client.clone());
	let db_service = DbLyricsFetchService::new(db_conn);

	let fetcher = LyricsFetcherBuilder::new(Box::new(db_service))
		.add_service(Box::new(lrclib_service))
		.build();

	let mut join_set = JoinSet::new();

	// Step 1: discover all the files.
	//
	// Walk the file structure in a separate task,
	// without blocking tokio's executors for async tasks.
	join_set.spawn_blocking(move || traverse(&untagged_tx, paths));

	// Step 2: read file tags, when possible.
	join_set.spawn(async move {
		let mut tagging_worker_handles = JoinSet::new();
		while let Some(dir_entry) = untagged_rx.recv().await {
			let tagged_tx = tagged_tx.clone();
			// TODO::perf consider using rayon's thread pool
			// instead of spawning a task for every file.
			tagging_worker_handles.spawn_blocking(move || {
				let path = dir_entry.into_path();
				let file_res = OpenOptions::new()
					.read(true)
					.write(true)
					.create(false)
					.open(&path);

				match file_res
					.err_into()
					.and_then(handle_file_guessing)
				{
					Ok(tagged_file) => {
						tagged_tx.send(tagged_file);
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
		while let Some(tagged_file) = tagged_rx.recv().await {
			let lrc_tx = lrc_tx.clone();
			networking_worker_handles.spawn(async move {
				// TODO: some networking here.
				// TODO: better lyrics type here than a plain `String`.
				lrc_tx.send((
					Lyrics::Unsynced("some lyrics here".into()),
					tagged_file,
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
		while let Some((lyrics, tagged_file)) = lrc_rx.recv().await {
			writing_worker_handles.spawn_blocking(|| {
				// TODO: write tags back to files.
			});
		}

		writing_worker_handles.join_all().await;
	});

	join_set.join_all().await;
}

// TODO::error_handling: change the return type to not box explicitly.
fn update_file_tags(file: StdFile) -> Result<(), Box<dyn Error>> {
	let mut tagged_file = handle_file_guessing(file)?;

	for tag_type in tagged_file.tag_types_to_write() {
		// TODO::logging
		dbg!(&tag_type);

		if let Some(tag) = tagged_file.tag_mut(tag_type) {
			// TODO::logging
			dbg!(tag.insert_text(
				lofty::tag::ItemKey::Lyrics,
				"I've been here before!".to_owned(),
			));
		}
	}

	tagged_file.save(Default::default())?;

	Ok(())
}

fn handle_file_guessing(file: StdFile) -> Result<TaggedFile, GuessFileError> {
	// TODO::config: add a way to make lofty guess (or not) track's filetype.
	lofty::probe::Probe::new(file)
		.guess_file_type()?
		.read_bound()
		.err_into()
		.and_then(|tagged_file| {
			match tagged_file.file_type() {
				// Do not support custom file types, since we wouldn't be able to write
				// their tags anyway. Also, it gets rid of "non-music" file problem
				// (.jpg, .png, .lrc, etc.).
				lofty::file::FileType::Custom(ft) => Err(GuessFileError::InvalidFileType(ft)),
				_ => Ok(tagged_file),
			}
		})
}

/// Traverse `paths` recursively,
/// sending any file (not a directory!) to `tx`.
pub(super) fn traverse<TX, I, P>(tx: &TX, paths: I)
where
	TX: UnboundedTx<Item = ChanUntagged>,
	I: IntoIterator<Item = P>,
	P: AsRef<Path>,
{
	for path in paths {
		for entry_path in WalkDir::new(&path)
			.into_iter()
			.consume_err(|err| {
				// TODO::logging
				dbg!(err);
			})
			.filter(|dir_entry| dir_entry.file_type().is_file())
		{
			// TODO::error_handling: remove unwrap,
			// (replace with `expect` that the channel will never be closed?)
			tx.send(entry_path).unwrap();
		}
	}
}
