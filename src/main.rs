// TODO: remove when we're done.
#![allow(unused)]

use itertools::Itertools;

use std::sync::Mutex;
use std::time::Duration;
use std::{env::home_dir, sync::LazyLock};

use sqlite::ffi::sqlite3_stmt_status;
use walkdir::WalkDir;

trait IterExt {
	fn process_err<T, E, F>(self, processor: F) -> impl Iterator<Item = Self::Item>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: Fn(&E),
	{
		self.inspect(move |res| match res {
			Ok(_) => (),
			Err(err) => processor(err),
		})
	}

	fn discard_err<T, E>(self) -> impl Iterator<Item = T>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		self.filter_map(|res| res.ok())
	}
}

impl<T> IterExt for T where T: Iterator {}

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

	let res: Vec<_> = WalkDir::new(&path)
		.into_iter()
		.inspect(|res| {
			dbg!(res);
		})
		.collect();
}
