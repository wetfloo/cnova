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
    #[serde(with = "duration_secs")]
    pub duration: Option<Duration>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResponse {
    pub id: u64,
    pub track_name: Option<String>,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
    #[serde(with = "duration_secs")]
    pub duration: Option<Duration>,
    pub instrumental: bool,
    pub plain_lyrics: String,
    pub synced_lyrics: Option<String>,
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

mod duration_secs {
    use serde::de::{self, Unexpected, Visitor};
    use serde::{Deserializer, Serializer};
    use std::fmt::{self, Formatter};
    use std::time::Duration;

    struct OptionVisitor;

    impl<'de> Visitor<'de> for OptionVisitor {
        type Value = Option<Duration>;

        fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
            formatter.write_str("optional floating point value, representing the amount of seconds")
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_f32(self)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            dbg!("visit_none");
            Ok(None)
        }

        fn visit_f32<E>(self, secs: f32) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Duration::try_from_secs_f32(secs)
                .map(Some)
                .map_err(|_| de::Error::invalid_value(Unexpected::Float(secs.into()), &self))
        }

        fn visit_f64<E>(self, secs: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Duration::try_from_secs_f64(secs)
                .map(Some)
                .map_err(|_| de::Error::invalid_value(Unexpected::Float(secs), &self))
        }
    }

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(duration) => serializer.serialize_f32(duration.as_secs_f32()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_option(OptionVisitor)
    }
}
