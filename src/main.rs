use clap::Parser as _;
use std::sync::Arc;
use tracing::level_filters::LevelFilter;

use cnova::{self, cli::Cli, remote::RemoteImpl};

const TRACING_SET_GLOBAL_DEFAULT_EXPECT_MSG: &str = "unable to set global tracing subscriber";

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

    let remote = Arc::new(RemoteImpl::new(cli.proxy.take()) // not gonna need proxy anywhere else
        .expect(
            "couldn't build remote. this means that we can't execute requests. are all the parameters verified at the cli level?"
        ));

    cnova::start_up(remote, cli).await;
}
