use clap::Parser as _;
use cli::Cli;
use file::{PackResult, PacksRx};
use remote::Remote;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{level_filters::LevelFilter, Instrument};
use util::TraceErr;

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
        handle_all(remote, semaphore, &mut rx).await;
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
async fn handle_all(remote: Arc<Remote>, semaphore: Arc<tokio::sync::Semaphore>, rx: &mut PacksRx) {
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
                    match response
                        .trace_err()
                        .ok()
                        .and_then(|response| response.synced_lyrics.or(response.plain_lyrics))
                    {
                        Some(lyrics) => tokio::fs::write(&path, &lyrics)
                            .await
                            .inspect_err(|e| tracing::error!(?e, "failed to write to a file"))
                            .unwrap_or_default(),
                        None => tracing::info!(?request, ?path, "couldn\'t extract lyrics"),
                    }
                }
                .in_current_span(),
            );
        }
    }

    join_set.join_all().await;
}
