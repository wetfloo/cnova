use super::{LyricsError, LyricsRequest, Remote};
use std::sync::Mutex;

pub struct RemoteImpl {
    results: Mutex<Vec<super::Result>>,
}

impl Default for RemoteImpl {
    fn default() -> Self {
        let vec = vec![Err(LyricsError::InvalidStatusCode {
            status: reqwest::StatusCode::BAD_REQUEST,
            url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        })];
        Self {
            results: Mutex::new(vec),
        }
    }
}

impl Remote for RemoteImpl {
    async fn get_lyrics(&self, _req: &LyricsRequest) -> super::Result {
        self.results.lock().unwrap().pop().unwrap()
    }
}
