use super::{LyricsError, LyricsRequest, LyricsResponse, Remote};
use std::iter;
use std::marker::PhantomData;
use std::sync::Mutex;

pub struct TestRemoteImpl<I, A> {
    /// Makes this type Send + Sync
    iter: Mutex<I>,
    /// Nasty hack to avoid "conflicting implementations" error
    _phantom: PhantomData<A>,
}

impl<I> Remote for TestRemoteImpl<I, super::Result>
where
    I: Iterator<Item = super::Result> + Send,
{
    async fn get_lyrics(&self, _req: &LyricsRequest) -> super::Result {
        self.iter.lock().unwrap().next().unwrap()
    }
}

impl<I> Remote for TestRemoteImpl<I, LyricsResponse>
where
    I: Iterator<Item = LyricsResponse> + Send,
{
    async fn get_lyrics(&self, _req: &LyricsRequest) -> super::Result {
        Ok(self.iter.lock().unwrap().next().unwrap())
    }
}

impl<I> Remote for TestRemoteImpl<I, LyricsError>
where
    I: Iterator<Item = LyricsError> + Send,
{
    async fn get_lyrics(&self, _req: &LyricsRequest) -> super::Result {
        Err(self.iter.lock().unwrap().next().unwrap())
    }
}

impl<T, I, A> From<T> for TestRemoteImpl<I, A>
where
    I: Iterator<Item = A>,
    T: IntoIterator<Item = I::Item, IntoIter = I>,
{
    fn from(iter: T) -> Self {
        Self {
            iter: Mutex::new(iter.into_iter()),
            _phantom: PhantomData,
        }
    }
}

impl<A> TestRemoteImpl<iter::Repeat<A>, A>
where
    A: Clone,
{
    /// Get a new [`TestRemoteImpl`] from a single value.
    /// This value will always be repeated when calling [`Remote::get_lyrics`]
    pub fn new_from_value(value: A) -> Self {
        iter::repeat(value).into()
    }
}
