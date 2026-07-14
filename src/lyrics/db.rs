//! Handle database cache interactions.

use std::rc::Rc;
use std::time::SystemTime;
use std::time::SystemTimeError;
use std::time::UNIX_EPOCH;

use rusqlite::OptionalExtension;
use strum::IntoDiscriminant;

use crate::lyrics;
use crate::lyrics::Lyrics;
use crate::lyrics::LyricsDiscriminants;
use crate::lyrics::TaggedFile;

/// An abstract cache over a database.
///
/// Using this type's [`Clone`]
/// implementation will clone the underlying [`Rc`].
///
/// This type is [`!Send`][Send], because the
/// underlying wrapped types are [`!Send`][Send].
#[derive(Clone, derive_more::Debug, derive_more::From)]
pub(crate) struct DbCache {
	#[debug(skip)]
	inner: Rc<rusqlite::Connection>,
}

impl DbCache {
	pub(crate) fn new(db_conn: rusqlite::Connection) -> Result<Self, rusqlite::Error> {
		db_conn.execute(queries::INIT_TABLE_LRC, ())?;

		Ok(Self {
			inner: db_conn.into(),
		})
	}

	pub(crate) fn get_lrc<T>(
		&self,
		tagged_file: &T,
	) -> Result<Option<lyrics::Lyrics>, DbCacheLyricsError>
	where
		T: TaggedFile,
	{
		let lyrics = self
			.inner
			.prepare_cached(queries::GET_LRC)?
			.query_row(
				&[
					(
						":artist",
						tagged_file
							.artist()
							.as_deref()
							.unwrap_or_default(),
					),
					(
						":album",
						tagged_file
							.album()
							.as_deref()
							.unwrap_or_default(),
					),
					(
						":title",
						tagged_file
							.title()
							.as_deref()
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

	pub(crate) fn insert_lrc<T>(
		&self,
		tagged_file: &T,
		lyrics: Lyrics,
	) -> Result<(), DbCacheLyricsError>
	where
		T: TaggedFile,
	{
		let mut stmt = self
			.inner
			.prepare_cached(queries::INSERT_LRC)?;
		stmt.execute(rusqlite::named_params! {
			":lyrics": lyrics.as_str().unwrap_or_default(),
			":artist": tagged_file
				.artist()
				.unwrap_or_default(),
			":album": tagged_file
				.album()
				.unwrap_or_default(),
			":title": tagged_file
				.title()
				.unwrap_or_default(),
			":duration_secs": tagged_file.duration().as_secs_f64(),
			":timestamp": SystemTime::now()
				.duration_since(UNIX_EPOCH)?
				.as_secs_f64(),
			":status": lyrics.discriminant() as i64,
		})?;

		Ok(())
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
	use std::borrow::Cow;
	use std::time::Duration;

	use crate::lyrics::Lyrics;
	use crate::lyrics::TaggedFile;
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

	struct TestTaggedFile {
		artist: String,
		album: String,
		title: String,
		duration: Duration,
	}

	impl TaggedFile for TestTaggedFile {
		fn artist(&self) -> Option<Cow<'_, str>> {
			Some(Cow::Borrowed(&self.title))
		}

		fn album(&self) -> Option<Cow<'_, str>> {
			Some(Cow::Borrowed(&self.album))
		}

		fn title(&self) -> Option<Cow<'_, str>> {
			Some(Cow::Borrowed(&self.title))
		}

		fn duration(&self) -> Duration {
			self.duration
		}
	}

	#[test]
	fn test_correct_debug_impl() {
		let cache = init_cache();

		assert_eq!(format!("{:?}", cache), "DbCache { .. }");
	}

	#[test]
	fn test_insert_and_get_twice() {
		let cache = init_cache();

		let tag_data = TestTaggedFile {
			artist: test_data::artist(1),
			album: test_data::album(1),
			title: test_data::title(1),
			duration: test_data::duration(1),
		};

		assert_matches!(
			cache.insert_lrc(
				&tag_data,
				Lyrics::Synced(test_data::lyrics(1)),
			),
			Ok(()),
			"must be able to insert values into the database",
		);
		assert_eq!(
			cache.get_lrc(&tag_data).ok().flatten(),
			Some(Lyrics::Synced(test_data::lyrics(1))),
			"must be able to get track metadata from the database",
		);
		assert_eq!(
			cache.get_lrc(&tag_data).ok().flatten(),
			Some(Lyrics::Synced(test_data::lyrics(1))),
			"must be able to get the same track metadata from the database repeatedly",
		);
	}

	#[test]
	fn test_insert_two_and_get() {
		let cache = init_cache();

		let tag_data_1 = TestTaggedFile {
			artist: test_data::artist(1),
			album: test_data::album(1),
			title: test_data::title(1),
			duration: test_data::duration(1),
		};
		let tag_data_2 = TestTaggedFile {
			artist: test_data::artist(2),
			album: test_data::album(2),
			title: test_data::title(2),
			duration: test_data::duration(2),
		};

		assert_matches!(
			cache.insert_lrc(
				&tag_data_1,
				Lyrics::Synced(test_data::lyrics(1)),
			),
			Ok(()),
			"must be able to insert values into the database",
		);
		assert_matches!(
			cache.insert_lrc(
				&tag_data_2,
				Lyrics::Synced(test_data::lyrics(2)),
			),
			Ok(()),
			"must be able to insert values into the database",
		);
		assert_eq!(
			cache
				.get_lrc(&tag_data_1)
				.ok()
				.flatten(),
			Some(Lyrics::Synced(test_data::lyrics(1))),
			"must be able to get track metadata from the database",
		);
		assert_eq!(
			cache
				.get_lrc(&tag_data_2)
				.ok()
				.flatten(),
			Some(Lyrics::Synced(test_data::lyrics(2))),
			"must be able to get track metadata from the database",
		);
	}
}
