use file::DirIterCfg;
use std::{env, process, sync::Arc};
use tokio::task::JoinSet;

mod file;
mod remote;
mod util;

#[tokio::main]
async fn main() {
    let dir_iter_cfg = DirIterCfg::default();

    let file_path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("This program needs a path to scan");
        process::exit(1);
    });

    let files = file::list_files(file_path, &dir_iter_cfg);
    let requests = file::all_file_requests(&files, &dir_iter_cfg);

    let mut join_set = JoinSet::new();
    // To not overload the site with insane number of requests
    let semaphore = Arc::new(tokio::sync::Semaphore::const_new(3));

    for request in requests {
        let semaphore = semaphore.clone();
        join_set.spawn(async move {
            let permit = semaphore
                .acquire_owned()
                .await
                .expect("semaphore closed unexpectedly");
            let response = remote::get_lyrics(&request).await;
            drop(permit); // manually drop to handle other tasks in this async block in the future

            response
        });
    }

    for item in join_set.join_all().await {}
}
