//! Do the program's work as fast as possible.
//!
//! This module is where the biggest chunk of work actually happens.
//! Its *steps* are split up into methods
//! (1[step_1], 2[step_2], 3[step_3], 4[step_4]),
//! so that it's easier to understand the distinct steps of the pipeline
//! (*discover files*, *read file tags*, *find lyrics*, *write file tags*).

mod tag_types;

use std::fs::OpenOptions;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use lofty::config::WriteOptions;
use lofty::error::LoftyError;
use lofty::file::TaggedFileExt as _;
use tokio::sync::mpsc::channel as tokio_channel;
use tokio::sync::mpsc::unbounded_channel as tokio_unbounded_channel;
use tokio::task::JoinSet;
use tokio::task::spawn_blocking;
use walkdir::WalkDir;
use wetutil::prelude::*;

use crate::lyrics::Lyrics;
use crate::lyrics::TaggedFileTags;
use crate::lyrics::TaggedFileWrapper;
use crate::lyrics::db::DbCache;
use crate::lyrics::fetcher::LyricsFetcher;
use crate::lyrics::fetcher::LyricsFetcherBuilder;
use crate::lyrics::service::LrclibLyricsFetchService;
use crate::lyrics::service::LyricsServiceAcquisitionValue;
use crate::worker::tag_types::TagTypesToWriteExt as _;

type StdFile = std::fs::File;
type TaggedFile = lofty::file::BoundTaggedFile<StdFile>;

type Sender<T> = tokio::sync::mpsc::UnboundedSender<T>;
type Receiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;
type ChanUntagged = PathBuf;
type ChanTagged = TaggedFile;
type ChanTaggedWithLyrics = (Lyrics, TaggedFile);

const CHANNEL_SEND_EXPECT_MSG: &str =
	"couldn't send the value to the channel. did someone close it?";
const DISK_IO_SEMAPHORE_EXPECT_MSG: &str = "couldn't acquire disk_io_semaphore, was it dropped?";
const NET_IO_SEMAPHORE_EXPECT_MSG: &str = "couldn't acquire net_io_semaphore, was it dropped?";

macro_rules! join_fail_error {
	() => {{
		::log::error!(
			"{}:{}:{}:failed to join a task",
			::core::file!(),
			::core::column!(),
			::core::line!(),
		);
	}};
	($err:expr$(,)?) => {{
		::log::error!(
			r#"{}:{}:{}: failed to join a task with error "{}""#,
			::core::file!(),
			::core::column!(),
			::core::line!(),
			$err,
		);
	}};
}

pub(super) async fn lurk_and_tag<I, P>(
	paths: I,
	db_cache: DbCache,
	http_client: reqwest::Client,
	disk_io_semaphore: tokio::sync::Semaphore,
	net_io_semaphore: tokio::sync::Semaphore,
) -> anyhow::Result<()>
where
	I: IntoIterator<Item = P> + Send + 'static,
	P: AsRef<Path>,
{
	let (untagged_tx, mut untagged_rx) = tokio_unbounded_channel::<ChanUntagged>();
	let (tagged_tx, mut tagged_rx) = tokio_unbounded_channel::<ChanTagged>();
	let (lrc_tx, mut lrc_rx) = tokio_unbounded_channel::<ChanTaggedWithLyrics>();

	let disk_io_semaphore: Arc<_> = disk_io_semaphore.into();
	let net_io_semaphore: Arc<_> = net_io_semaphore.into();

	// TODO: add more lyrics services and use this ref counter there.
	let http_client: Arc<_> = http_client.into();

	let lrc_fetcher = LyricsFetcherBuilder::new(Box::new(LrclibLyricsFetchService::new(
		http_client.clone(),
	)))
	.build();
	log::debug!(
		"initialized lyrics fetcher {:?}",
		lrc_fetcher,
	);

	let ((), (), (), ()) = tokio::join!(
		step_1(
			paths,
			disk_io_semaphore.clone(),
			untagged_tx,
		),
		step_2(
			disk_io_semaphore.clone(),
			&mut untagged_rx,
			tagged_tx,
		),
		step_3(
			net_io_semaphore.clone(),
			&mut tagged_rx,
			lrc_tx,
			db_cache,
			lrc_fetcher,
		),
		step_4(disk_io_semaphore.clone(), &mut lrc_rx),
	);

	Ok(())
}

