use super::HTTP_CLIENT;
use super::LyricsServiceError;
use crate::lyrics::Lyrics;
use crate::lyrics::TagData;
use crate::lyrics::service::LyricsFetchService;
use crate::lyrics::service::LyricsServiceResult;

#[derive(Debug)]
pub(super) struct LrclibLyricsFetchService;

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
		Box::pin(async {
			let response: LrclibLyricsResponse = HTTP_CLIENT
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
