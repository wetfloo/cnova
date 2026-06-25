use crate::lyrics::TaggedFileData;

pub(crate) type DbConnection = sqlite::Connection;

pub(crate) struct DbCache(DbCacheInner);

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
	) -> Result<String, sqlite::Error> {
		self.0
			.with_get_lyrics_statement_mut(|statement| {
				statement.bind(
					&[
						(
							":artist",
							tagged_file_data
								.artist()
								.unwrap_or_default(),
						),
						(
							":title",
							tagged_file_data
								.title()
								.unwrap_or_default(),
						),
					][..],
				)?;

				statement.read(0)
			})
	}
}

impl TryFrom<DbConnection> for DbCache {
	type Error = sqlite::Error;

	#[inline]
	fn try_from(value: DbConnection) -> Result<Self, Self::Error> {
		Self::new(value)
	}
}

pub(super) mod queries {
	const INIT_TABLE_LRC: &str = "\
	CREATE TABLE IF NOT EXISTS lrc (\
		lyrics TEXT NOT NULL, \
		artist TEXT NOT NULL, \
		title TEXT NOT NULL, \
		duration_secs REAL NOT NULL, \
		timestamp INTEGER NOT NULL, \
		status INTEGER NOT NULL\
	) STRICT;\
	";

	macro_rules! transact {
		($($arg: expr),+ $(,)?) => (
			{
				const_format::concatcp!(
					"BEGIN TRANSACTION;",
					' ',
					INIT_TABLE_LRC,
					' ',
					$( ( $arg ), ' ', )*
					"COMMIT;",
				)
			}
		)
	}

	pub(super) const GET_LRC: &str = transact!(
		"\
		SELECT lyrics \
		FROM lrc \
		WHERE artist = :artist \
			AND title = :title \
		LIMIT 1;\
		",
	);

	pub(super) const INSERT_LRC: &str = transact!(
		"\
		INSERT INTO lrc VALUES (\
			:lyrics, \
			:artist, \
			:title, \
			:duration_secs, \
			:timestamp, \
			:status\
		);\
		"
	);
}

#[cfg(test)]
mod test {
	use crate::lyrics::db::DbCache;
	use crate::lyrics::db::queries;

	fn init_cache() -> DbCache {
		sqlite::Connection::open(":memory:")
			.and_then(DbCache::new)
			.unwrap()
	}

	#[test]
	fn test_init_empty() {
		let cache = init_cache();
	}
}