/// Traverse `paths` recursively,
/// sending any file (not a directory!) to `untagged_tx`.
fn traverse<I, P>(untagged_tx: Sender<ChanUntagged>, paths: I)
where
	I: IntoIterator<Item = P>,
	P: AsRef<Path>,
{
	for path in paths {
		for entry_path in WalkDir::new(&path)
			.into_iter()
			.consume_err(|e| {
				log::warn!(
					r#"failed to traverse path {:?} due to error "{}""#,
					path.as_ref(),
					e,
				)
			})
			.filter(|dir_entry| dir_entry.file_type().is_file())
			.map(|dir_entry| dir_entry.into_path())
		{
			untagged_tx
				.send(entry_path)
				.expect(CHANNEL_SEND_EXPECT_MSG);
		}

		log::info!(
			"completed recursive path traversal @ {:?}",
			path.as_ref()
		);
	}
}

#[derive(Debug, thiserror::Error)]
enum GuessFileError {
	#[error(r#"Unsupported file type: "{}""#, .ft)]
	InvalidFileType {
		ft: &'static str,
	},
	#[error(transparent)]
	Lofty(#[from] LoftyError),
	#[error(transparent)]
	Io(#[from] std::io::Error),
}

fn load_file_tags<P>(path: P) -> Result<TaggedFile, GuessFileError>
where
	P: AsRef<Path>,
{
	OpenOptions::new()
		.read(true)
		.write(true)
		.create(false)
		.open(&path)
		.err_into()
		.and_then(|file| {
			lofty::probe::Probe::new(file)
				.guess_file_type()?
				.read_bound()
				.err_into()
		})
		.and_then(|tagged_file| {
			match tagged_file.file_type() {
				// Do not support custom file types, since we wouldn't be able to write
				// those tags anyway.
				lofty::file::FileType::Custom(ft) => Err(GuessFileError::InvalidFileType { ft }),
				_ => Ok(tagged_file),
			}
		})
		.ok_into()
}

fn update_file_lyrics_tag(
	tagged_file: &mut TaggedFile,
	lyrics: &Lyrics,
) -> lofty::error::Result<()> {
	for tag_type in tagged_file.tag_types_to_write() {
		if let Some(tag) = tagged_file.tag_mut(tag_type) {
			use lofty::tag::ItemKey as K;

			match lyrics {
				Lyrics::Synced(lrc) => {
					tag.insert_text(K::Lyrics, lrc.to_owned());
				},
				Lyrics::Unsynced(lrc) => {
					tag.insert_text(K::UnsyncLyrics, lrc.to_owned());
				},
				Lyrics::Instrumental => {
					tag.remove_key(K::Lyrics);
					tag.remove_key(K::UnsyncLyrics);
				},
			}
		}
	}

	tagged_file.save(WriteOptions::default())
}

/// Step 1: discover all the files.
///
/// Walk the file structure in a separate task,
/// without blocking tokio's executors for async tasks.
async fn step_1<I, P>(
	paths: I,
	disk_io_semaphore: Arc<tokio::sync::Semaphore>,
	tx: Sender<ChanUntagged>,
) where
	I: IntoIterator<Item = P> + Send + 'static,
	P: AsRef<Path>,
{
	let permit = disk_io_semaphore
		.acquire_owned()
		.await
		.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
	log::trace!(
		"acquired a permit ({:?}) for step 1",
		permit,
	);

	if let Err(e) = spawn_blocking(move || {
		let _permit = permit;
		// Once the function completes,
		// we expect to be able to close the channel.
		//
		// TODO: make it so that it doesn't use the channel directly,
		// returning an iterator instead.
		traverse(tx, paths);
	})
	.await
	{
		join_fail_error!(e);
	}
}

/// Step 2: read file tags, when possible.
async fn step_2(
	disk_io_semaphore: Arc<tokio::sync::Semaphore>,
	rx: &mut Receiver<ChanUntagged>,
	tx: Sender<ChanTagged>,
) {
	let mut worker_handles = JoinSet::new();

	while let Some(path) = rx.recv().await {
		let disk_io_semaphore = disk_io_semaphore.clone();
		let tx = tx.clone();

		// Need async context here to acquire tokio's semaphore.
		worker_handles.spawn(async move {
			let permit = disk_io_semaphore
				.acquire_owned()
				.await
				.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
			log::trace!(
				"acquired a permit ({:?}) for step 2",
				permit,
			);

			// TODO::perf consider using rayon's thread pool
			// instead of spawning a task for every file.
			match spawn_blocking(move || {
				// Drops the permit when `fn load_file_tags` completes,
				// so the other tasks are able to pick up the work.
				let _permit = permit;
				(load_file_tags(&path), path)
			})
			.await
			{
				Ok((Ok(tagged_file), path)) => {
					log::info!(
						"successfully loaded file tags @ path {:?}",
						path,
					);

					tx.send(tagged_file)
						.expect(CHANNEL_SEND_EXPECT_MSG);
				},

				Ok((Err(GuessFileError::InvalidFileType { ft }), path)) => {
					log::info!(
						r#"file type "{}" not recognized @ path {:?}; skipping..."#,
						ft,
						path,
					);
				},

				Ok((Err(e), path)) => {
					log::warn!(
						r#"error "{}": failed to load file tags @ path {:?}; skipping..."#,
						e,
						path,
					);
				},

				Err(e) => {
					join_fail_error!(e);
				},
			}
		});
	}

	while let Some(join_res) = worker_handles.join_next().await {
		if let Err(join_err) = join_res {
			join_fail_error!(join_err);
		}
	}
}

/// Step 3: use file tags to request lyrics.
async fn step_3(
	net_io_semaphore: Arc<tokio::sync::Semaphore>,
	rx: &mut Receiver<ChanTagged>,
	tx: Sender<ChanTaggedWithLyrics>,
	db_cache: DbCache,
	lrc_fetcher: LyricsFetcher,
) {
	let (lrc_request_tx, mut lrc_request_rx) = tokio_channel(1);
	let (db_insert_tx, mut db_insert_rx) = tokio_channel(1);

	let ((), mut net_workers, ()) = tokio::join!(
		{
			let tx = tx.clone();
			let db_cache = db_cache.clone();

			async move {
				while let Some(tagged_file_wrapper) = rx
					.recv()
					.await
					.map(TaggedFileWrapper::new)
				{
					// First, attempt to get lyrics from the database...
					log::trace!(
						"reading lyrics for {} from the database...",
						tagged_file_wrapper,
					);
					match db_cache.get_lrc(&tagged_file_wrapper) {
						Ok(Some(lrc)) => {
							log::info!(
								"successfully loaded lyrics from cache for {}",
								tagged_file_wrapper,
							);

							tx.send((lrc, tagged_file_wrapper.0))
								.expect(CHANNEL_SEND_EXPECT_MSG);
							// ...if that worked, move on.
							continue;
						},

						Ok(None) => {
							log::debug!(
								"couldn't find lyrics for track {} inside a db cache",
								tagged_file_wrapper,
							);

							lrc_request_tx
								.send(tagged_file_wrapper)
								.await
								.expect(CHANNEL_SEND_EXPECT_MSG);
						},

						Err(e) => {
							log::warn!(
								r#"db cache interaction failed with error "{}""#,
								e,
							);
						},
					}
				}
			}
		},
		{
			let tx = tx.clone();

			async move {
				let mut net_workers = JoinSet::new();

				while let Some(tagged_file_wrapper) = lrc_request_rx.recv().await {
					// ...if it didn't work, get/init network lyrics fetcher.
					net_workers.spawn({
						let net_io_semaphore: Arc<_> = net_io_semaphore.clone();
						let tx = tx.clone();
						let lrc_fetcher = lrc_fetcher.clone();
						let db_insert_tx = db_insert_tx.clone();

						async move {
							let permit = net_io_semaphore
								.acquire_owned()
								.await
								.expect(NET_IO_SEMAPHORE_EXPECT_MSG);
							log::trace!(
								"acquired a permit ({:?}) for step 3",
								permit,
							);
							let res = lrc_fetcher
								.request_lyrics(&tagged_file_wrapper)
								.await
								.map(|lyrics| (lyrics, tagged_file_wrapper));

							mem::drop(permit);

							match res {
								Ok((
									LyricsServiceAcquisitionValue {
										lyrics,
										service_name,
									},
									tagged_file_wrapper,
								)) => {
									log::info!(
										r#"service "{}" successfully found lyrics data for {}"#,
										service_name,
										tagged_file_wrapper,
									);

									db_insert_tx
										.send((
											TaggedFileTags::new_from_existing(&tagged_file_wrapper),
											lyrics.clone(),
										))
										.await
										.expect(CHANNEL_SEND_EXPECT_MSG);

									tx.send((lyrics, tagged_file_wrapper.0))
										.expect(CHANNEL_SEND_EXPECT_MSG);
								},

								Err(service_errors) => {
									for e in service_errors {
										log::warn!(r#"lyrics service error "{}""#, e);
									}
								},
							}
						}
					});
				}

				net_workers
			}
		},
		{
			let db_cache = db_cache.clone();

			async move {
				while let Some((tags, lyrics)) = db_insert_rx.recv().await {
					match db_cache.insert_lrc(&tags, lyrics) {
						Ok(()) => log::info!(
							"successfully cached lyrics for {}",
							&tags,
						),
						Err(e) => log::warn!(
							r#"failed to insert lyrics for {} with error "{}", it will not be cached!"#,
							&tags,
							e,
						),
					}
				}
			}
		}
	);

	while let Some(join_res) = net_workers.join_next().await {
		if let Err(join_err) = join_res {
			join_fail_error!(join_err);
		}
	}
}

/// Step 4: write lyrics tags back to files.
async fn step_4(
	disk_io_semaphore: Arc<tokio::sync::Semaphore>,
	rx: &mut Receiver<ChanTaggedWithLyrics>,
) {
	let mut worker_handles = JoinSet::new();

	while let Some((lyrics, mut tagged_file)) = rx.recv().await {
		let disk_io_semaphore = disk_io_semaphore.clone();

		worker_handles.spawn(async move {
			let permit = disk_io_semaphore
				.acquire_owned()
				.await
				.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
			log::trace!(
				"acquired a permit ({:?}) for step 4",
				permit,
			);

			match spawn_blocking(move || {
				let _permit = permit;
				(
					update_file_lyrics_tag(&mut tagged_file, &lyrics),
					TaggedFileWrapper::new(tagged_file),
				)
			})
			.await
			{
				Ok((Ok(()), tagged_file_wrapper)) => {
					log::info!(
						"successfully wrote tags for {}",
						tagged_file_wrapper,
					);
				},
				Ok((Err(e), tagged_file_wrapper)) => {
					log::warn!(
						r#"failed to write tags for file {} with error "{}""#,
						tagged_file_wrapper,
						e,
					);
				},
				Err(join_err) => {
					join_fail_error!(join_err);
				},
			}
		});
	}
}
