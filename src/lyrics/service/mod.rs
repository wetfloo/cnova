pub(super) mod lrclib;

use std::fmt;
use std::pin::Pin;
use std::sync::LazyLock;

use crate::lyrics::Lyrics;
use crate::lyrics::TagData;

pub(super) type LyricsServiceResult =
	Pin<Box<dyn Future<Output = Result<Lyrics, LyricsServiceError>> + Send + Sync>>;

pub(super) static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

pub(super) trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(&self, data: &TagData) -> LyricsServiceResult;
}

#[derive(Debug, thiserror::Error)]
pub(super) enum LyricsServiceError {
	#[error("failed a network request")]
	Network(#[source] reqwest::Error),
	#[error("failed to parse")]
	Parse(#[source] reqwest::Error),
	#[error("unknown error")]
	Unknown,
}
