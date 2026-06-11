use std::path::Path;

use walkdir::WalkDir;
use wetutil::prelude::*;

use super::UnboundedTx;

/// Traverse `paths` recursively,
/// sending any file (not a directory!) to `tx`.
pub fn traverse_v2<TX, I, P>(tx: &TX, paths: I)
where
	TX: UnboundedTx<Item = walkdir::DirEntry>,
	I: IntoIterator<Item = P>,
	P: AsRef<Path>,
{
	for path in paths {
		for entry_path in WalkDir::new(&path)
			.into_iter()
			.consume_err(|err| {
				// TODO::logging
				dbg!(err);
			})
			.filter(|dir_entry| dir_entry.file_type().is_file())
		{
			// TODO::error_handling: remove unwrap,
			// (replace with `expect` that the channel will never be closed?)
			tx.send(entry_path).unwrap();
		}
	}
}
