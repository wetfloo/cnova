mod cli;
mod net;
mod op;
mod remote;
mod trace;

use clap::Parser as _;
use tracing::level_filters::LevelFilter;

use crate::cli::Cli;
use net::RemoteImpl;

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

    let remote = Box::leak(Box::new(RemoteImpl::new(cli.proxy.take()) // not gonna need proxy anywhere else
        .expect(
            "couldn't build remote. this means that we can't execute requests. are all the parameters verified at the cli level?"
        )));

    op::start_up(remote, cli).await;
}
