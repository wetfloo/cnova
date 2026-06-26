pub(crate) mod db;
pub(crate) mod fetcher;
pub(crate) mod service;

use std::borrow::Cow;
use std::time::Duration;

use lofty::file::AudioFile as _;
use lofty::file::TaggedFileExt as _;
use lofty::tag::Accessor as _;
use wetutil::prelude::*;

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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TaggedFileData<'a> {
	pub(crate) tag_data: TagData<'a>,
	pub(crate) duration: Duration,
}

impl TaggedFileData<'_> {
	#[inline]
	pub(crate) fn title(&self) -> Option<&str> {
		self.tag_data.title.as_deref()
	}

	#[inline]
	pub(crate) fn artist(&self) -> Option<&str> {
		self.tag_data.artist.as_deref()
	}

	#[inline]
	pub(crate) fn album(&self) -> Option<&str> {
		self.tag_data.album.as_deref()
	}

	#[inline]
	pub(crate) fn duration(&self) -> Duration {
		self.duration
	}
}

impl<'i, 'o, T> From<&'i lofty::file::BoundTaggedFile<T>> for TaggedFileData<'o>
where
	'i: 'o,
{
	fn from(value: &'i lofty::file::BoundTaggedFile<T>) -> Self {
		Self {
			tag_data: value
				.primary_tag()
				.val_into()
				.unwrap_or_default(),
			duration: value.properties().duration(),
		}
	}
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TagData<'a> {
	pub(crate) artist: Option<Cow<'a, str>>,
	pub(crate) album: Option<Cow<'a, str>>,
	pub(crate) title: Option<Cow<'a, str>>,
}

impl<'i, 'o> From<&'i lofty::tag::Tag> for TagData<'o>
where
	'i: 'o,
{
	#[inline]
	fn from(value: &'i lofty::tag::Tag) -> Self {
		Self {
			artist: value.artist(),
			album: value.album(),
			title: value.title(),
		}
	}
}
