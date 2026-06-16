mod db;
mod lrclib;

use std::fmt;
use std::pin::Pin;
use std::sync::LazyLock;

pub(crate) use db::DbLyricsFetchService;
pub(crate) use lrclib::LrclibLyricsFetchService;

use crate::lyrics::Lyrics;
use crate::lyrics::TagData;
use crate::lyrics::TaggedFileData;

pub(crate) type LyricsServiceResult =
	Pin<Box<dyn Future<Output = Result<Lyrics, LyricsServiceError>> + Send + Sync>>;

pub(crate) trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(&self, data: &TaggedFileData) -> LyricsServiceResult;
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum LyricsServiceError {
	#[error("failed a network request")]
	Network(#[source] reqwest::Error),
	#[error("failed to parse")]
	Parse(#[source] reqwest::Error),
	#[error("unknown error")]
	Unknown,
}
