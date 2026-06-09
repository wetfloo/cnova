pub mod lrclib;

use crate::lyrics::LyricsServiceRequestResult;
use crate::lyrics::TagData;

use std::fmt;

pub trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(&self, data: &TagData) -> LyricsServiceRequestResult;
}
