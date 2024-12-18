use crate::remote::LyricsRequest;
use lofty::{
    file::{TaggedFile, TaggedFileExt},
    probe::Probe,
    read_from_path,
    tag::Accessor,
};
use rayon::{iter::IntoParallelRefIterator, prelude::*};
use std::fmt::Debug;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug)]
pub struct DirIterCfg {
    pub skip_hidden: bool,
    pub skip_non_music_ext: bool,
    pub laxed_ext_mode: bool,
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

#[tracing::instrument(level = "trace")]
pub fn list_files<P>(path: P, cfg: &DirIterCfg) -> Vec<DirEntry>
where
    P: AsRef<Path> + Debug,
{
    WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| !cfg.skip_hidden || entry.is_hidden())
        .filter_map(|entry_res| entry_res.inspect_err(|e| tracing::warn!(?e)).ok())
        .filter(|entry| entry.is_suitable_file(cfg))
        .collect()
}

pub fn all_file_requests(entries: &[DirEntry], cfg: &DirIterCfg) -> Vec<LyricsRequest> {
    entries
        .par_iter()
        .filter_map(|entry| {
            let _span = tracing::span!(tracing::Level::TRACE, "filter_ok_files", ?entries, ?cfg);

            if cfg.laxed_ext_mode {
                read_from_path(entry.path())
                    .inspect_err(|e| tracing::warn!(?e))
                    .ok()
            } else {
                Probe::open(entry.path())
                    .inspect_err(|e| tracing::warn!(?e))
                    .ok()
                    .and_then(|probe| {
                        probe
                            .guess_file_type()
                            .inspect_err(|e| tracing::warn!(?e))
                            .ok()
                    })
                    .and_then(|probe| probe.read().inspect_err(|e| tracing::warn!(?e)).ok())
            }
        })
        .filter_map(prepare_lyrics_request)
        .collect() // TODO: remove this and send requests
}

fn prepare_lyrics_request(file: TaggedFile) -> Option<LyricsRequest> {
    let tags_set = file.tags().first()?;
    let request = LyricsRequest {
        artist: tags_set.artist()?.into_owned(),
        title: tags_set.title()?.into_owned(),
        album: tags_set.album().map(|cow| cow.into_owned()),
        duration: None, // TODO
    };
    Some(request)
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
        // TODO: this does not follow symlinks, fix it
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
