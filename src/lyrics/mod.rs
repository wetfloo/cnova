pub(crate) mod db;
pub(crate) mod fetcher;
pub(crate) mod service;

use std::borrow::Cow;
use std::fmt;
use std::time::Duration;

use lofty::file::AudioFile;
use lofty::file::TaggedFileExt;
use lofty::tag::Accessor;

#[derive(Clone, Debug, PartialEq, strum::EnumDiscriminants)]
#[strum_discriminants(repr(i64))]
#[strum_discriminants(derive(strum::FromRepr))]
pub(crate) enum Lyrics {
	Synced(String),
	Unsynced(String),
	Instrumental,
}

impl Lyrics {
	pub(crate) fn as_str(&self) -> Option<&str> {
		match self {
			Lyrics::Synced(v) | Lyrics::Unsynced(v) => Some(v),
			Lyrics::Instrumental => None,
		}
	}
}

pub(crate) trait TaggedFile {
	fn artist(&self) -> Option<Cow<'_, str>>;
	fn album(&self) -> Option<Cow<'_, str>>;
	fn title(&self) -> Option<Cow<'_, str>>;
	fn duration(&self) -> Duration;

	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			r#"track "{}" by "{}" in album "{}""#,
			self.title().as_deref().unwrap_or("?"),
			self.artist().as_deref().unwrap_or("?"),
			self.album().as_deref().unwrap_or("?"),
		)
	}
}

impl<F> TaggedFile for lofty::file::BoundTaggedFile<F> {
	fn artist(&self) -> Option<Cow<'_, str>> {
		self.primary_tag()
			.and_then(|tag| tag.artist())
	}

	fn album(&self) -> Option<Cow<'_, str>> {
		self.primary_tag()
			.and_then(|tag| tag.album())
	}

	fn title(&self) -> Option<Cow<'_, str>> {
		self.primary_tag()
			.and_then(|tag| tag.title())
	}

	fn duration(&self) -> Duration {
		self.properties().duration()
	}
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TaggedFileWrapper<T>(T);

impl<T> TaggedFileWrapper<T> {
	#[inline]
	pub(crate) fn new(value: T) -> Self {
		value.into()
	}

	#[inline]
	pub(crate) fn into_inner(self) -> T {
		self.0
	}

	#[inline]
	pub(crate) fn inner(&self) -> &T {
		&self.0
	}

	#[inline]
	pub(crate) fn inner_mut(&mut self) -> &mut T {
		&mut self.0
	}
}

impl<T> From<T> for TaggedFileWrapper<T> {
	fn from(value: T) -> Self {
		Self(value)
	}
}

impl<T> fmt::Display for TaggedFileWrapper<T>
where
	T: TaggedFile,
{
	#[inline]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		TaggedFile::fmt(self, f)
	}
}

impl<T> TaggedFile for TaggedFileWrapper<T>
where
	T: TaggedFile,
{
	#[inline]
	fn artist(&self) -> Option<Cow<'_, str>> {
		self.0.artist()
	}

	#[inline]
	fn album(&self) -> Option<Cow<'_, str>> {
		self.0.album()
	}

	#[inline]
	fn title(&self) -> Option<Cow<'_, str>> {
		self.0.title()
	}

	#[inline]
	fn duration(&self) -> Duration {
		self.0.duration()
	}
}
