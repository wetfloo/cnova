use super::duration_secs;
use core::fmt;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::Duration;

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
#[cfg_attr(test, derive(Clone))]
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

pub type Result = std::result::Result<LyricsResponse, LyricsError>;

pub trait Remote {
    fn get_lyrics(&self, req: &LyricsRequest) -> impl Future<Output = Result> + Send;
}
