pub(crate) mod fetcher;
pub(crate) mod service;

use std::borrow::Cow;
use std::time::Duration;

use lofty::file::AudioFile as _;
use lofty::file::TaggedFileExt as _;
use lofty::tag::Accessor as _;

#[derive(Debug, PartialEq)]
pub(crate) enum Lyrics {
	Synced(String),
	Unsynced(String),
	Instrumental,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TagData<'a> {
	pub(crate) artist: Option<Cow<'a, str>>,
	pub(crate) album: Option<Cow<'a, str>>,
	pub(crate) title: Option<Cow<'a, str>>,
	pub(crate) duration: Duration,
}

impl<'i, 'o, T> From<&'i lofty::file::BoundTaggedFile<T>> for TagData<'o>
where
	'i: 'o,
{
	fn from(value: &'i lofty::file::BoundTaggedFile<T>) -> Self {
		let artist = value
			.primary_tag()
			.and_then(|t| t.artist());
		let album = value
			.primary_tag()
			.and_then(|t| t.album());
		let title = value
			.primary_tag()
			.and_then(|t| t.title());
		let duration = value.properties().duration();

		Self {
			artist,
			album,
			title,
			duration,
		}
	}
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
					"These are test lyrics for a song {:?} by {:?}.",
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
			artist: Some("Deftones".into()),
			album: Some("Adrenaline".into()),
			title: Some("Fireal".into()),
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
