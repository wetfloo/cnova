use clap::Parser as _;
use cli::Cli;
use file::{PackResult, PacksRx};
use remote::{LyricsError, LyricsResponse, Remote};
use reqwest::StatusCode;
use std::{io, path::PathBuf, sync::Arc};
use tokio::task::JoinSet;
use tracing::{level_filters::LevelFilter, Instrument};
use util::{TraceErr as _, TraceLog as _};

mod cli;
mod file;
mod remote;
mod util;

#[tokio::main]
#[tracing::instrument(level = "trace")]
async fn main() {
    const JOIN_HANDLE_EXPECT_MSG: &str =
        "seems like child job panicked. we shouldn't ever panic like that!";
    // tracing
    const TRACING_SET_GLOBAL_DEFAULT_EXPECT_MSG: &str = "unable to set global tracing subscriber";

    if cfg!(debug_assertions) {
        let sub = tracing_subscriber::fmt()
            .with_max_level(LevelFilter::DEBUG)
            .finish();
        tracing::subscriber::set_global_default(sub).expect(TRACING_SET_GLOBAL_DEFAULT_EXPECT_MSG);
    } else {
        let sub = tracing_subscriber::fmt()
            .with_max_level(LevelFilter::INFO)
            .finish();
        tracing::subscriber::set_global_default(sub).expect(TRACING_SET_GLOBAL_DEFAULT_EXPECT_MSG);
    }

    let mut cli = Cli::parse();

    // async preparations
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PackResult>();
    let remote = Arc::new(Remote::new(cli.proxy.take()) // not gonna need proxy anywhere else
        .expect("couldn't build remote. this means that we can't execute requests. are all the parameters verified at the cli level?",
    ));
    // To not overload the site with insane number of requests
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cli.download_jobs.into()));

    let handle = tokio::spawn(async move {
        handle_all(remote, semaphore, &mut rx, cli.deny_nolrc).await;
    });

    tokio::task::spawn_blocking(move || {
        file::prepare_entries(&tx, &cli)
            .expect("the amount of paths provided has to be verified at the cli level");
    })
    .await
    .expect(JOIN_HANDLE_EXPECT_MSG);

    handle.await.expect(JOIN_HANDLE_EXPECT_MSG);
}

#[tracing::instrument(level = "trace", skip_all)]
async fn handle_all(
    remote: Arc<Remote>,
    semaphore: Arc<tokio::sync::Semaphore>,
    rx: &mut PacksRx,
    deny_nolrc: bool,
) {
    let mut join_set = JoinSet::new();

    while let Some(res) = rx.recv().await {
        if let Ok((request, dir_entry)) = res.trace_err() {
            tracing::debug!(?request, ?dir_entry, "received new value");

            let remote = remote.clone();
            let permit = semaphore.clone().acquire_owned();

            join_set.spawn(
                async move {
                    let permit = permit.await.expect("semaphore closed unexpectedly");
                    let response = remote.get_lyrics(&request).await;
                    drop(permit); // manually drop to handle other tasks in this async block in the future

                    let mut path = dir_entry.into_path();

                    assert!(
                        path.set_extension("lrc"),
                        "at this stage, we should always be able to update extensions on files"
                    );

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
                        ) => match replace_nolrc(&mut path, &lyrics).await {
                            Ok(()) => (),
                            Err(ReplaceNolrcError::Delete(e)) if e.kind() == io::ErrorKind::NotFound => tracing::trace!(?path, "nolrc file not found"),
                            Err(ReplaceNolrcError::Write(e)) => tracing::warn!(?path, ?e, "failed to write to lyrics file"),
                            Err(ReplaceNolrcError::Delete(e)) => tracing::warn!(?path, ?e, "failed to delete existing nolrc file"),
                        },

                        Err(LyricsError::InvalidStatusCode {
                            status: StatusCode::NOT_FOUND,
                            url: _,
                        })
                        | Ok(_) => if !deny_nolrc {
                            let mut path = path.to_owned();
                            tracing::info!(?request, ?response, ?path, "couldn\'t extract lyrics");

                            assert!(
                                path.set_extension("nolrc"),
                                "at this stage, we should always be able to update extensions on files"
                            );
                            match tokio::fs::File::create_new(&path).await.map_err(|e| e.kind()) {
                                Ok(_file) => tracing::info!(?path, "successfully created nolrc file"),
                                Err(io::ErrorKind::AlreadyExists) => tracing::trace!("skipping creation of nolrc file, since it exists"),
                                Err(e) => tracing::warn!(?e, "failed to create nolrc file"),
                            }
                        } else {
                            tracing::trace!(?path, ?deny_nolrc, "not writing nolrc file")
                        }

                        Err(e) => {
                            e.trace_log();
                        }
                    }
                }
                .in_current_span(),
            );
        }
    }

    join_set.join_all().await;
}

#[derive(Debug, thiserror::Error)]
enum ReplaceNolrcError {
    #[error("failed to write to lrc file")]
    Write(#[from] io::Error),
    #[error("failed to delete nolrc file")]
    Delete(#[source] io::Error),
}

#[tracing::instrument(level = "trace", skip(lyrics))]
async fn replace_nolrc<C>(path: &mut PathBuf, lyrics: C) -> Result<(), ReplaceNolrcError>
where
    C: AsRef<[u8]>,
{
    tokio::fs::write(&path, &lyrics).await?;
    tracing::info!(?path, "successfully wrote lyrics file");

    path.set_extension("nolrc");
    tokio::fs::remove_file(&path)
        .await
        .map_err(ReplaceNolrcError::Delete)?;
    tracing::info!(?path, "successfully removed nolrc file");

    Ok(())
}
