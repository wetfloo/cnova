use super::{LyricsError, LyricsRequest, LyricsResponse, Remote};
use reqwest::Proxy;
use std::time::Duration;

mod url {
    use const_format::concatcp;

    const BASE: &str = "https://lrclib.net/api/";
    pub const GET: &str = concatcp!(BASE, "get");
}

pub struct RemoteImpl {
    client: reqwest::Client,
}

impl RemoteImpl {
    pub fn new(proxy: Option<Proxy>) -> Result<Self, reqwest::Error> {
        let mut builder = reqwest::ClientBuilder::new().timeout(Duration::from_secs(10));
        builder = if let Some(proxy) = proxy {
            builder.proxy(proxy)
        } else {
            builder.no_proxy()
        };

        builder.build().map(|client| Self { client })
    }
}

impl Remote for RemoteImpl {
    #[tracing::instrument(level = "trace", skip(self))]
    async fn get_lyrics(&self, req: &LyricsRequest) -> Result<LyricsResponse, LyricsError> {
        tracing::trace!("building request");
        let request = self
            .client
            .get(url::GET)
            .query(req)
            .build()
            .map_err(LyricsError::InvalidRequest)?;

        tracing::trace!("requesting the value");
        self.client
            .execute(request)
            .await
            .map_err(|e| e.into())
            .and_then(|response| {
                let status = response.status();
                if status.is_success() {
                    Ok(response)
                } else {
                    Err(LyricsError::InvalidStatusCode {
                        status,
                        url: url::GET,
                    })
                }
            })?
            .json()
            .await
            .map_err(|e| e.into())
    }
}
