use file::DirIterCfg;
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

    let files = file::list_files(file_path, &dir_iter_cfg);
    let requests = file::prepare_entries(files, &dir_iter_cfg);

    let mut join_set = JoinSet::new();
    // To not overload the site with insane number of requests
    let semaphore = Arc::new(tokio::sync::Semaphore::const_new(3));

    for (request, dir_entry) in requests {
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
                    .inspect_err(|err| tracing::warn!(?err, "got hit with response error"))
                    .ok()
                    .and_then(|response| response.synced_lyrics.or(response.plain_lyrics)) {
                    Some(lyrics) => tokio::fs::write(&path, &lyrics).await.inspect_err(|e|
                        tracing::warn!(?e, "failed to write to a file")
                    ).unwrap_or_default(),
                    None => tracing::info!(?request, "couldn\'t extract lyrics")
                }
            }
        });
    }

    join_set.join_all().await;
}
