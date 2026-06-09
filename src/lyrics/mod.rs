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

impl From<LrclibLyricsResponse> for Lyrics {
	fn from(value: LrclibLyricsResponse) -> Self {
		if value.instrumental {
			return Self::Instrumental;
		}

		let LrclibLyricsResponse {
			synced_lyrics,
			plain_lyrics,
			..
		} = value;
		if !synced_lyrics.trim().is_empty() {
			Self::Synced(synced_lyrics)
		} else {
			Self::Unsynced(plain_lyrics)
		}
	}
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

pub trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(&self, data: &TagData) -> LyricsServiceRequestResult;
}

#[derive(Default, Debug)]
pub struct LyricsFetcher {
	services: Vec<Box<dyn LyricsFetchService>>,
}

impl LyricsFetcher {
	async fn request_lyrics(&self, data: &TagData) -> LyricsFetcherResult {
		let mut errors = Vec::with_capacity(0);

		for service in self.services.iter() {
			match service.request_lyrics(data).await {
				Ok(v) => return Ok(v),
				Err(e) => errors.push(e),
			}
		}

		Err(errors)
	}
}

#[derive(Default, Debug)]
pub struct LyricsFetcherBuilder {
	fetcher: LyricsFetcher,
}

impl LyricsFetcherBuilder {
	pub fn new(service: Box<dyn LyricsFetchService>) -> Self {
		Self {
			fetcher: LyricsFetcher {
				services: vec![service],
			},
		}
	}

	pub fn add_service(mut self, service: Box<dyn LyricsFetchService>) -> Self {
		self.fetcher.services.push(service);

		self
	}

	pub fn build(self) -> LyricsFetcher {
		self.fetcher
	}
}

#[cfg(test)]
mod test {
	use std::future;
	use std::time::Duration;

	use super::Lyrics;
	use super::LyricsFetchError;
	use super::LyricsFetchService;
	use super::LyricsFetcher;
	use super::LyricsFetcherBuilder;
	use super::LyricsServiceRequestResult;
	use super::TagData;

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
