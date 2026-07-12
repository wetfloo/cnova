mod tag_types;

use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use lofty::config::WriteOptions;
use lofty::error::LoftyError;
use lofty::file::TaggedFileExt as _;
use tokio::sync::mpsc::unbounded_channel as tokio_unbounded_channel;
use tokio::task::JoinSet;
use tokio::task::spawn_blocking;
use walkdir::WalkDir;
use wetutil::prelude::*;

use crate::lyrics::Lyrics;
use crate::lyrics::TaggedFileData;
use crate::lyrics::db::DbCache;
use crate::lyrics::fetcher::LyricsFetcherBuilder;
use crate::lyrics::service::LrclibLyricsFetchService;
use crate::lyrics::service::LyricsServiceAcquisitionValue;
use crate::worker::tag_types::TagTypesToWriteExt as _;

type StdFile = std::fs::File;
type TaggedFile = lofty::file::BoundTaggedFile<StdFile>;

type Sender<T> = tokio::sync::mpsc::UnboundedSender<T>;
type ChanUntagged = PathBuf;
type ChanTagged = TaggedFile;
type ChanTaggedWithLyrics = (Lyrics, TaggedFile);

const CHANNEL_SEND_EXPECT_MSG: &str =
	"couldn't send the value to the channel. did someone close it?";
const DISK_IO_SEMAPHORE_EXPECT_MSG: &str = "couldn't acquire disk_io_semaphore, was it dropped?";
const NET_IO_SEMAPHORE_EXPECT_MSG: &str = "couldn't acquire net_io_semaphore, was it dropped?";

macro_rules! join_fail_error {
	() => {{
		::log::error!("failed to join a task");
	}};
	($err:expr$(,)?) => {{
		::log::error!(
			r#"failed to join a task with error "{}""#,
			$err,
		);
	}};
}

