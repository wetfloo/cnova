use serde::de::{self, Unexpected, Visitor};
use serde::{Deserializer, Serializer};
use std::fmt::{self, Formatter};
use std::time::Duration;

struct OptionVisitor;

impl<'de> Visitor<'de> for OptionVisitor {
    type Value = Option<Duration>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("optional number point value, representing the amount of seconds")
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

    fn visit_u64<E>(self, secs: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Some(Duration::from_secs(secs)))
    }

    fn visit_u32<E>(self, secs: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_u64(secs.into())
    }

    fn visit_u16<E>(self, secs: u16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_u64(secs.into())
    }

    fn visit_u8<E>(self, secs: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_u64(secs.into())
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

#[cfg(test)]
mod test {
    use super::super::LyricsResponse;
    use indoc::indoc;

    use std::time::Duration;

    #[test]
    fn parse_with_duration_float() {
        let data = indoc! {r#"
                {
                    "id": 42069,
                    "trackName": "title",
                    "artistName": "artist",
                    "albumName": "album",
                    "duration": 300.0,
                    "instrumental": true,
                    "plainLyrics": "Some lyrics",
                    "syncedLyrics": "Some synced lyrics"
                }
            "#};
        let value: LyricsResponse = serde_json::from_str(data).unwrap();

        assert_eq!(
            LyricsResponse {
                id: Some(42069),
                title: "title".to_owned(),
                artist: "artist".to_owned(),
                album: Some("album".to_owned()),
                duration: Some(Duration::from_secs(300)),
                instrumental: Some(true),
                plain_lyrics: Some("Some lyrics".to_owned()),
                synced_lyrics: Some("Some synced lyrics".to_owned()),
            },
            value
        );
    }

    #[test]
    fn parse_with_duration_int() {
        let data = indoc! {r#"
                {
                    "id": 42069,
                    "trackName": "title",
                    "artistName": "artist",
                    "albumName": "album",
                    "duration": 300,
                    "instrumental": true,
                    "plainLyrics": "Some lyrics",
                    "syncedLyrics": "Some synced lyrics"
                }
            "#};
        let value: LyricsResponse = serde_json::from_str(data).unwrap();

        assert_eq!(
            LyricsResponse {
                id: Some(42069),
                title: "title".to_owned(),
                artist: "artist".to_owned(),
                album: Some("album".to_owned()),
                duration: Some(Duration::from_secs(300)),
                instrumental: Some(true),
                plain_lyrics: Some("Some lyrics".to_owned()),
                synced_lyrics: Some("Some synced lyrics".to_owned()),
            },
            value
        );
    }
}
