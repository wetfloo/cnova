use std::fmt;
use std::time::SystemTime;
use std::time::SystemTimeError;
use std::time::UNIX_EPOCH;

use pretty_type_name::pretty_type_name;
use rusqlite::OptionalExtension;
use strum::IntoDiscriminant;

use crate::lyrics;
use crate::lyrics::Lyrics;
use crate::lyrics::LyricsDiscriminants;
use crate::lyrics::TaggedFileData;

pub(crate) type DbConnection = rusqlite::Connection;

pub(crate) struct DbCache(DbConnection);

impl fmt::Debug for DbCache {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&pretty_type_name::<Self>())
	}
}

impl DbCache {
	pub(crate) fn new(db_conn: DbConnection) -> Result<Self, rusqlite::Error> {
		db_conn.execute(queries::INIT_TABLE_LRC, ())?;

		Ok(Self(db_conn))
	}

	pub(crate) fn get_lrc(
		&self,
		tagged_file_data: &TaggedFileData,
	) -> Result<Option<lyrics::Lyrics>, DbCacheLyricsError> {
		let lyrics = self
			.0
			.prepare_cached(queries::GET_LRC)?
			.query_row(
				&[
					(
						":artist",
						tagged_file_data
							.artist()
							.unwrap_or_default(),
					),
					(
						":album",
						tagged_file_data
							.album()
							.unwrap_or_default(),
					),
					(
						":title",
						tagged_file_data
							.title()
							.unwrap_or_default(),
					),
				],
				|row| Ok((row.get("lyrics")?, row.get("status")?)),
			)
			.optional()?;

		lyrics
			.map(|(lyrics, status)| {
				LyricsDiscriminants::from_repr(status)
					.map(|discriminant| match discriminant {
						LyricsDiscriminants::Synced => Lyrics::Synced(lyrics),
						LyricsDiscriminants::Unsynced => Lyrics::Unsynced(lyrics),
						LyricsDiscriminants::Instrumental => Lyrics::Instrumental,
					})
					.ok_or(DbCacheLyricsError::StatusMismatch(
						status,
					))
			})
			.transpose()
	}

	pub(crate) fn insert_lrc(
		&self,
		tagged_file_data: &TaggedFileData,
		lyrics: Lyrics,
	) -> Result<(), DbCacheLyricsError> {
		let mut stmt = self
			.0
			.prepare_cached(queries::INSERT_LRC)?;
		stmt.execute(rusqlite::named_params! {
			":lyrics": lyrics.as_str().unwrap_or_default(),
			":artist": tagged_file_data
				.artist()
				.unwrap_or_default(),
			":album": tagged_file_data
				.album()
				.unwrap_or_default(),
			":title": tagged_file_data
				.title()
				.unwrap_or_default(),
			":duration_secs": tagged_file_data.duration.as_secs_f64(),
			":timestamp": SystemTime::now()
				.duration_since(UNIX_EPOCH)?
				.as_secs_f64(),
			":status": lyrics.discriminant() as i64,
		})?;

		Ok(())
	}
}

impl TryFrom<DbConnection> for DbCache {
	type Error = rusqlite::Error;

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
	Sqlite(#[from] rusqlite::Error),
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

	use crate::lyrics::Lyrics;
	use crate::lyrics::TagData;
	use crate::lyrics::TaggedFileData;
	use crate::lyrics::db::DbCache;

	fn init_cache() -> DbCache {
		let res = rusqlite::Connection::open(":memory:").and_then(DbCache::new);

		assert_matches!(res, Ok(_));

		res.expect("we've just verified that this value is Ok")
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
		let mut cache = init_cache();

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
		let mut cache = init_cache();

		let tag_data = TagData {
			artist: Some(test_data::artist(1).into()),
			album: Some(test_data::album(1).into()),
			title: Some(test_data::title(1).into()),
		};
		let tagged_file_data_1 = TaggedFileData {
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
				&tagged_file_data_1,
				Lyrics::Synced(test_data::lyrics(1)),
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
				.get_lrc(&tagged_file_data_1)
				.ok()
				.flatten(),
			Some(Lyrics::Synced(test_data::lyrics(1))),
			"must be able to get track metadata from the database",
		);
		assert_eq!(
			cache
				.get_lrc(&tagged_file_data_2)
				.ok()
				.flatten(),
			Some(Lyrics::Synced(test_data::lyrics(2))),
			"must be able to get track metadata from the database",
		);
	}
}
