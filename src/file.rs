use crate::remote::LyricsRequest;
use lofty::{
    file::{TaggedFile, TaggedFileExt},
    probe::Probe,
    read_from_path,
    tag::Accessor,
};
use rayon::prelude::*;
use std::fmt::Debug;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug)]
pub struct DirIterCfg {
    pub ignore_hidden: bool,
    pub ignore_non_music_ext: bool,
    pub strict_mode: bool,
}

impl Default for DirIterCfg {
    fn default() -> Self {
        Self {
            ignore_hidden: false,
            ignore_non_music_ext: true,
            strict_mode: false,
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
        .filter_entry(|entry| !cfg.ignore_hidden || entry.is_hidden())
        .filter_map(|entry_res| entry_res.inspect_err(|e| tracing::warn!(?e)).ok())
        .filter(|entry| entry.is_suitable_file(cfg))
        .collect()
}

pub fn prepare_entries<I>(entries: I, cfg: &DirIterCfg) -> Vec<(LyricsRequest, DirEntry)>
where
    I: Debug + IntoParallelIterator<Item = DirEntry>,
{
    entries
        .into_par_iter()
        .filter_map(|entry| {
            let _span = tracing::span!(tracing::Level::TRACE, "filter_ok_files", ?entry, ?cfg);
            let path = entry.path();

            let tagged_file = if path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("lrc"))
                .unwrap_or(false)
            {
                tracing::info!(
                    ?entry,
                    "found an entry with existing lrc extension, skipping"
                );
                None
            } else if !cfg.strict_mode {
                read_from_path(path)
                    .inspect_err(|e| tracing::warn!(?e))
                    .ok()
            } else {
                Probe::open(path)
                    .inspect_err(|e| tracing::warn!(?e))
                    .ok()
                    .and_then(|probe| {
                        probe
                            .guess_file_type()
                            .inspect_err(|e| tracing::warn!(?e))
                            .ok()
                    })
                    .and_then(|probe| probe.read().inspect_err(|e| tracing::warn!(?e)).ok())
            };
            tagged_file
                .and_then(prepare_lyrics_request)
                .map(|request| (request, entry))
        })
        .collect() // TODO: remove this and send requests
}

fn prepare_lyrics_request(file: TaggedFile) -> Option<LyricsRequest> {
    let _span = tracing::span!(tracing::Level::TRACE, "prepare_lyrics_request");

    let tags_slice = file.tags();

    let artist = tags_slice
        .iter()
        .find_map(|tags| tags.artist())
        .map(|cow| cow.into_owned());
    let title = tags_slice
        .iter()
        .find_map(|tags| tags.title())
        .map(|cow| cow.into_owned());
    let album = tags_slice
        .iter()
        .find_map(|tags| tags.album())
        .map(|cow| cow.into_owned());
    let duration = None; // TODO

    if title.is_none() {
        tracing::warn!("title couldn't be read");
    }
    if artist.is_none() {
        tracing::warn!("artist couldn't be read");
    }
    if album.is_none() {
        tracing::warn!("album couldn't be read");
    }
    // TODO
    //if duration.is_none() {
    //    tracing::warn!("duration couldn't be read");
    //}

    Some(LyricsRequest {
        title: title?,
        artist: artist?,
        album,
        duration,
    })
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

        if !cfg.ignore_non_music_ext {
            return true;
        }

        self.path()
            .extension()
            .map(|ext| {
                ext.eq_ignore_ascii_case("aac")
                    || ext.eq_ignore_ascii_case("alac")
                    || ext.eq_ignore_ascii_case("flac")
                    || ext.eq_ignore_ascii_case("mp3")
                    || ext.eq_ignore_ascii_case("ogg")
                    || ext.eq_ignore_ascii_case("opus")
                    || ext.eq_ignore_ascii_case("wav")
            })
            .unwrap_or(false)
    }
}
