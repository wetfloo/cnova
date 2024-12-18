use reqwest::Proxy;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;

mod url {
    use const_format::concatcp;

    const BASE: &str = "https://lrclib.net/api/";
    pub const GET: &str = concatcp!(BASE, "get");
}

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| client_init().unwrap());

#[derive(Debug, Serialize)]
pub struct LyricsRequest {
    #[serde(rename = "artist_name")]
    pub artist: String,
    #[serde(rename = "track_name")]
    pub title: String,
    #[serde(rename = "album_name")]
    pub album: Option<String>,
    #[serde(rename = "duration")]
    pub duration_secs: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResponse {
    pub id: u64,
    pub track_name: Option<String>,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
    #[serde(rename = "duration")]
    pub duration_secs: f32,
    pub instrumental: bool,
    pub plain_lyrics: String,
    pub synced_lyrics: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("invalid Reqwest request")]
    InvalidRequest(#[source] reqwest::Error),
    #[error("an error occured")]
    Misc(#[from] reqwest::Error),
    #[error("invalid status code {status} from url {url}")]
    InvalidStatusCode {
        status: reqwest::StatusCode,
        url: &'static str,
    },
}

pub async fn get_lyrics(req: &LyricsRequest) -> Result<LyricsResponse, LyricsError> {
    let request = CLIENT
        .get(url::GET)
        .query(req)
        .build()
        .map_err(LyricsError::InvalidRequest)?;

    CLIENT
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

#[derive(Debug, thiserror::Error)]
enum ClientInitErr {
    #[error("entered proxy address is not valid")]
    ProxyError(#[source] reqwest::Error),
    #[error("failed to build client")]
    Misc(#[from] reqwest::Error),
}

#[inline(always)]
fn client_init() -> Result<reqwest::Client, ClientInitErr> {
    let proxy = Proxy::all("socks5://127.0.0.1:2080").map_err(ClientInitErr::ProxyError)?;
    reqwest::ClientBuilder::new()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.into())
}
