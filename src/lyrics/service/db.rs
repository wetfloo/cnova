use core::fmt;
use std::any::type_name;

use pretty_type_name::pretty_type_name;

use super::LyricsServiceError;
use crate::lyrics::Lyrics;
use crate::lyrics::TagData;
use crate::lyrics::TaggedFileData;
use crate::lyrics::service::LyricsFetchService;
use crate::lyrics::service::LyricsServiceResult;

pub(crate) struct DbLyricsFetchService {
	db_conn: sqlite::ConnectionThreadSafe,
}

impl DbLyricsFetchService {
	pub(crate) fn new(db_conn: sqlite::ConnectionThreadSafe) -> Self {
		Self { db_conn }
	}
}

impl fmt::Debug for DbLyricsFetchService {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&pretty_type_name::<Self>())
	}
}

struct DbLyricsResponse {}

impl LyricsFetchService for DbLyricsFetchService {
	fn request_lyrics(&self, data: &TaggedFileData) -> LyricsServiceResult {
		todo!()
	}
}

impl From<DbLyricsResponse> for Lyrics {
	fn from(value: DbLyricsResponse) -> Self {
		todo!()
	}
}
