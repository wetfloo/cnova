use std::sync::Arc;

use super::LyricsServiceError;
use crate::lyrics::Lyrics;
use crate::lyrics::TagData;
use crate::lyrics::service::LyricsFetchService;
use crate::lyrics::service::LyricsServiceResult;

#[derive(Debug)]
struct LrclibLyricsFetchService {
	http_client: Arc<reqwest::Client>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibLyricsResponse {
	id: u64,
	track_name: String,
	artist_name: String,
	album_name: String,
	/// Duration of a song in seconds.
	pub(crate) duration: u64,
	instrumental: bool,
	plain_lyrics: String,
	synced_lyrics: String,
}

impl LrclibLyricsFetchService {
	fn new<R>(http_client: R) -> Self
	where
		R: Into<Arc<reqwest::Client>>,
	{
		Self {
			http_client: http_client.into(),
		}
	}
}

impl LyricsFetchService for LrclibLyricsFetchService {
	fn request_lyrics(&self, data: &TagData) -> LyricsServiceResult {
		let url = reqwest::Url::parse_with_params(
			"https://lrclib.net/api/get/",
			[
				("track_name", &data.title),
				("artist_name", &data.artist),
				("album_name", &data.album),
				(
					"duration",
					&data.duration.as_secs().to_string(),
				),
			],
		)
		.expect(
			"since we typed this url by hand without user input, we expect it to always parse correctly",
		);

		let http_client = self.http_client.clone();

		Box::pin(async move {
			let response: LrclibLyricsResponse = http_client
				.get(url)
				.send()
				.await
				.map_err(LyricsServiceError::Network)?
				.json()
				.await
				.map_err(LyricsServiceError::Parse)?;

			Ok(response.into())
		})
	}
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
