use super::*;
use crate::cli::{Cli, FileMatchStrictness, LrcAcquireBehavior};
use crate::remote::{self, LyricsError, LyricsRequest, LyricsResponse, Remote};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::{env, tempdir_in, NamedTempFile};
use tokio::fs::try_exists;

use std::iter;
use std::sync::Mutex;

const CREATE_TEMP_FILE_EXPECT_MSG: &str = "failed to create temp file";

struct TestRemoteImpl<I> {
    /// [`Mutex`] makes this type [`Send`] + [`Sync`]
    inner: Mutex<TestRemoteImplInner<I>>,
}

struct TestRemoteImplInner<I> {
    call_count: usize,
    iter: I,
}

impl<I> Remote for TestRemoteImpl<I>
where
    I: Iterator<Item = remote::Result> + Send,
{
    async fn get_lyrics(&self, _req: &LyricsRequest) -> remote::Result {
        let mut lock = self.inner.lock().unwrap();

        lock.call_count += 1;
        lock.iter.next().unwrap()
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
            inner: Mutex::new(TestRemoteImplInner {
                iter: iter.into_iter(),
                call_count: 0,
            }),
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

impl<I> TestRemoteImpl<I> {
    fn call_count(&self) -> usize {
        self.inner.lock().unwrap().call_count
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

fn typical_cli<I>(paths: I) -> Cli
where
    I: IntoIterator<Item = PathBuf>,
{
    Cli {
        paths: paths.into_iter().collect(),
        no_ignore_hidden: false,
        no_follow_symlinks: false,
        lrc_acquire_behavior: LrcAcquireBehavior::LrcMissing,
        deny_nolrc: false,
        strictness: FileMatchStrictness::FilterByExt,
        download_jobs: 1,
        traversal_jobs: 1,
        proxy: None,
    }
}

#[tokio::test]
async fn test_empty_dirs() {
    // 0 files
    let dir = tempdir_in(env::temp_dir()).unwrap();

    let remote = Box::leak(Box::new(TestRemoteImpl::with(typical_ok)));
    let cli = typical_cli(iter::once(dir.into_path()));
    super::start_up(remote, cli).await;

    assert_eq!(0, remote.call_count());
}

#[tokio::test]
async fn test_bad_files() {
    let dir = tempdir_in(env::temp_dir()).unwrap();
    let _file1 = NamedTempFile::with_suffix_in(".flac", dir.path()).unwrap();
    let _file2 = NamedTempFile::with_suffix_in(".mp3", dir.path()).unwrap();

    let remote = Box::leak(Box::new(TestRemoteImpl::with(typical_ok)));
    let cli = typical_cli(iter::once(dir.into_path()));
    super::start_up(remote, cli).await;

    assert_eq!(0, remote.call_count());
}

#[tokio::test]
async fn test_create_nolrc() {
    let file = NamedTempFile::new().expect(CREATE_TEMP_FILE_EXPECT_MSG);
    let mut path = file.path().to_owned();
    let mut path_clone = path.with_extension("flac");

    let res = create_nolrc(&mut path_clone).await;
    assert!(res.is_ok(), "{:?}", path_clone);

    let og_path_exists = try_exists(&path).await;
    assert!(
        matches!(og_path_exists, Ok(true)),
        "{:?}, {:?}",
        og_path_exists,
        path
    );

    path.set_extension("nolrc");
    assert_eq!(path, path_clone);

    let nolrc_exists = try_exists(&path).await;
    assert!(matches!(nolrc_exists, Ok(true)), "{:?}", path);
}

#[tokio::test]
async fn test_replace_nolrc_halfway() {
    let lyrics = "some lyrics go here, right?";
    let file = NamedTempFile::new().expect(CREATE_TEMP_FILE_EXPECT_MSG);
    let mut path = file.path().to_owned();
    let mut path_clone = path.with_extension("flac");

    let res = replace_nolrc(&mut path_clone, lyrics).await;
    assert!(
        matches!(res, Err(ReplaceNolrcError::Delete(_))),
        "{:?}",
        path_clone
    );

    let og_path_exists = try_exists(&path).await;
    assert!(
        matches!(og_path_exists, Ok(true)),
        "{:?}, {:?}",
        og_path_exists,
        path
    );

    path.set_extension("lrc");
    let lrc_exists = try_exists(&path).await;
    assert!(matches!(lrc_exists, Ok(true)), "{:?}", path);
    let lrc_content = tokio::fs::read_to_string(&path).await;
    assert!(lrc_content.is_ok());
    assert_eq!(lyrics, lrc_content.unwrap());

    path.set_extension("nolrc");
    let nolrc_exists = try_exists(&path).await;
    assert!(matches!(nolrc_exists, Ok(false)), "{:?}", path);
}

#[tokio::test]
async fn test_replace_nolrc_fully() {
    let lyrics = "some lyrics go here, right?";
    let file = NamedTempFile::new().expect(CREATE_TEMP_FILE_EXPECT_MSG);
    let mut path = file.path().to_owned();
    let mut path_clone = path.with_extension("flac");

    let res = create_nolrc(&mut path_clone).await;
    assert!(res.is_ok(), "{:?}", path_clone);

    let res = replace_nolrc(&mut path_clone, lyrics).await;
    assert!(res.is_ok(), "{:?}", path_clone);

    let og_path_exists = try_exists(&path).await;
    assert!(
        matches!(og_path_exists, Ok(true)),
        "{:?}, {:?}",
        og_path_exists,
        path
    );

    path.set_extension("lrc");
    let lrc_exists = try_exists(&path).await;
    assert!(matches!(lrc_exists, Ok(true)), "{:?}", path);
    let lrc_content = tokio::fs::read_to_string(&path).await;
    assert!(lrc_content.is_ok());
    assert_eq!(lyrics, lrc_content.unwrap());

    path.set_extension("nolrc");
    let nolrc_exists = try_exists(&path).await;
    assert!(matches!(nolrc_exists, Ok(false)), "{:?}", path);
}
