use clap::Parser as _;
use file::PackResult;
use std::sync::Arc;
use tracing::level_filters::LevelFilter;

use cnova::{self, cli::Cli, file, remote::RemoteImpl};

const TRACING_SET_GLOBAL_DEFAULT_EXPECT_MSG: &str = "unable to set global tracing subscriber";
const JOIN_HANDLE_EXPECT_MSG: &str =
    "seems like child job panicked. we shouldn't ever panic like that!";

#[tokio::main]
#[tracing::instrument(level = "trace")]
async fn main() {
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
    let remote = Arc::new(RemoteImpl::new(cli.proxy.take()) // not gonna need proxy anywhere else
        .expect("couldn't build remote. this means that we can't execute requests. are all the parameters verified at the cli level?",
    ));
    // To not overload the site with insane number of requests
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cli.download_jobs.into()));

    let handle = tokio::spawn(async move {
        cnova::handle_all(remote, semaphore, &mut rx, cli.deny_nolrc).await;
    });

    tokio::task::spawn_blocking(move || {
        file::prepare_entries(&tx, &cli)
            .expect("the amount of paths provided has to be verified at the cli level");
    })
    .await
    .expect(JOIN_HANDLE_EXPECT_MSG);

    handle.await.expect(JOIN_HANDLE_EXPECT_MSG);
}
