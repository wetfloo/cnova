use clap::Parser as _;
use cli::Cli;
use remote::{Remote, RemoteInitError};
use std::{process, sync::Arc};
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

    // cli
    let cli = Cli::parse();

    // basic entities that depend on cli
    let remote = match (Remote::new(cli.proxy.as_ref()), cli.proxy.as_ref()) {
        (Ok(remote), _) => remote,
        (Err(RemoteInitError::Misc(e)), _) => panic!("{:?}", e),
        (Err(RemoteInitError::ProxyError(_)), Some(proxy)) => {
            eprintln!("{} is not a valid proxy string", proxy);
            process::exit(1)
        }
        (Err(RemoteInitError::ProxyError(_)), None) => {
            unreachable!()
        }
    };
    let mut rx = file::prepare_entries(&cli)
        .expect("the amount of paths provided has to be verified at the cli level");

    // async preparations
    let remote = Arc::new(remote);
    let mut join_set = JoinSet::new();
    // To not overload the site with insane number of requests
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cli.download_jobs.into()));

    // TODO: MASSIVE bottleneck here. Jobs don't come in until all files are processed, why???
    while let Some((request, dir_entry)) = rx.recv().await.and_then(|res| res.trace_err().ok()) {
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

    join_set.join_all().await;
}
