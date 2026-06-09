mod fetcher;
mod service;

use service::lrclib::LrclibLyricsResponse;
use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::LazyLock;
use std::time::Duration;

pub(crate) type LyricsServiceRequestResult =
	Pin<Box<dyn Future<Output = Result<Lyrics, LyricsFetchError>> + Send + Sync>>;
pub(crate) type LyricsFetcherResult = Result<Lyrics, Vec<LyricsFetchError>>;

pub(super) static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

#[derive(Debug, PartialEq)]
pub enum Lyrics {
	Synced(String),
	Unsynced(String),
	Instrumental,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LyricsFetchError {
	#[error("unknown error")]
	Unknown,
}

#[derive(Debug, PartialEq)]
// TODO::perf consider using string slice refs here to implement zero-copy
pub struct TagData {
	pub artist: String,
	pub album: String,
	pub title: String,
	pub duration: Duration,
}

#[cfg(test)]
mod test {
	use std::future;
	use std::time::Duration;

	use super::Lyrics;
	use super::LyricsFetchError;
	use super::LyricsServiceRequestResult;
	use super::TagData;
	use crate::lyrics::fetcher::LyricsFetcher;
	use crate::lyrics::fetcher::LyricsFetcherBuilder;
	use crate::lyrics::service::LyricsFetchService;

	#[derive(Debug, Default)]
	struct OkLyricsFetcher;

	impl LyricsFetchService for OkLyricsFetcher {
		fn request_lyrics(&self, data: &TagData) -> LyricsServiceRequestResult {
			Box::pin(future::ready(Ok(Lyrics::Unsynced(
				format!(
					"These are test lyrics for a song {} by {}.",
					data.title, data.artist,
				),
			))))
		}
	}

	#[derive(Debug, Default)]
	struct ErrInstrumentalLyricsFetcher;

	impl LyricsFetchService for ErrInstrumentalLyricsFetcher {
		fn request_lyrics(&self, data: &TagData) -> LyricsServiceRequestResult {
			Box::pin(future::ready(Err(
				LyricsFetchError::Unknown,
			)))
		}
	}

	#[tokio::test]
	async fn test_ok_only_service() {
		let lyrics_fetcher = LyricsFetcherBuilder::new(Box::new(OkLyricsFetcher)).build();
		let tag_data = TagData {
			artist: "Deftones".into(),
			album: "Adrenaline".into(),
			title: "Fireal".into(),
			duration: Duration::from_secs((6 * 60) + 32),
		};

		let res = lyrics_fetcher
			.request_lyrics(&tag_data)
			.await;

		assert_eq!(
			Ok(Lyrics::Unsynced(
				"These are test lyrics for a song Fireal by Deftones.".into()
			)),
			res
		);
	}
}
