use tracing::level_filters::LevelFilter;

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

    cnova::prepare().await;
}
