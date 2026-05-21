// TODO: remove when we're done.
#![allow(unused)]

use std::sync::Mutex;
use std::time::Duration;
use std::{env::home_dir, sync::LazyLock};

use sqlite::ffi::sqlite3_stmt_status;
use walkdir::WalkDir;

trait LyricsHolder {
	fn lyrics(metadata: &Metadata) -> Option<Lyrics>;
}

trait LyricsResolver {
	async fn resolve_lyrics(metadata: &Metadata) -> Result<Lyrics, LyricsResolveError>;
}

struct Metadata {
	title: Option<String>,
	artist: Option<String>,
	album_artist: Option<String>,
	album: Option<String>,
	duration: Duration,
}

struct Lyrics {
	kind: LyricsKind,
	data: String,
}

enum LyricsKind {
	Synced,
	Unsynced,
}

struct LyricsResolveError;

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
