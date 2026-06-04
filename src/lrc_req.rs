use std::cmp;
use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::LazyLock;
use std::time::Duration;

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

pub(crate) type LyricsRequestRes =
	Pin<Box<dyn Future<Output = Result<Lyrics, LyricsFetchError>> + Send + Sync>>;
pub(crate) type LyricsRes = Result<Lyrics, Vec<LyricsFetchError>>;

#[derive(Debug, PartialEq)]
// TODO::perf consider using string slice refs here to implement zero-copy
pub struct TagData {
	pub artist: String,
	pub album: String,
	pub title: String,
	pub duration: Duration,
}

pub struct TaggedFileInfo {
	pub file: lofty::file::TaggedFile,
	pub path: PathBuf,
}

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

pub trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(&self, data: &TagData) -> LyricsRequestRes;
}

#[derive(Debug)]
pub struct LrclibLyricsFetchService;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LrclibLyricsResponse {
	id: u64,
	track_name: String,
	artist_name: String,
	album_name: String,
	/// Duration of a song in seconds.
	duration: u64,
	instrumental: bool,
	plain_lyrics: String,
	synced_lyrics: String,
}

impl LyricsFetchService for LrclibLyricsFetchService {
	fn request_lyrics(&self, data: &TagData) -> LyricsRequestRes {
		let mut url = reqwest::Url::parse_with_params(
			"https://lrclib.net/api/get/",
			[
				("track_name", &data.title),
				("artist_name", &data.artist),
				("album_name", &data.album),
			],
		)
		.expect(
			"since we typed this url by hand without user input, we expect it to always parse correctly",
		);
		Box::pin(async {
			let response: LrclibLyricsResponse = HTTP_CLIENT
				.get(url)
				.send()
				.await
				// TODO::error_handling: make a better user facing error when we're done here.
				.map_err(|_| LyricsFetchError::Unknown)?
				.json()
				.await
				// TODO::error_handling: make a better user facing error when we're done here.
				.map_err(|_| LyricsFetchError::Unknown)?;

			Ok(response.into())
		})
	}
}

#[derive(Default, Debug)]
pub struct LyricsFetcher {
	services: Vec<Box<dyn LyricsFetchService>>,
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

impl LyricsFetcher {
	async fn request_lyrics(&self, data: &TagData) -> LyricsRes {
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

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LyricsFetchError {
	#[error("unknown error")]
	Unknown,
}

pub struct FakeLyricsFetcher;

#[cfg(test)]
mod test {
	use std::future;
	use std::time::Duration;

	use super::Lyrics;
	use super::LyricsFetchError;
	use super::LyricsFetchService;
	use super::LyricsFetcher;
	use super::LyricsFetcherBuilder;
	use super::LyricsRequestRes;
	use super::TagData;
	use super::TaggedFileInfo;

	#[derive(Debug, Default)]
	struct OkLyricsFetcher;

	impl LyricsFetchService for OkLyricsFetcher {
		fn request_lyrics(&self, data: &TagData) -> LyricsRequestRes {
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
		fn request_lyrics(&self, data: &TagData) -> LyricsRequestRes {
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
