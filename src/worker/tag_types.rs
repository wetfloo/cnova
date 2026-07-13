use lofty::file::TaggedFileExt as _;

pub(super) struct TagTypesToWrite {
	tag_type: lofty::tag::TagType,
	/// Whether we have shown the primary tag type before.
	///
	/// For most types of tags,
	/// this will be `true` after [`tag_type`][Self::tag_type]
	/// is returned via this struct's [`Iterator`] implementation.
	/// For some tag types, like [`Id3v2`](lofty::tag::TagType::Id3v2),
	/// we also want to write a fallback tag (like [`Id3v1`](lofty::tag::TagType::Id3v1)).
	primary_shown: bool,
}

impl TagTypesToWrite {
	pub(super) fn new(tag_type: lofty::tag::TagType) -> Self {
		Self {
			tag_type,
			primary_shown: false,
		}
	}
}

impl Iterator for TagTypesToWrite {
	type Item = lofty::tag::TagType;

	fn next(&mut self) -> Option<Self::Item> {
		match (self.primary_shown, self.tag_type) {
			(false, tag_type) => {
				self.primary_shown = true;
				Some(tag_type)
			},
			(true, lofty::tag::TagType::Id3v2) => Some(lofty::tag::TagType::Id3v1),
			(true, _) => None,
		}
	}
}

pub(super) trait TagTypesToWriteExt {
	type Iter: Iterator<Item = lofty::tag::TagType>;

	fn tag_types_to_write(&self) -> Self::Iter;
}

impl TagTypesToWriteExt for lofty::file::TaggedFile {
	type Iter = TagTypesToWrite;

	fn tag_types_to_write(&self) -> Self::Iter {
		TagTypesToWrite::new(self.primary_tag_type())
	}
}

impl<T> TagTypesToWriteExt for lofty::file::BoundTaggedFile<T> {
	type Iter = TagTypesToWrite;

	fn tag_types_to_write(&self) -> Self::Iter {
		TagTypesToWrite::new(self.primary_tag_type())
	}
}
