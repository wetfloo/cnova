use std::fmt;
use std::time;
use std::time::SystemTime;
use std::time::SystemTimeError;
use std::time::UNIX_EPOCH;

use pretty_type_name::pretty_type_name;
use strum::IntoDiscriminant;

use crate::lyrics;
use crate::lyrics::Lyrics;
use crate::lyrics::LyricsDiscriminants;
use crate::lyrics::TaggedFileData;

pub(crate) type DbConnection = sqlite::Connection;

pub(crate) struct DbCache(DbCacheInner);

impl fmt::Debug for DbCache {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&pretty_type_name::<Self>())
	}
}

#[ouroboros::self_referencing]
struct DbCacheInner {
	db_conn: DbConnection,
	#[borrows(db_conn)]
	#[covariant]
	get_lyrics_statement: sqlite::Statement<'this>,
	#[borrows(db_conn)]
	#[covariant]
	insert_lyrics_statement: sqlite::Statement<'this>,
}

impl DbCache {
	pub(crate) fn new(db_conn: DbConnection) -> Result<Self, sqlite::Error> {
		db_conn.execute(queries::INIT_TABLE_LRC)?;

		let inner = DbCacheInner::try_new(
			db_conn,
			|db_conn: &DbConnection| db_conn.prepare(queries::GET_LRC),
			|db_conn: &DbConnection| db_conn.prepare(queries::INSERT_LRC),
		)?;

		Ok(Self(inner))
	}

	pub(crate) fn get_lrc(
		&mut self,
		tagged_file_data: &TaggedFileData,
	) -> Result<Option<lyrics::Lyrics>, DbCacheLyricsError> {
		match self
			.0
			.with_get_lyrics_statement_mut(|statement| {
				Self::get_lrc_internal(statement, tagged_file_data)
			})? {
			Some((lyrics, status)) => LyricsDiscriminants::from_repr(status)
				.map(|discriminant| {
					Some(match discriminant {
						LyricsDiscriminants::Synced => Lyrics::Synced(lyrics),
						LyricsDiscriminants::Unsynced => Lyrics::Unsynced(lyrics),
						LyricsDiscriminants::Instrumental => Lyrics::Instrumental,
					})
				})
				.ok_or(DbCacheLyricsError::StatusMismatch(
					status,
				)),
			None => Ok(None),
		}
	}

	fn get_lrc_internal(
		statement: &mut sqlite::Statement,
		tagged_file_data: &TaggedFileData,
	) -> Result<Option<(String, i64)>, DbCacheLyricsError> {
		// Necessary for repeated calls.
		statement.reset()?;

		statement.bind((
			":artist",
			tagged_file_data
				.artist()
				.unwrap_or_default(),
		))?;
		statement.bind((
			":album",
			tagged_file_data
				.album()
				.unwrap_or_default(),
		))?;
		statement.bind((
			":title",
			tagged_file_data
				.title()
				.unwrap_or_default(),
		))?;

		Ok(match statement.next()? {
			sqlite::State::Row => {
				let lyrics: String = statement.read("lyrics")?;
				let status: i64 = statement.read("status")?;
				Some((lyrics, status))
			},
			sqlite::State::Done => None,
		})
	}

	pub(crate) fn insert_lrc(
		&mut self,
		tagged_file_data: &TaggedFileData,
		lyrics: Lyrics,
	) -> Result<(), DbCacheLyricsError> {
		self.0
			.with_insert_lyrics_statement_mut(|statement| {
				Self::insert_lrc_internal(statement, tagged_file_data, lyrics)
			})
	}

	fn insert_lrc_internal(
		statement: &mut sqlite::Statement,
		tagged_file_data: &TaggedFileData,
		lyrics: Lyrics,
	) -> Result<(), DbCacheLyricsError> {
		// Necessary for repeated calls.
		// Yes, even for inserts.
		statement.reset();

		statement.bind((
			":lyrics",
			lyrics.as_str().unwrap_or_default(),
		))?;
		statement.bind((
			":artist",
			tagged_file_data
				.artist()
				.unwrap_or_default(),
		))?;
		statement.bind((
			":album",
			tagged_file_data
				.album()
				.unwrap_or_default(),
		))?;
		statement.bind((
			":title",
			tagged_file_data
				.title()
				.unwrap_or_default(),
		))?;
		statement.bind((
			":duration_secs",
			tagged_file_data.duration.as_secs_f64(),
		))?;
		statement.bind((
			":timestamp",
			SystemTime::now()
				.duration_since(UNIX_EPOCH)?
				.as_secs_f64(),
		))?;
		statement.bind((":status", lyrics.discriminant() as i64))?;

		match statement.next()? {
			sqlite::State::Row => unreachable!(),
			sqlite::State::Done => Ok(()),
		}
	}
}

impl TryFrom<DbConnection> for DbCache {
	type Error = sqlite::Error;

