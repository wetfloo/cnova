use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;

#[derive(Debug)]
// TODO::perf consider using string slice refs here to implement zero-copy
pub struct TagData {
	pub artist: String,
	pub album: String,
	pub title: String,
}

pub struct TaggedFileInfo {
	pub file: lofty::file::TaggedFile,
	pub path: PathBuf,
}

pub struct Lyrics(pub String);

pub trait LyricsFetchService: fmt::Debug {
	fn request_lyrics(
		&self,
		data: &TagData,
	) -> Pin<Box<dyn Future<Output = Result<Lyrics, LyricsFetchError>> + Send + Sync>>;
}

#[derive(Default, Debug)]
pub struct LyricsFetcher {
	services: Vec<Box<dyn LyricsFetchService>>,
}

#[derive(Default, Debug)]
pub struct LyricsFetcherBuilder {
	fetcher: LyricsFetcher,
}

impl LyricsFetcherBuilder {
	pub fn new() -> Self {
		Self {
			fetcher: LyricsFetcher {
				services: Vec::new(),
			},
		}
	}

	pub fn add_service(mut self, service: Box<dyn LyricsFetchService>) -> Self {
		self.fetcher.services.push(service);

		self
	}

	/// Attempts to build [LyricsFetcher], returning [Some] on success,
	/// and returning [None] if no services were provided
	/// (using [add_service](Self::add_service))
	pub fn build(self) -> Option<LyricsFetcher> {
		if !self.fetcher.services.is_empty() {
			Some(self.fetcher)
		} else {
			None
		}
	}
}

impl LyricsFetcher {
	async fn request_lyrics(&self, data: &TagData) -> Result<Lyrics, Vec<LyricsFetchError>> {
		let mut errors = Vec::with_capacity(0);

		for service in self.services.iter() {
			match service.request_lyrics(data).await {
				Ok(v) => return Ok(v),
				Err(e) => errors.push(e),
			}
		}

		Err(errors)
	}
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum LyricsFetchError {
	#[error("this track is instrumental and, therefore, there aren't any lyrics")]
	Instrumental,
	#[error("TODO: some net error here?")]
	Network,
}

pub struct FakeLyricsFetcher;

#[cfg(test)]
mod test {
	use rand::RngCore;

	use crate::lrc_req::LyricsFetcher;

	use super::Lyrics;
	use super::LyricsFetchError;
	use super::LyricsFetchService;
	use super::LyricsFetcherBuilder;
	use super::TagData;
	use super::TaggedFileInfo;

	use std::cell::RefCell;
	use std::future;
	use std::pin::Pin;

	#[derive(Debug, Default)]
	struct OkLyricsFetcher;

	impl LyricsFetchService for OkLyricsFetcher {
		fn request_lyrics(
			&self,
			data: &TagData,
		) -> Pin<Box<dyn Future<Output = Result<Lyrics, LyricsFetchError>> + Send + Sync>> {
			Box::pin(future::ready(Ok(Lyrics(format!(
				"These are test lyrics for a song {} by {}.",
				data.title, data.artist,
			)))))
		}
	}

	#[derive(Debug, Default)]
	struct ErrInstrumentalLyricsFetcher;

	impl LyricsFetchService for ErrInstrumentalLyricsFetcher {
		fn request_lyrics(
			&self,
			data: &TagData,
		) -> Pin<Box<dyn Future<Output = Result<Lyrics, LyricsFetchError>> + Send + Sync>> {
			Box::pin(future::ready(Err(
				LyricsFetchError::Instrumental,
			)))
		}
	}

	#[tokio::test]
	async fn test1() {
		let lyrics_fetcher = LyricsFetcherBuilder::new()
			.add_service(Box::new(OkLyricsFetcher))
			.build()
			.unwrap();
	}
}
