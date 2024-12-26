use cnova::cli::Cli;
use cnova::file::{self, PacksRx, PacksTx};
use cnova::remote::{self, LyricsError, LyricsRequest, LyricsResponse, Remote};
use cnova::wrapper;
use std::time::Duration;

use std::iter;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

struct TestRemoteImpl<I> {
    /// [`Mutex`] makes this type [`Send`] + [`Sync`]
    iter: Mutex<I>,
}

impl<I> Remote for TestRemoteImpl<I>
where
    I: Iterator<Item = remote::Result> + Send,
{
    async fn get_lyrics(&self, _req: &LyricsRequest) -> remote::Result {
        self.iter.lock().unwrap().next().unwrap()
    }
}

impl<I, A> TestRemoteImpl<I>
where
    I: Iterator<Item = A>,
{
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = I::Item, IntoIter = I>,
    {
        Self {
            iter: Mutex::new(iter.into_iter()),
        }
    }
}

impl<A, F> TestRemoteImpl<iter::RepeatWith<F>>
where
    F: FnMut() -> A,
{
    fn with(f: F) -> Self {
        Self::from_iter(iter::repeat_with(f))
    }
}

fn typical_ok() -> remote::Result {
    Ok(LyricsResponse {
        id: Some(0),
        title: "title".to_owned(),
        artist: "artist".to_owned(),
        album: Some("album".to_owned()),
        duration: Some(Duration::from_secs(10)),
        instrumental: Some(false),
        plain_lyrics: Some("plain_lyrics".to_owned()),
        synced_lyrics: Some("synced_lyrics".to_owned()),
    })
}

fn typical_err() -> remote::Result {
    Err(LyricsError::InvalidStatusCode {
        status: reqwest::StatusCode::FORBIDDEN,
        url: "url",
    })
}

#[tokio::test]
async fn test_a() {
    let remote = Arc::new(TestRemoteImpl::with(|| typical_ok()));
}
