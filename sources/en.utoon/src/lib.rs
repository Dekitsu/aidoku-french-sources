#![no_std]
use aidoku::{prelude::*, Result, Source, Viewer};
use madara::{Impl, LoadMoreStrategy, Madara, Params};

struct Utoon;

impl Impl for Utoon {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://utoon.net".into(),
			source_path: "manga".into(),
			use_new_chapter_endpoint: true,
			use_load_more_request: LoadMoreStrategy::Always,
			default_viewer: Viewer::Webtoon,
			datetime_format: "dd MMM yyyy".into(),
			datetime_locale: "en_US_POSIX".into(),
			// Exclude premium-gated chapters
			chapter_selector: "li.wp-manga-chapter:not(.premium-block)".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<Utoon>,
	Home,
	ListingProvider,
	DynamicFilters,
	ImageRequestProvider,
	DeepLinkHandler,
	MigrationHandler
);
