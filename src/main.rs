// TODO: remove when we're done.
#![allow(unused)]

mod iter;

use itertools::Itertools;

use std::iter::Inspect;
use std::sync::Mutex;
use std::time::Duration;
use std::{env::home_dir, sync::LazyLock};

use sqlite::ffi::sqlite3_stmt_status;
use walkdir::WalkDir;

struct ProcessError<N> {
	inner_iter: N,
}

impl<N> ProcessError<N>
{
	fn new<I, F>(iter: I, f: F) -> ProcessError<impl Iterator<Item = I::Item>>
	where
		I : Iterator,
		F: FnMut(&I::Item),
	{
		ProcessError {
			inner_iter: iter.inspect(f),
		}
	}
}

impl<N> Iterator for ProcessError<N>
where
	N: Iterator,
{
	type Item = N::Item;

	fn next(&mut self) -> Option<Self::Item> {
		self.inner_iter.next()
	}
}

trait IterExt: Iterator {
	fn process_err<T, E, F>(self, processor: F) -> ProcessError<impl Iterator<Item = Self::Item>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E);

	fn discard_err<T, E>(self) -> impl Iterator<Item = T>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		self.filter_map(|res| res.ok())
	}
}

impl<I> IterExt for I
where
	I: Iterator,
{
	fn process_err<T, E, F>(
		self,
		mut processor: F,
	) -> ProcessError<impl Iterator<Item = Self::Item>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E),
	{
		ProcessError {
			inner_iter: self.inspect(move |res| {
				if let Err(err) = res {
					processor(err)
				}
			}),
		}
	}
}

impl<I> DoubleEndedIterator for ProcessError<I>
where
	I: DoubleEndedIterator,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		self.inner_iter.next_back()
	}
}

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
