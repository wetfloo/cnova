mod tag_types;

use std::error::Error;
use std::fmt;
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;

use lofty::config::WriteOptions;
use lofty::error::LoftyError;
use lofty::file::TaggedFileExt as _;
use tokio::sync::mpsc::unbounded_channel as tokio_unbounded_channel;
use tokio::task::JoinSet;
use walkdir::WalkDir;
use wetutil::prelude::*;

use crate::lyrics::Lyrics;
use crate::lyrics::TaggedFileData;
use crate::lyrics::db::DbCache;
use crate::lyrics::fetcher::LyricsFetcherBuilder;
use crate::lyrics::service::LrclibLyricsFetchService;
use crate::worker::tag_types::TagTypesToWriteExt as _;

type StdFile = std::fs::File;
type TaggedFile = lofty::file::BoundTaggedFile<StdFile>;

type Sender<T> = tokio::sync::mpsc::UnboundedSender<T>;
type ChanUntagged = walkdir::DirEntry;
type ChanTagged = TaggedFile;
type ChanTaggedWithLyrics = (Lyrics, TaggedFile);

#[derive(Debug, thiserror::Error)]
pub(super) enum GuessFileError {
	#[error("Unsupported file type: {}", .0)]
	InvalidFileType(&'static str),
	#[error(transparent)]
	Lofty(#[from] LoftyError),
	#[error(transparent)]
	Io(#[from] std::io::Error),
}

pub(super) async fn lurk_and_tag<I, P, DB>(paths: I, db_path: DB)
where
	I: IntoIterator<Item = P> + Send + 'static,
	P: AsRef<Path>,
	DB: AsRef<Path> + 'static,
{
	let (untagged_tx, mut untagged_rx) = tokio_unbounded_channel::<ChanUntagged>();
	let (tagged_tx, mut tagged_rx) = tokio_unbounded_channel::<ChanTagged>();
	let (lrc_tx, mut lrc_rx) = tokio_unbounded_channel::<ChanTaggedWithLyrics>();

	let lrc_fetcher: LazyLock<Arc<_>> = LazyLock::new(|| {
		// TODO: use this client for more services, outside of just LRCLIB
		let http_client: Arc<_> = reqwest::Client::new().into();

		let lrclib_service = LrclibLyricsFetchService::new(http_client.clone());

		LyricsFetcherBuilder::new(Box::new(lrclib_service))
			.build()
			.into()
	});

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

		while let Some(join_res) = tagging_worker_handles.join_next().await {
			if let Err(e) = join_res {
				// TDOO::logging
				dbg!(e);
			}
		}
	});

	// Step 3: use file tags to request lyrics.
	let db_local_set = tokio::task::LocalSet::new();
	join_set.spawn_local_on(
		async move {
			// TODO: accept configuration to open the database in different locations
			let mut db_cache = sqlite::Connection::open(&db_path)
				.and_then(DbCache::new)
				// TODO::error_handling remove unwrap
				.unwrap();

			let mut lrc_fetch_worker_handles = JoinSet::new();

			while let Some(tagged_file) = tagged_rx.recv().await {
				let lrc_tx = lrc_tx.clone();

				// First, attempt to get lyrics from the database...
				match db_cache.get_lrc(&(&tagged_file).into()) {
					Ok(Some(v)) => {
						lrc_tx.send((v, tagged_file));
						// ...if that worked, move on.
						continue;
					},
					Ok(None) => (),
					Err(e) => {
						// TODO::logging
						dbg!(e);
					},
				}

				// ...if it didn't work, get/init network lyrics fetcher.
				let lrc_fetcher = lrc_fetcher.clone();

				lrc_fetch_worker_handles.spawn(async move {
					lrc_fetcher
						.request_lyrics(&(&tagged_file).into())
						.await
						.map(|lyrics| (lyrics, tagged_file))
				});
			}

			while let Some(join_res) = lrc_fetch_worker_handles
				.join_next()
				.await
			{
				match join_res {
					Ok(Ok((lyrics, tagged_file))) => {
						if let Err(e) = db_cache.insert_lrc(&(&tagged_file).into(), lyrics.clone())
						{
							// TODO::logging
							dbg!(e);
						}

						lrc_tx.send((lyrics, tagged_file));
					},

					Ok(Err(errors)) => {
						// TODO::logging
						//
						// TODO::error_handling consider streaming service errors via channels
						// instead of collecting them into Vec.
						dbg!(errors);
					},

					Err(e) => {
						// TODO::logging
						dbg!(e);
					},
				}
			}
		},
		&db_local_set,
	);

	// Step 4: write lyrics tags back to files.
	join_set.spawn(async move {
		let mut writing_worker_handles = JoinSet::new();

		while let Some((lyrics, tagged_file)) = lrc_rx.recv().await {
			writing_worker_handles.spawn_blocking(move || {
				let mut tagged_file = tagged_file;

				for tag_type in tagged_file.tag_types_to_write() {
					if let Some(tag) = tagged_file.tag_mut(tag_type) {
						use lofty::tag::ItemKey as K;

						// TODO::perf don't clone this if it's not needed
						match lyrics.clone() {
							Lyrics::Synced(lrc) => {
								tag.insert_text(K::Lyrics, lrc);
							},
							Lyrics::Unsynced(lrc) => {
								tag.insert_text(K::UnsyncLyrics, lrc);
							},
							Lyrics::Instrumental => {
								tag.remove_key(K::Lyrics);
								tag.remove_key(K::UnsyncLyrics);
							},
						}
					}
				}

				if let Err(e) = tagged_file.save(WriteOptions::default()) {
					// TODO::logging
					dbg!(e);
				}
			});
		}

		while let Some(join_res) = writing_worker_handles.join_next().await {
			if let Err(e) = join_res {
				// TDOO::logging
				dbg!(e);
			}
		}
	});

	db_local_set.await;
	while let Some(join_res) = join_set.join_next().await {
		if let Err(e) = join_res {
			// TDOO::logging
			dbg!(e);
		}
	}
}

/// Traverse `paths` recursively,
/// sending any file (not a directory!) to `tx`.
fn traverse<I, P>(tx: &Sender<ChanUntagged>, paths: I)
where
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

fn handle_file_guessing(file: StdFile) -> Result<TaggedFile, GuessFileError> {
	// TODO::config: add a way to make lofty guess (or not) track's filetype.
	lofty::probe::Probe::new(file)
		.guess_file_type()?
		.read_bound()
		.err_into()
		.and_then(|tagged_file| {
			match tagged_file.file_type() {
				// Do not support custom file types, since we wouldn't be able to write
				// those tags anyway. Also, it gets rid of "non-music" file problem
				// (.jpg, .png, .lrc, etc.).
				lofty::file::FileType::Custom(ft) => Err(GuessFileError::InvalidFileType(ft)),
				_ => Ok(tagged_file),
			}
		})
}
