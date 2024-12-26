use core::fmt;
use reqwest::Proxy;
use serde::{Deserialize, Serialize};
use std::time::Duration;

mod duration_secs;

mod url {
    use const_format::concatcp;

    const BASE: &str = "https://lrclib.net/api/";
    pub const GET: &str = concatcp!(BASE, "get");
}

#[derive(Debug, Serialize)]
pub struct LyricsRequest {
    #[serde(rename = "artist_name")]
    pub artist: String,
    #[serde(rename = "track_name")]
    pub title: String,
    #[serde(rename = "album_name")]
    pub album: Option<String>,
    #[serde(with = "duration_secs")]
    pub duration: Option<Duration>,
}

/// Represents a response containing all the available info about the track, deserialized
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResponse {
    pub id: Option<u64>,
    #[serde(rename = "trackName")]
    pub title: String,
    #[serde(rename = "artistName")]
    pub artist: String,
    #[serde(rename = "albumName")]
    pub album: Option<String>,
    #[serde(with = "duration_secs")]
    /// Track duration, parsed from seconds
    pub duration: Option<Duration>,
    pub instrumental: Option<bool>,
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
}

impl fmt::Display for LyricsResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "title: {}, artist: {}, ", self.title, self.artist)?;

        let lyrics = self
            .synced_lyrics
            .as_ref()
            .or(self.plain_lyrics.as_ref())
            .filter(|_| !self.instrumental.unwrap_or(false));
        match lyrics {
            Some(_lyrics) => f.write_str("LYRICS PRESENT")?,
            None => f.write_str("NO LYRICS")?,
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("invalid Reqwest request, THIS ONE IS BAD! {0:?}")]
    InvalidRequest(#[source] reqwest::Error),
    #[error(transparent)]
    Misc(#[from] reqwest::Error),
    #[error("invalid status code {status} from url {url}")]
    InvalidStatusCode {
        status: reqwest::StatusCode,
        url: &'static str,
    },
}

pub struct Remote {
    client: reqwest::Client,
}

impl Remote {
    pub fn new(proxy: Option<Proxy>) -> Result<Self, reqwest::Error> {
        let mut builder = reqwest::ClientBuilder::new().timeout(Duration::from_secs(10));
        builder = if let Some(proxy) = proxy {
            builder.proxy(proxy)
        } else {
            builder.no_proxy()
        };

        builder.build().map(|client| Self { client })
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn get_lyrics(&self, req: &LyricsRequest) -> Result<LyricsResponse, LyricsError> {
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
