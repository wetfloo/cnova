use crate::{
    cli::{Cli, FileMatchStrictness},
    remote::LyricsRequest,
    util::TraceLog,
};
use ignore::WalkState;
use lofty::{
    error::LoftyError,
    file::{TaggedFile, TaggedFileExt},
    probe::Probe,
    read_from_path,
    tag::Accessor,
};
use std::{fmt::Debug, path::PathBuf};
use std::{io, path::Path};

impl Default for FileMatchStrictness {
    fn default() -> Self {
        Self::FilterByExt
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("underlying tagging error")]
    Lofty {
        path: PathBuf,
        #[source]
        src: LoftyError,
    },
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

impl TraceLog for PackError {
    fn trace_log(&self) {
        tracing::warn!(?self);
    }
}

pub type EntriesRx =
    crossbeam_channel::Receiver<Result<(LyricsRequest, ignore::DirEntry), PackError>>;

#[derive(Debug, thiserror::Error)]
#[error("no paths were provided")]
pub struct NoPathsError;

pub fn prepare_entries(cli: &Cli) -> Result<EntriesRx, NoPathsError> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut iter = cli.paths.iter();

    let mut builder = ignore::WalkBuilder::new(iter.next().ok_or(NoPathsError)?);

    for path in iter {
        builder.add(path);
    }

    let walk = builder
        .ignore_case_insensitive(true) // TODO (config): put this into config, maybe?
        .ignore(true)
        .git_ignore(false) // TODO (config): is this even needed in context of music?
        .git_global(false)
        .git_exclude(false)
        .require_git(true)
        .follow_links(cli.follow_symlinks)
        .hidden(cli.ignore_hidden)
        .build_parallel();

    walk.run(move || {
        let tx = tx.clone();
        Box::new(move |entry| {
            if let Some(res) = entry
                .map_err(PackError::Ignore)
                .and_then(|entry| from_entry(entry, cli)).transpose() {
                    tx.send(res).expect("this channel is unbounded, and, therefore, should always be available to send to");
                }

            WalkState::Continue
        })
    });

    Ok(rx)
}

#[tracing::instrument(level = "trace")]
fn from_entry(
    entry: ignore::DirEntry,
    cli: &Cli,
) -> Result<Option<(LyricsRequest, ignore::DirEntry)>, PackError> {
    let path = entry.path();

    if !path.is_file() {
        tracing::debug!(?path, "entry is not a file");
        return Ok(None);
    }

    if !cli.overwrite_lrc_files {
        let mut path = path.to_owned();
        if path.set_extension("lrc") && path.exists() {
            tracing::info!(
                ?path,
                "not overwriting an existing lrc file for a corresponding path",
            );
            return Ok(None);
        }
    }

    let ext_matches = path
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
        .unwrap_or(false);

    let tagged_file = match cli.strictness {
        FileMatchStrictness::Paranoid | FileMatchStrictness::FilterByExt if !ext_matches => {
            tracing::debug!(?path, ?cli.strictness, "entry didn't match");
            return Ok(None);
        }

        FileMatchStrictness::FilterByExt | FileMatchStrictness::TrustyGuesser => {
            tracing::debug!(?path, ?cli.strictness, ?ext_matches, "probing by extension");
            shallow_inspect(path)?
        }

        FileMatchStrictness::Paranoid => {
            tracing::debug!(?path, ?cli.strictness, ?ext_matches, "deep probing");
            deep_inspect(path)?
        }
    };

    Ok(Some((prepare_lyrics_request(tagged_file)?, entry)))
}

#[tracing::instrument]
fn deep_inspect(path: &Path) -> Result<TaggedFile, PackError> {
    Probe::open(path)
        .inspect_err(|e| tracing::warn!(?e))
        .map_err(|e| PackError::Lofty {
            path: path.to_owned(),
            src: e,
        })?
        .guess_file_type()
        .inspect_err(|e| tracing::warn!(?e))
        .map_err(PackError::Io)?
        .read()
        .inspect_err(|e| tracing::warn!(?e))
        .map_err(|e| PackError::Lofty {
            path: path.to_owned(),
            src: e,
        })
}

#[tracing::instrument]
fn shallow_inspect(path: &Path) -> Result<TaggedFile, PackError> {
    read_from_path(path)
        .inspect_err(|e| tracing::warn!(?e))
        .map_err(|e| PackError::Lofty {
            path: path.to_owned(),
            src: e,
        })
}

#[tracing::instrument(level = "trace", skip(file))]
fn prepare_lyrics_request(file: TaggedFile) -> Result<LyricsRequest, PackError> {
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
