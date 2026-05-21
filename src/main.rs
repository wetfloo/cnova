use std::sync::Mutex;
use std::{env::home_dir, sync::LazyLock};

use sqlite::ffi::sqlite3_stmt_status;
use walkdir::WalkDir;

static LYRICS_DB: LazyLock<sqlite::ConnectionThreadSafe> =
	LazyLock::new(|| sqlite::Connection::open_thread_safe(":memory:").unwrap());

fn main() {}

fn traverse() {
	let mut path = home_dir().unwrap();
	path.push("Music/Experiment");

	for item in WalkDir::new(&path)
		.into_iter()
		.filter_map(|res| res.ok())
	{
		println!("{:?}", item);
	}
}
