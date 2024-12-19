use file::DirIterCfg;
use reqwest::StatusCode;
use std::{env, process, sync::Arc};
use tokio::task::JoinSet;

mod file;
mod remote;

#[tokio::main]
#[tracing::instrument(level = "trace")]
async fn main() {
    let dir_iter_cfg = DirIterCfg::default();

    let file_path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("This program needs a path to scan");
        process::exit(1);
    });

    let rx = file::prepare_v2(file_path, &dir_iter_cfg);

    let mut join_set = JoinSet::new();
    // To not overload the site with insane number of requests
    let semaphore = Arc::new(tokio::sync::Semaphore::const_new(3));

    for (request, dir_entry) in rx
        .into_iter()
        // TODO: do something with the result
        .filter_map(|pack| pack.ok().map(|pack| pack.into()))
    {
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
                    .inspect_err(|e| {
                        match e {
                            remote::LyricsError::InvalidRequest(e) => tracing::error!(?e, "this is bad, please report this to the developer"),
                            remote::LyricsError::Misc(e) => tracing::warn!(?e, "misc request error"),
                            remote::LyricsError::InvalidStatusCode { status, url } => if *status == StatusCode::NOT_FOUND {
                                // TODO: save this info somewhere and don't try to attempt to get
                                // the song lyrics
                                tracing::info!(?e, "lyrics not found");
                            } else {
                                tracing::warn!(?e, ?url, "received http error");
                            },
                        }
                        tracing::warn!(?e, "got hit with response error")
                    })
                    .ok()
                    .and_then(|response| response.synced_lyrics.or(response.plain_lyrics)) {
                    Some(lyrics) => tokio::fs::write(&path, &lyrics).await.inspect_err(|e|
                        tracing::error!(?e, "failed to write to a file")
                    ).unwrap_or_default(),
                    None => tracing::info!(?request, "couldn\'t extract lyrics"),
                }
            }
        });
    }

    join_set.join_all().await;
}
