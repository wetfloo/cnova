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

	let mut join_set = JoinSet::new();

	// Step 1: discover all the files.
	//
	// Walk the file structure in a separate task,
	// without blocking tokio's executors for async tasks.
	let disk_io_semaphore_step_1 = disk_io_semaphore.clone();
	join_set.spawn(async move {
		// It's okay to not use the permit for anything,
		// because we just need to drop it to release it
		// after we're done with directory traversal.
		let _permit = disk_io_semaphore_step_1
			.acquire_owned()
			.await
			.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
		log::trace!("acquired a permit for step 1");

		let join_res = spawn_blocking(move || {
			traverse(&untagged_tx, paths);
		})
		.await;
		if let Err(e) = join_res {
			join_fail_error!(e);
		}
	});

	// Step 2: read file tags, when possible.
	let disk_io_semaphore_step_2 = disk_io_semaphore.clone();
	join_set.spawn(async move {
		let mut tagging_worker_handles = JoinSet::new();

		while let Some(path) = untagged_rx.recv().await {
			let disk_io_semaphore_step_2 = disk_io_semaphore_step_2.clone();
			let _permit = disk_io_semaphore_step_2
				.acquire_owned()
				.await
				.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
			log::trace!("acquired a permit for step 2");
			let tagged_tx = tagged_tx.clone();

			// TODO::perf consider using rayon's thread pool
			// instead of spawning a task for every file.
			tagging_worker_handles.spawn_blocking(move || match load_file_tags(&path) {
				Ok(tagged_file) => {
					log::info!(
						"successfully loaded file tags @ path {:?}",
						path,
					);

					tagged_tx
						.send(tagged_file)
						.expect(CHANNEL_SEND_EXPECT_MSG);
				},

				Err(GuessFileError::InvalidFileType { ft }) => {
					log::info!(
						r#"file type "{}" not recognized @ path {:?}; skipping..."#,
						ft,
						path,
					);
				},

				Err(e) => {
					log::warn!(
						r#"error "{}": failed to load file tags @ path {:?}; skipping..."#,
						e,
						path,
					);
				},
			});
		}

		while let Some(join_res) = tagging_worker_handles.join_next().await {
			if let Err(e) = join_res {
				join_fail_error!(e);
			}
		}
	});

	// Step 3: use file tags to request lyrics.
	let db_local_set = tokio::task::LocalSet::new();
	let net_io_semaphore_step_3 = net_io_semaphore.clone();
	join_set.spawn_local_on(
		async move {
			let mut lrc_fetch_worker_handles = JoinSet::new();

			while let Some(tagged_file) = tagged_rx.recv().await {
				let lrc_tx = lrc_tx.clone();

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

				let net_io_semaphore_step_3 = net_io_semaphore_step_3.clone();
				lrc_fetch_worker_handles.spawn(async move {
					let _permit = net_io_semaphore_step_3
						.acquire_owned()
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
					Ok(Ok((lyrics, tagged_file))) => {
						let tagged_file_data = (&tagged_file).into();
						log::info!(
							"successfully found lyrics data for {}",
							tagged_file_data
						);

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

					Err(e) => {
						join_fail_error!(e);
					},
				}
			}
		},
		&db_local_set,
	);

	// Step 4: write lyrics tags back to files.
	let disk_io_semaphore_step_4 = disk_io_semaphore.clone();
	join_set.spawn(async move {
		let mut writing_worker_handles = JoinSet::<lofty::error::Result<()>>::new();

		while let Some((lyrics, mut tagged_file)) = lrc_rx.recv().await {
			let disk_io_semaphore_step_4 = disk_io_semaphore_step_4.clone();
			let _permit = disk_io_semaphore_step_4
				.acquire_owned()
				.await
				.expect(DISK_IO_SEMAPHORE_EXPECT_MSG);
			log::trace!("acquired a permit for step 4");

			writing_worker_handles.spawn_blocking(move || {
				update_file_lyrics_tag(&mut tagged_file, &lyrics)?;

				let tagged_file_data: TaggedFileData = (&tagged_file).into();
				log::info!(
					"successfully wrote tags for {}",
					tagged_file_data,
				);
				Ok(())
			});
		}

		while let Some(join_res) = writing_worker_handles.join_next().await {
			if let Err(e) = join_res {
				join_fail_error!(e);
			}
		}
	});

	db_local_set.await;
	while let Some(join_res) = join_set.join_next().await {
		if let Err(e) = join_res {
			join_fail_error!(e);
		}
	}

	Ok(())
}

/// Traverse `paths` recursively,
/// sending any file (not a directory!) to `untagged_tx`.
fn traverse<I, P>(untagged_tx: &Sender<ChanUntagged>, paths: I)
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
		.open(path.as_ref())
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
