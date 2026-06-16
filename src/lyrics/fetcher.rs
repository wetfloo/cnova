use crate::lyrics::Lyrics;
use crate::lyrics::TagData;
use crate::lyrics::TaggedFileData;
use crate::lyrics::service;
use crate::lyrics::service::LyricsServiceError;

pub(crate) type LyricsFetcherResult = Result<Lyrics, Vec<LyricsServiceError>>;
type LyricsFetchService = Box<dyn service::LyricsFetchService>;

#[derive(Default, Debug)]
pub(crate) struct LyricsFetcher {
	services: Vec<LyricsFetchService>,
}

impl LyricsFetcher {
	/// For any service added via
	/// [`LyricsFetcherBuilder::add_service`] or [`LyricsFetcherBuilder::new`],
	/// it will be polled one by one, *from first to last added*,
	/// until one returns successfully, then its value will be returned.
	/// Any remaining errors will be returned in [`Err`],
	/// and any other services that could do work would be ignored.
	pub(crate) async fn request_lyrics(&self, data: &TaggedFileData<'_>) -> LyricsFetcherResult {
		let mut errors = Vec::with_capacity(0);

		for service in self.services.iter() {
			match service.request_lyrics(data).await {
				Ok(v) => return Ok(v),
				Err(e) => errors.push(e),
			}
		}

		Err(errors)
	}

	/// Creates a new [`LyricsFetcherBuilder`] to make [`LyricsFetcher`].
	/// See [LyricsFetcherBuilder::new] for details.
	pub(crate) fn builder(service: LyricsFetchService) -> LyricsFetcherBuilder {
		LyricsFetcherBuilder::new(service)
	}
}

#[derive(Default, Debug)]
pub(crate) struct LyricsFetcherBuilder {
	fetcher: LyricsFetcher,
}

impl LyricsFetcherBuilder {
	/// Creates a new builder, accepting an instance of [`LyricsFetchService`],
	/// accepting additional instances via [`add_service`](LyricsFetcherBuilder::add_service).
	///
	/// Also see: [`LyricsFetcher::request_lyrics`].
	pub(crate) fn new(service: LyricsFetchService) -> Self {
		Self {
			fetcher: LyricsFetcher {
				services: vec![service],
			},
		}
	}

	pub(crate) fn add_service(mut self, service: LyricsFetchService) -> Self {
		self.fetcher.services.push(service);

		self
	}

	pub(crate) fn build(self) -> LyricsFetcher {
		self.fetcher
	}
}