pub(super) async fn lurk_and_tag<I, P>(
	paths: I,
	mut db_cache: DbCache,
	http_client: reqwest::Client,
	disk_io_semaphore: Arc<tokio::sync::Semaphore>,
	net_io_semaphore: Arc<tokio::sync::Semaphore>,
) -> anyhow::Result<()>
where
	I: IntoIterator<Item = P> + Send + 'static,
	P: AsRef<Path>,
{
	let (untagged_tx, mut untagged_rx) = tokio_unbounded_channel::<ChanUntagged>();
	let (tagged_tx, mut tagged_rx) = tokio_unbounded_channel::<ChanTagged>();
	let (lrc_tx, mut lrc_rx) = tokio_unbounded_channel::<ChanTaggedWithLyrics>();

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
	let lrc_fetcher: Arc<_> = lrc_fetcher.into();

	let ((), (), (), ()) = tokio::join!(
		{
			// Step 1: discover all the files.
			//
			// Walk the file structure in a separate task,
			// without blocking tokio's executors for async tasks.
			let disk_io_semaphore_local = disk_io_semaphore.clone();

			async move {
				// It's okay to not use the permit for anything,
				// because we just need to drop it to release it
				// after we're done with directory traversal.
				let _permit = disk_io_semaphore_local
					.acquire_owned()
					.await
					.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
				log::trace!("acquired a permit for step 1");

				if let Err(e) = spawn_blocking(move || {
					// Once the function completes,
					// we expect to be able to close the channel.
					traverse(untagged_tx, paths);
				})
				.await
				{
					join_fail_error!(e);
				}
			}
		},
		{
			// Step 2: read file tags, when possible.
			let disk_io_semaphore_local = disk_io_semaphore.clone();

			async move {
				let mut tagging_worker_handles = JoinSet::new();

				while let Some(path) = untagged_rx.recv().await {
					let _permit = disk_io_semaphore_local
						.acquire()
						.await
						.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
					log::trace!("acquired a permit for step 2");

					tagging_worker_handles.spawn_blocking(move || (load_file_tags(&path), path));
				}

				// TODO::perf consider using rayon's thread pool
				// instead of spawning a task for every file.
				while let Some(join_res) = tagging_worker_handles.join_next().await {
					match join_res {
						Ok((Ok(tagged_file), path)) => {
							log::info!(
								"successfully loaded file tags @ path {:?}",
								&path,
							);

							tagged_tx
								.send(tagged_file)
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
				}
			}
		},
		{
			// Step 3: use file tags to request lyrics.
			let net_io_semaphore_local = net_io_semaphore.clone();

			async move {
				let mut lrc_fetch_worker_handles = JoinSet::new();

				while let Some(tagged_file) = tagged_rx.recv().await {
					// First, attempt to get lyrics from the database...
					let tagged_file_data = (&tagged_file).into();
					log::trace!(
						"reading lyrics for {:?} from the database...",
						tagged_file_data,
					);
					match db_cache.get_lrc(&tagged_file_data) {
						Ok(Some(lrc)) => {
							log::info!(
								"successfully loaded lyrics from cache for {}",
								tagged_file_data,
							);

							lrc_tx
								.send((lrc, tagged_file))
								.expect(CHANNEL_SEND_EXPECT_MSG);
							// ...if that worked, move on.
							continue;
						},

						Ok(None) => {
							log::debug!(
								"couldn't find lyrics for track {} inside a db cache",
								tagged_file_data,
							);
						},

						Err(e) => {
							log::warn!(
								r#"db cache interaction failed with error "{}""#,
								e,
							);
						},
					}

					// ...if it didn't work, get/init network lyrics fetcher.
					let lrc_fetcher = lrc_fetcher.clone();

					let net_io_semaphore_local = net_io_semaphore_local.clone();
					lrc_fetch_worker_handles.spawn(async move {
						let _permit = net_io_semaphore_local
							.acquire()
							.await
							.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
						log::trace!("acquired a permit for step 3");

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
						Ok(Ok((
							LyricsServiceAcquisitionValue {
								lyrics,
								service_name,
							},
							tagged_file,
						))) => {
							let tagged_file_data: TaggedFileData = (&tagged_file).into();
							log::info!(
								r#"service "{}" successfully found lyrics data for {}"#,
								service_name,
								tagged_file_data,
							);

							let tagged_file_data = (&tagged_file).into();
							if let Err(e) = db_cache.insert_lrc(&tagged_file_data, lyrics.clone()) {
								log::warn!(
									r#"failed to insert lyrics for {} with error "{}", it will not be cached!"#,
									tagged_file_data,
									e,
								)
							}

							lrc_tx
								.send((lyrics, tagged_file))
								.expect(CHANNEL_SEND_EXPECT_MSG);
						},

						Ok(Err(service_errors)) => {
							for e in service_errors {
								log::warn!(r#"lyrics service error "{}""#, e);
							}
						},

						Err(join_err) => {
							join_fail_error!(join_err);
						},
					}
				}
			}
		},
		{
			// Step 4: write lyrics tags back to files.
			let disk_io_semaphore_local = disk_io_semaphore.clone();

			async move {
				let mut writing_worker_handles = JoinSet::new();

				while let Some((lyrics, mut tagged_file)) = lrc_rx.recv().await {
					let disk_io_semaphore_local = disk_io_semaphore_local.clone();
					let _permit = disk_io_semaphore_local
						.acquire_owned()
						.await
						.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
					log::trace!("acquired a permit for step 4");

					writing_worker_handles.spawn_blocking(move || {
						(
							update_file_lyrics_tag(&mut tagged_file, &lyrics),
							tagged_file,
						)
					});
				}

				while let Some(join_res) = writing_worker_handles.join_next().await {
					match join_res {
						Ok((Ok(()), tagged_file)) => {
							let tagged_file_data: TaggedFileData = (&tagged_file).into();
							log::info!(
								"successfully wrote tags for {}",
								tagged_file_data,
							);
						},
						Ok((Err(e), tagged_file)) => {
							let tagged_file_data: TaggedFileData = (&tagged_file).into();
							log::warn!(
								r#"failed to write tags for file {} with error "{}""#,
								tagged_file_data,
								e,
							);
						},
						Err(join_err) => {
							join_fail_error!(join_err);
						},
					}
				}
			}
		},
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
