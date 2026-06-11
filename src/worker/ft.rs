use std::path::Path;

use lofty::file::TaggedFileExt as _;
use wetutil::prelude::*;

use super::GuessFileError;

pub(super) fn handle_file_guessing<P>(path: P) -> Result<lofty::file::TaggedFile, GuessFileError>
where
	P: AsRef<Path>,
{
	// TODO::config: add a way to make lofty guess (or not) track's filetype.
	lofty::probe::Probe::open(path)?
		.guess_file_type()?
		.read()
		.err_into()
		.and_then(|tagged_file| {
			match tagged_file.file_type() {
				// Do not support custom file types, since we wouldn't be able to write
				// their tags anyway. Also, it gets rid of "non-music" file problem
				// (.jpg, .png, .lrc, etc.).
				lofty::file::FileType::Custom(ft) => Err(GuessFileError::InvalidFileType(ft)),
				_ => Ok(tagged_file),
			}
		})
}
