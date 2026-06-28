mod lrclib;

use std::any::TypeId;
use std::fmt;
use std::pin::Pin;

pub(crate) use lrclib::LrclibLyricsFetchService;

use crate::lyrics::Lyrics;
use crate::lyrics::TaggedFileData;

pub(crate) type LyricsServiceResult =
	Pin<Box<dyn Future<Output = Result<Lyrics, LyricsServiceErrorInner>> + Send + Sync + 'static>>;

pub(crate) trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(&self, data: &TaggedFileData) -> LyricsServiceResult;
}

#[derive(Debug, thiserror::Error)]
#[error("service {:#?} failed to request due to error {}", .service, .inner)]
pub(crate) struct LyricsServiceError {
	pub(crate) service: TypeId,
	pub(crate) inner: LyricsServiceErrorInner,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum LyricsServiceErrorInner {
	#[error("failed a network request with {:?}", .0)]
	Network(#[source] reqwest::Error),
	#[error("failed to parse, with {:?}", .0)]
	Parse(#[source] reqwest::Error),
	#[error("unknown error")]
	Unknown,
}
