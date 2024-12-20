use tracing_subscriber::prelude::*;

use crate::remote::LyricsRequest;
use ignore::WalkState;
use lofty::{
    error::LoftyError,
    file::{TaggedFile, TaggedFileExt},
    probe::Probe,
    read_from_path,
    tag::Accessor,
};
use std::{ffi::OsStr, fmt::Debug};
use std::{io, path::Path};

#[derive(Debug)]
pub struct DirIterCfg<'a, 'b: 'a> {
    pub ignore_hidden: bool,
    pub ignore_non_music_ext: bool,
    pub strictness: FileMatchStrictness,
    pub plain_ignore_files: bool,
    pub follow_symlinks: bool,
    pub ignored_file_exts: &'a [&'b OsStr],
}

impl Default for DirIterCfg<'_, '_> {
    fn default() -> Self {
        Self {
            ignore_hidden: false,
            ignore_non_music_ext: true,
            strictness: Default::default(),
            follow_symlinks: true,
            plain_ignore_files: true,
            ignored_file_exts: [].as_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FileMatchStrictness {
    TrustyGuesser,
    FilterByExt,
    Paranoid,
}

impl Default for FileMatchStrictness {
    fn default() -> Self {
        Self::FilterByExt
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error(transparent)]
    Lofty(#[from] LoftyError),
    #[error(transparent)]
    Io(io::Error),
    #[error(
        "failed to prepare a request. artist is {:?}, title is {:?}",
        artist,
        title
    )]
    RequestPrepare {
        artist: Option<String>,
        title: Option<String>,
    },
    #[error(transparent)]
    Ignore(ignore::Error),
    // TODO (errors): add file match error
}

pub fn prepare_entries<P>(
    path: P,
    cfg: &DirIterCfg,
) -> crossbeam_channel::Receiver<Result<(LyricsRequest, ignore::DirEntry), PackError>>
where
    P: AsRef<Path> + Debug,
{
    let (tx, rx) = crossbeam_channel::unbounded();

    let walk = ignore::WalkBuilder::new(path)
        .ignore_case_insensitive(true) // TODO (config): put this into config, maybe?
        .ignore(true)
        .git_ignore(false) // TODO (config): is this even needed in context of music?
        .git_global(false)
        .git_exclude(false)
        .require_git(true)
        .follow_links(cfg.follow_symlinks)
        .hidden(cfg.ignore_hidden)
        .build_parallel();

    walk.run(move || {
        let tx = tx.clone();
        Box::new(move |entry| {
            if let Some(res) = entry
                .map_err(PackError::Ignore)
                .and_then(|entry| from_entry(entry, cfg.strictness)).transpose() {
                    tx.send(res).expect("this channel is unbounded, and, therefore, should always be available to send to");
                }

            WalkState::Continue
        })
    });

    rx
}

fn from_entry(
    entry: ignore::DirEntry,
    strictness: FileMatchStrictness,
) -> Result<Option<(LyricsRequest, ignore::DirEntry)>, PackError> {
    let path = entry.path();

    let ext_matches = path
        .extension()
        .and_then(|ext| {
            if !ext.eq_ignore_ascii_case("lrc") {
                Some(ext)
            } else {
                tracing::info!(?path, "given path is an lrc file already, skipping");
                None
            }
        })
        .map(|ext| {
            ext.eq_ignore_ascii_case("aac")
                || ext.eq_ignore_ascii_case("alac")
                || ext.eq_ignore_ascii_case("flac")
                || ext.eq_ignore_ascii_case("mp3")
                || ext.eq_ignore_ascii_case("ogg")
                || ext.eq_ignore_ascii_case("opus")
                || ext.eq_ignore_ascii_case("wav")
        })
        .unwrap_or(false);

    let tagged_file = match strictness {
        FileMatchStrictness::Paranoid | FileMatchStrictness::TrustyGuesser if !ext_matches => {
            Probe::open(path)
                .inspect_err(|e| tracing::warn!(?e))?
                .guess_file_type()
                .inspect_err(|e| tracing::warn!(?e))
                .map_err(PackError::Io)?
                .read()
                .inspect_err(|e| tracing::warn!(?e))?
        }
        FileMatchStrictness::Paranoid => return Ok(None),
        FileMatchStrictness::FilterByExt | FileMatchStrictness::TrustyGuesser => {
            read_from_path(path).inspect_err(|e| tracing::warn!(?e))?
        }
    };

    Ok(Some((prepare_lyrics_request(tagged_file)?, entry)))
}

fn prepare_lyrics_request(file: TaggedFile) -> Result<LyricsRequest, PackError> {
    // Have to do this, since TaggedFile is not Debug
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

    let (title, artist) = match (title, artist) {
        (Some(title), Some(artist)) => (title, artist),
        (title_opt, artist_opt) => {
            return Err(PackError::RequestPrepare {
                artist: artist_opt,
                title: title_opt,
            })
        }
    };

    Ok(LyricsRequest {
        title,
        artist,
        album,
        duration,
    })
}
