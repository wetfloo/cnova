pub mod lrclib;

use crate::lyrics::Lyrics;
use crate::lyrics::TagData;

use std::fmt;
use std::pin::Pin;
use std::sync::LazyLock;

pub(crate) type LyricsServiceRequestResult =
	Pin<Box<dyn Future<Output = Result<Lyrics, LyricsFetchError>> + Send + Sync>>;

pub(super) static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

pub trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(&self, data: &TagData) -> LyricsServiceRequestResult;
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LyricsFetchError {
	#[error("unknown error")]
	Unknown,
}
