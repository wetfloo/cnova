use file::DirIterCfg;
use std::{env, process, sync::Arc};
use tokio::task::JoinSet;
use tracing::Instrument;
use tracing_subscriber::prelude::*;

mod file;
mod remote;

#[tokio::main]
#[tracing::instrument(level = "trace")]
async fn main() {
    let stdout_log = tracing_subscriber::fmt::layer().pretty();
    let subscriber = tracing_subscriber::Registry::default().with(stdout_log);
    tracing::subscriber::set_global_default(subscriber)
        .expect("unable to set global tracing subscriber");

    let dir_iter_cfg = DirIterCfg::default();

    let file_path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("This program needs a path to scan");
        process::exit(1);
    });

    let rx = file::prepare_entries(file_path, &dir_iter_cfg);

    let mut join_set = JoinSet::new();
    // To not overload the site with insane number of requests
    let semaphore = Arc::new(tokio::sync::Semaphore::const_new(3));

    for (request, dir_entry) in rx.into_iter().filter_map(|pack| {
        pack.inspect_err(|e| {
            eprintln!("TODO (tracing) {:?}", e);
        })
        .ok()
    }) {
        let semaphore = semaphore.clone();
        join_set.spawn(async move {
            let permit = semaphore
                .acquire_owned()
                .await
                .expect("semaphore closed unexpectedly");
            let response = remote::get_lyrics(&request).await;
            drop(permit); // manually drop to handle other tasks in this async block in the future

            let mut path = dir_entry.into_path();
            if !path.set_extension("lrc") {
                tracing::warn!(?path, "failed to update file path to `lrc` extension. this could mean that this entry is not a file");
            } else {
                match response
                    .inspect_err(|e| e.trace())
                    .ok()
                    .and_then(|response| response.synced_lyrics.or(response.plain_lyrics)) {
                    Some(lyrics) => tokio::fs::write(&path, &lyrics).await.inspect_err(|e|
                        tracing::error!(?e, "failed to write to a file")
                    ).unwrap_or_default(),
                    None => tracing::info!(?request, "couldn\'t extract lyrics"),
                }
            }
        }.in_current_span());
    }

    join_set.join_all().await;
}
