mod file;
#[cfg(test)]
mod test;

use crate::cli::Cli;
use crate::remote::{LyricsError, LyricsRequest, LyricsResponse, Remote};
use file::PackResult;
use file::PacksRx;
use reqwest::StatusCode;
use std::{
    future::Future,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::task::JoinSet;

const JOIN_HANDLE_EXPECT_MSG: &str =
    "seems like child job panicked. we shouldn't ever panic like that!";

/// Starts up the whole process of going through tracks
/// and creating corresponding `.lrc` and `.nolrc` files, taking `cli`
/// configuration into account
///
/// To understand, why `remote` has to have all these type constraints,
/// consult [`tokio::runtime::Runtime::spawn`]
/// and [`tokio::task::JoinSet::spawn`] documentation
pub async fn start_up<R>(remote: &'static R, cli: Cli)
where
    R: Remote + Sync,
{
    let deny_nolrc = cli.deny_nolrc;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PackResult>();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cli.download_jobs.into()));
    let handle = tokio::spawn(async move {
        handle_all(remote, semaphore, &mut rx, deny_nolrc).await;
    });

    tokio::task::spawn_blocking(move || {
        file::prepare_entries(&tx, &cli)
            .expect("the amount of paths provided has to be verified at the cli level");
    })
    .await
    .expect(JOIN_HANDLE_EXPECT_MSG);

    handle.await.expect(JOIN_HANDLE_EXPECT_MSG);
}

/// Handles all the given packs of data from `rx`. Will not create `.nolrc` files
/// if `deny_nolrc` is `true`. Doesn't spawn any more jobs requesting lyrics from
/// `remote` than `semaphore` has permits at one time
#[tracing::instrument(level = "trace", skip_all)]
async fn handle_all<R>(
    remote: &'static R,
    semaphore: Arc<tokio::sync::Semaphore>,
    rx: &mut PacksRx,
    deny_nolrc: bool,
) where
    R: Remote + Sync,
{
    let mut join_set = JoinSet::new();

    while let Some(res) = rx.recv().await {
        if let Ok((request, path)) = res.inspect_err(|e| tracing::warn!(%e)) {
            tracing::trace!(?request, ?path, "received new value");

            let permit = semaphore.clone().acquire_owned();

            join_set.spawn(handle_entry(permit, remote, request, path, deny_nolrc));
        }
    }

    join_set.join_all().await;
}

#[tracing::instrument(level = "trace", skip_all)]
async fn handle_entry<A, P, R>(
    permit: P,
    remote: &R,
    request: LyricsRequest,
    path: A,
    deny_nolrc: bool,
) where
    A: AsRef<Path>,
    P: Future<Output = Result<tokio::sync::OwnedSemaphorePermit, tokio::sync::AcquireError>>,
    R: Remote,
{
    let path = path.as_ref();
    let permit = permit.await.expect("semaphore closed unexpectedly");
    let response = remote.get_lyrics(&request).await;
    drop(permit); // manually drop, since we're done bombarding the website with requests

    match response {
        Ok(
            LyricsResponse {
                synced_lyrics: Some(lyrics),
                instrumental: Some(false) | None,
                ..
            }
            | LyricsResponse {
                plain_lyrics: Some(lyrics),
                instrumental: Some(false) | None,
                ..
            },
        ) => {
            let mut path_owned = path.to_owned();
            match replace_nolrc(&mut path_owned, &lyrics).await {
                Ok(()) => {
                    tracing::info!(path = %path.display(), "successfully replaced nolrc with lrc file");
                }
                Err(ReplaceNolrcError::Delete(e)) if e.kind() == io::ErrorKind::NotFound => {
                    tracing::debug!(path = %path_owned.display(), "nolrc file not found");
                }
                Err(ReplaceNolrcError::Write(e)) => {
                    tracing::warn!(%e, path = %path_owned.display(),"failed to write to lyrics file");
                }
                Err(ReplaceNolrcError::Delete(e)) => {
                    tracing::warn!(%e, path = %path_owned.display(), "failed to delete existing nolrc file");
                }
            }
        }

        // TODO: separate instrumental and tracks with no lyrics available (at this moment)
        Err(LyricsError::InvalidStatusCode {
            status: StatusCode::NOT_FOUND,
            url: _,
        })
        | Ok(_) => {
            if !deny_nolrc {
                // TODO (caching): save this info somewhere and don't try to attempt to get
                // the song lyrics
                tracing::info!(path = %path.display(), "no lyrics found");

                let mut path_owned = path.to_owned();
                match create_nolrc(&mut path_owned).await.map_err(|e| e.kind()) {
                    Ok(_file) => {
                        tracing::info!(path = %path.display(), "successfully created nolrc file");
                    }
                    Err(io::ErrorKind::AlreadyExists) => {
                        tracing::debug!(path = %path_owned.display(), "skipping creation of nolrc file, since it exists");
                    }
                    Err(kind) => {
                        tracing::warn!(path = %path_owned.display(), ?kind, "failed to create nolrc file");
                    }
                }
            } else {
                tracing::debug!(path = %path.display(), "not writing nolrc file");
            }
        }

        Err(e) => match e {
            LyricsError::InvalidRequest(e) => {
                panic!(
                    "constructed invalid request! this is not supposed to happen, ever. {:?}",
                    e
                )
            }
            LyricsError::Misc(inner) => tracing::warn!(%inner),
            LyricsError::InvalidStatusCode { .. } => tracing::warn!(%e),
        },
    }
}

#[derive(Debug, thiserror::Error)]
enum ReplaceNolrcError {
    #[error("failed to write to lrc file due to error: {0}")]
    Write(#[source] io::Error),
    #[error("failed to delete nolrc file due to error: {0}")]
    Delete(#[source] io::Error),
}

#[tracing::instrument(level = "trace", skip_all)]
async fn replace_nolrc<C>(path: &mut PathBuf, lyrics: C) -> Result<(), ReplaceNolrcError>
where
    C: AsRef<[u8]>,
{
    path.set_extension("lrc");
    tokio::fs::write(&path, &lyrics)
        .await
        .map_err(ReplaceNolrcError::Write)?;

    path.set_extension("nolrc");
    tokio::fs::remove_file(&path)
        .await
        .map_err(ReplaceNolrcError::Delete)?;

    Ok(())
}

#[tracing::instrument(level = "trace")]
async fn create_nolrc(path: &mut PathBuf) -> Result<tokio::fs::File, io::Error> {
    path.set_extension("nolrc");
    tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .await
}
