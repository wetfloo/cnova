use super::LyricsServiceError;
use crate::lyrics::Lyrics;
use crate::lyrics::TagData;
use crate::lyrics::service::LyricsFetchService;
use crate::lyrics::service::LyricsServiceResult;

#[derive(Debug)]
struct DbLyricsFetchService {}

impl DbLyricsFetchService {}

struct DbLyricsResponse {}

impl LyricsFetchService for DbLyricsFetchService {
	fn request_lyrics(&self, data: &TagData) -> LyricsServiceResult {
		todo!()
	}
}

impl From<DbLyricsResponse> for Lyrics {
	fn from(value: DbLyricsResponse) -> Self {
		todo!()
	}
}
