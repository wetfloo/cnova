// TODO: remove when we're done.
#![allow(unused)]

use cnova::IterExt;

use std::iter::Inspect;
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

fn main() {
	traverse();
}

fn traverse() {
	let mut path = home_dir().unwrap();
	path.push("Music/Experiment");

	let _: Vec<_> = WalkDir::new(&path)
		.into_iter()
		.inspect_err(|err| {
			dbg!(err);
		})
		.inspect_ok(|val| {
			dbg!(val);
		})
		.discard_err()
		.collect();
}
