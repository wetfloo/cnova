use std::fmt;

use crate::lyrics::Lyrics;
use crate::lyrics::TagData;
use crate::lyrics::service::LyricsFetchService;
use crate::lyrics::service::LyricsServiceError;

pub(crate) type LyricsFetcherResult = Result<Lyrics, Vec<LyricsServiceError>>;

#[derive(Default, Debug)]
pub struct LyricsFetcher {
	services: Vec<Box<dyn LyricsFetchService>>,
}

impl LyricsFetcher {
	pub async fn request_lyrics(&self, data: &TagData) -> LyricsFetcherResult {
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

#[derive(Default, Debug)]
pub struct LyricsFetcherBuilder {
	fetcher: LyricsFetcher,
}

impl LyricsFetcherBuilder {
	pub fn new(service: Box<dyn LyricsFetchService>) -> Self {
		Self {
			fetcher: LyricsFetcher {
				services: vec![service],
			},
		}
	}

	pub fn add_service(mut self, service: Box<dyn LyricsFetchService>) -> Self {
		self.fetcher.services.push(service);

		self
	}

	pub fn build(self) -> LyricsFetcher {
		self.fetcher
	}
}
