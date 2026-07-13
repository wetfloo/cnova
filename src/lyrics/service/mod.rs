mod lrclib;

use std::any::type_name;
use std::fmt;
use std::pin::Pin;

pub(crate) use lrclib::LrclibLyricsFetchService;

use crate::lyrics::Lyrics;
use crate::lyrics::TaggedFile;

pub(crate) type LyricsServiceResult =
	Pin<Box<dyn Future<Output = Result<Lyrics, LyricsServiceErrorInner>> + Send + Sync + 'static>>;

pub(crate) trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(&self, data: &dyn TaggedFile) -> LyricsServiceResult;

	/// Used to get the name of this service to print to the user.
	fn name(&self) -> &'static str {
		type_name::<Self>()
	}
}

/// Lyrics with the id of service
/// that was able to acquire said lyrics.
pub(crate) struct LyricsServiceAcquisitionValue {
	pub(crate) lyrics: Lyrics,
	pub(crate) service_name: &'static str,
}

/// Lyrics error with the id of service
/// that failed able to acquire said lyrics.
#[derive(Debug, thiserror::Error)]
#[error(r#"service "{}" failed to request due to error {}"#, .service_name, .src_err)]
pub(crate) struct LyricsServiceError {
	#[source]
	pub(crate) src_err: LyricsServiceErrorInner,
	pub(crate) service_name: &'static str,
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