	#[inline]
	fn try_from(value: DbConnection) -> Result<Self, Self::Error> {
		Self::new(value)
	}
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DbCacheLyricsError {
	#[error("Invalid lyrics status in the database: {}", .0)]
	StatusMismatch(i64),
	#[error(transparent)]
	Sqlite(#[from] sqlite::Error),
	#[error(transparent)]
	Time(#[from] SystemTimeError),
}

pub(super) mod queries {
	pub(super) const INIT_TABLE_LRC: &str = "\
		CREATE TABLE IF NOT EXISTS lrc (\
			lyrics TEXT NOT NULL, \
			artist TEXT NOT NULL, \
			album TEXT NOT NULL, \
			title TEXT NOT NULL, \
			duration_secs REAL NOT NULL, \
			timestamp REAL NOT NULL, \
			status INTEGER NOT NULL\
		) STRICT;\
	";

	pub(super) const GET_LRC: &str = "\
		SELECT lyrics, status \
		FROM lrc \
		WHERE artist = :artist \
			AND album = :album \
			AND title = :title \
		LIMIT 1;\
	";

	pub(super) const INSERT_LRC: &str = "\
		INSERT INTO lrc VALUES (\
			:lyrics, \
			:artist, \
			:album, \
			:title, \
			:duration_secs, \
			:timestamp, \
			:status\
		);\
	";
}

#[cfg(test)]
mod test {
	use std::assert_matches;
	use std::borrow::Cow;
	use std::fmt::format;
	use std::time::Duration;

	use crate::lyrics::Lyrics;
	use crate::lyrics::TagData;
	use crate::lyrics::TaggedFileData;
	use crate::lyrics::db::DbCache;
	use crate::lyrics::db::DbCacheLyricsError;
	use crate::lyrics::db::queries;

	macro_rules! init_cache {
		() => {{
			let res = sqlite::Connection::open(":memory:").and_then(DbCache::new);

			assert_matches!(res, Ok(_));

			res.expect("we've just verified that this value is Ok")
		}};
	}

	mod test_data {
		use std::fmt;
		use std::time::Duration;

		pub(super) fn artist(index: impl fmt::Display) -> String {
			format!("test artist {}", index)
		}
		pub(super) fn album(index: impl fmt::Display) -> String {
			format!("test album {}", index)
		}
		pub(super) fn title(index: impl fmt::Display) -> String {
			format!("test title {}", index)
		}
		pub(super) const fn duration(index: u8) -> Duration {
			Duration::from_secs(42 * (index as u64))
		}

		pub(super) fn lyrics(index: impl fmt::Display) -> String {
			format!("test lyrics {}", index)
		}
	}

	#[test]
	fn test_insert_and_get_twice() {
		let mut cache = init_cache!();

		let tag_data = TagData {
			artist: Some((test_data::artist(1)).into()),
			album: Some((test_data::album(1)).into()),
			title: Some((test_data::title(1)).into()),
		};
		let tagged_file_data = TaggedFileData {
			tag_data,
			duration: test_data::duration(1),
		};

		assert_matches!(
			cache.insert_lrc(
				&tagged_file_data,
				Lyrics::Synced(test_data::lyrics(1)),
			),
			Ok(()),
			"must be able to insert values into the database",
		);
		assert_eq!(
			cache
				.get_lrc(&tagged_file_data)
				.ok()
				.flatten(),
			Some(Lyrics::Synced(test_data::lyrics(1))),
			"must be able to get track metadata from the database",
		);
		assert_eq!(
			cache
				.get_lrc(&tagged_file_data)
				.ok()
				.flatten(),
			Some(Lyrics::Synced(test_data::lyrics(1))),
			"must be able to get the same track metadata from the database repeatedly",
		);
	}

	#[test]
	fn test_insert_two_and_get() {
		let mut cache = init_cache!();

		let tag_data = TagData {
			artist: Some(test_data::artist(1).into()),
			album: Some(test_data::album(1).into()),
			title: Some(test_data::title(1).into()),
		};
		let tagged_file_data = TaggedFileData {
			tag_data,
			duration: test_data::duration(1),
		};
		let tag_data_2 = TagData {
			artist: Some(test_data::artist(2).into()),
			album: Some(test_data::album(2).into()),
			title: Some(test_data::title(2).into()),
		};
		let tagged_file_data_2 = TaggedFileData {
			tag_data: tag_data_2,
			duration: test_data::duration(2),
		};

		assert_matches!(
			cache.insert_lrc(
				&tagged_file_data,
				Lyrics::Synced(test_data::lyrics(2)),
			),
			Ok(()),
			"must be able to insert values into the database",
		);
		assert_matches!(
			cache.insert_lrc(
				&tagged_file_data_2,
				Lyrics::Synced(test_data::lyrics(2)),
			),
			Ok(()),
			"must be able to insert values into the database",
		);
		assert_eq!(
			cache
				.get_lrc(&tagged_file_data)
				.ok()
				.flatten(),
			Some(Lyrics::Synced(test_data::lyrics(2))),
			"must be able to get track metadata from the database",
		);
		assert_eq!(
			cache
				.get_lrc(&tagged_file_data)
				.ok()
				.flatten(),
			Some(Lyrics::Synced(test_data::lyrics(2))),
			"must be able to get track metadata from the database",
		);
	}
}
