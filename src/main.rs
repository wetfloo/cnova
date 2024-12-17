use lofty::{file::TaggedFileExt, probe::Probe, read_from_path, tag::Accessor};
use std::{env, process};
use util::todo_err;
use walkdir::{DirEntry, WalkDir};

mod remote;
mod util;

struct DirIterCfg {
    skip_hidden: bool,
    skip_non_music_ext: bool,
    laxed_ext_mode: bool,
}

impl Default for DirIterCfg {
    fn default() -> Self {
        Self {
            skip_hidden: false,
            skip_non_music_ext: true,
            laxed_ext_mode: false,
        }
    }
}

fn main() {
    let dir_iter_cfg = DirIterCfg::default();

    let file_path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("This program needs a path to scan");
        process::exit(1);
    });

    let iter = WalkDir::new(file_path).into_iter();
    for entry in iter
        .filter_entry(|entry| !dir_iter_cfg.skip_hidden || entry.is_hidden())
        .filter_map(|entry_res| entry_res.map_err(|e| todo_err!(e)).ok())
    {
        if entry.is_suitable_file(&dir_iter_cfg) {
            dbg!(&entry);
            let tagged_file = if dir_iter_cfg.laxed_ext_mode {
                read_from_path(entry.path()).map_err(|e| todo_err!(e))
            } else {
                Probe::open(entry.path())
                    .map_err(|e| todo_err!(e))
                    .and_then(|probe| probe.guess_file_type().map_err(|e| todo_err!(e)))
                    .and_then(|probe| probe.read().map_err(|e| todo_err!(e)))
            }
            .ok();

            match tagged_file {
                Some(tagged_file) => {
                    for tag in tagged_file.tags() {
                        dbg!(tag.title(), tag.artist());
                    }
                }
                None => eprintln!("TODO: failed to read file"),
            }
        }
    }
}

trait DirEntryExt {
    fn is_hidden(&self) -> bool;

    fn is_suitable_file(&self, cfg: &DirIterCfg) -> bool;
}

impl DirEntryExt for DirEntry {
    fn is_hidden(&self) -> bool {
        self.file_name()
            .to_str()
            .map(|s| s.starts_with("."))
            .unwrap_or(false)
    }

    fn is_suitable_file(&self, cfg: &DirIterCfg) -> bool {
        if !self.file_type().is_file() {
            return false;
        }

        if !cfg.skip_non_music_ext {
            return true;
        }

        match self
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_ascii_lowercase())
        {
            Some(extension) => {
                matches!(
                    extension.as_str(),
                    "mp3" | "mp4" | "aac" | "alac" | "flac" | "opus" | "ogg" | "wav"
                )
            }
            None => false,
        }
    }
}
