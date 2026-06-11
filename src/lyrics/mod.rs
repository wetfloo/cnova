mod fetcher;
mod service;

use std::time::Duration;

#[derive(Debug, PartialEq)]
pub enum Lyrics {
	Synced(String),
	Unsynced(String),
	Instrumental,
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
	use std::assert_matches;
	use std::future;
	use std::time::Duration;

	use super::Lyrics;
	use super::TagData;
	use crate::lyrics::fetcher::LyricsFetcherBuilder;
	use crate::lyrics::service::LyricsFetchService;
	use crate::lyrics::service::LyricsServiceError;
	use crate::lyrics::service::LyricsServiceResult;

	#[derive(Debug, Default)]
	struct OkLyricsFetcher;

	impl LyricsFetchService for OkLyricsFetcher {
		fn request_lyrics(&self, data: &TagData) -> LyricsServiceResult {
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
		fn request_lyrics(&self, data: &TagData) -> LyricsServiceResult {
			Box::pin(future::ready(Err(
				LyricsServiceError::Unknown,
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

		assert_matches!(
			Ok::<_, LyricsServiceError>(Lyrics::Unsynced(
				"These are test lyrics for a song Fireal by Deftones.".to_owned()
			)),
			res
		);
	}
}
