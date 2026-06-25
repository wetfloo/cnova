use crate::lyrics::Lyrics;
use crate::lyrics::TaggedFileData;

pub(crate) type DbConnection = sqlite::Connection;

pub(crate) struct DbCache(DbCacheInner);

#[ouroboros::self_referencing]
struct DbCacheInner {
	db_conn: DbConnection,
	#[borrows(db_conn)]
	#[covariant]
	statement: sqlite::Statement<'this>,
}

impl DbCache {
	pub(crate) fn new(db_conn: DbConnection) -> Self {
		Self(
			DbCacheInnerBuilder {
				db_conn,
				statement_builder: |db_conn: &DbConnection| {
					db_conn
						.prepare(
							// TODO: make a better statement
							"SELECT lyrics \
							FROM lrc \
							WHERE artist = :artist \
								AND title = :title \
							LIMIT 1;",
						)
						// TODO::error_handling remove unwrap
						.unwrap()
				},
			}
			.build(),
		)
	}

	pub(crate) fn get_lrc(
		&mut self,
		tagged_file_data: &TaggedFileData,
	) -> Result<String, sqlite::Error> {
		self.0.with_statement_mut(|statement| {
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

impl From<DbConnection> for DbCache {
	#[inline]
	fn from(value: DbConnection) -> Self {
		Self::new(value)
	}
}
