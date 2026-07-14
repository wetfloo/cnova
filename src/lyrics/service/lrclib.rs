use std::sync::Arc;

use crate::lyrics::Lyrics;
use crate::lyrics::TaggedFile;
use crate::lyrics::service::LyricsFetchService;
use crate::lyrics::service::LyricsServiceErrorInner;
use crate::lyrics::service::LyricsServiceResult;

use wetutil::impl_gen;

pub(crate) struct LrclibLyricsFetchService {
	http_client: Arc<reqwest::Client>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibLyricsResponse {
	id: Option<u64>,
	track_name: Option<String>,
	artist_name: Option<String>,
	album_name: Option<String>,
	/// Duration of a song in seconds.
	duration: Option<f64>,
	#[serde(default = "default_instrumental")]
	instrumental: bool,
	plain_lyrics: Option<String>,
	synced_lyrics: Option<String>,
}

#[inline]
fn default_instrumental() -> bool {
	false
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

impl_gen::debug::from_type_name!(LrclibLyricsFetchService);

impl LyricsFetchService for LrclibLyricsFetchService {
	fn request_lyrics(&self, data: &dyn TaggedFile) -> LyricsServiceResult {
		let mut params = Vec::with_capacity(4);

		let title = data.title();
		if let Some(v) = title.as_deref() {
			params.push(("track_name", v));
		}

		let artist = data.artist();
		if let Some(v) = artist.as_deref() {
			params.push(("artist_name", v));
		}

		let album = data.album();
		if let Some(v) = album.as_deref() {
			params.push(("album_name", v));
		}

		let duration = data.duration().as_secs().to_string();
		params.push(("duration", &duration));

		let url = reqwest::Url::parse_with_params("https://lrclib.net/api/get", params).expect(
			"since we typed this url by hand without user input, we expect it to always parse correctly",
		);

		let http_client = self.http_client.clone();

		Box::pin(async move {
			let response: LrclibLyricsResponse = http_client
				.get(url)
				.send()
				.await
				.map_err(LyricsServiceErrorInner::Network)?
				.json()
				.await
				.map_err(LyricsServiceErrorInner::Parse)?;

			Ok(response.into())
		})
	}

	fn name(&self) -> &'static str {
		"lrclib.net readonly service"
	}
}

impl From<LrclibLyricsResponse> for Lyrics {
	fn from(value: LrclibLyricsResponse) -> Self {
		value
			.synced_lyrics
			.map(Lyrics::Synced)
			.or_else(|| value.plain_lyrics.map(Lyrics::Unsynced))
			.filter(|_| !value.instrumental)
			.unwrap_or(Lyrics::Instrumental)
	}
}
