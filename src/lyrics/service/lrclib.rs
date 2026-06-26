use std::fmt;
use std::sync::Arc;

use pretty_type_name::pretty_type_name;

use super::LyricsServiceError;
use crate::lyrics::Lyrics;
use crate::lyrics::TaggedFileData;
use crate::lyrics::service::LyricsFetchService;
use crate::lyrics::service::LyricsServiceResult;

pub(crate) struct LrclibLyricsFetchService {
	http_client: Arc<reqwest::Client>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibLyricsResponse {
	id: Option<u64>,
	track_name: String,
	artist_name: String,
	album_name: String,
	/// Duration of a song in seconds.
	duration: u64,
	instrumental: bool,
	plain_lyrics: String,
	synced_lyrics: String,
}

impl LrclibLyricsFetchService {
	pub(crate) fn new<R>(http_client: R) -> Self
	where
		R: Into<Arc<reqwest::Client>>,
	{
		Self {
			http_client: http_client.into(),
		}
	}
}

impl fmt::Debug for LrclibLyricsFetchService {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&pretty_type_name::<Self>())
	}
}

impl LyricsFetchService for LrclibLyricsFetchService {
	fn request_lyrics(&self, data: &TaggedFileData) -> LyricsServiceResult {
		let mut params = Vec::with_capacity(4);

		if let Some(v) = data.title() {
			params.push(("track_name", v));
		}
		if let Some(v) = data.artist() {
			params.push(("artist_name", v));
		}
		if let Some(v) = data.album() {
			params.push(("album_name", v));
		}
		let duration = &data.duration.as_secs().to_string();
		params.push(("duration", duration));

		let url = reqwest::Url::parse_with_params("https://lrclib.net/api/get/", params).expect(
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
