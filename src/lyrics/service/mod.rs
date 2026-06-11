pub mod lrclib;

use std::fmt;
use std::pin::Pin;
use std::sync::LazyLock;

use crate::lyrics::Lyrics;
use crate::lyrics::TagData;

pub type LyricsServiceResult =
	Pin<Box<dyn Future<Output = Result<Lyrics, LyricsServiceError>> + Send + Sync>>;

pub(super) static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

pub trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(&self, data: &TagData) -> LyricsServiceResult;
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LyricsServiceError {
	#[error("unknown error")]
	Unknown,
}
