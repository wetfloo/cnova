use super::HTTP_CLIENT;
use super::LyricsFetchError;
use super::LyricsFetchService;
use super::LyricsRequestRes;
use super::TagData;

#[derive(Debug)]
pub struct LrclibLyricsFetchService;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LrclibLyricsResponse {
	pub(super) id: u64,
	pub(super) track_name: String,
	pub(super) artist_name: String,
	pub(super) album_name: String,
	/// Duration of a song in seconds.
	pub(super) duration: u64,
	pub(super) instrumental: bool,
	pub(super) plain_lyrics: String,
	pub(super) synced_lyrics: String,
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
