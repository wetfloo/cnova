use file::DirIterCfg;
use std::{env, process, sync::Arc};
use tokio::task::JoinSet;
use tracing::{level_filters::LevelFilter, Instrument};
use util::TraceErr;

mod file;
mod remote;
mod util;

#[tokio::main]
#[tracing::instrument(level = "trace")]
async fn main() {
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

    let dir_iter_cfg = DirIterCfg::default();

    let file_path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("This program needs a path to scan");
        process::exit(1);
    });

    let rx = file::prepare_entries(file_path, &dir_iter_cfg);

    let mut join_set = JoinSet::new();
    // To not overload the site with insane number of requests
    let semaphore = Arc::new(tokio::sync::Semaphore::const_new(3));

    for (request, dir_entry) in rx.into_iter().filter_map(|pack| pack.trace_err().ok()) {
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
                    .trace_err()
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
