use core::fmt;
use std::any::type_name;

use super::LyricsServiceError;
use crate::lyrics::Lyrics;
use crate::lyrics::TagData;
use crate::lyrics::service::LyricsFetchService;
use crate::lyrics::service::LyricsServiceResult;

use pretty_type_name::pretty_type_name;

struct DbLyricsFetchService {
	db_conn: sqlite::ConnectionThreadSafe,
}

impl DbLyricsFetchService {}

impl fmt::Debug for DbLyricsFetchService {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&pretty_type_name::<Self>())
	}
}

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
