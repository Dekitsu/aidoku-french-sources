#![no_std]
use aidoku::{prelude::*, Source, Viewer};
use madara::{Impl, Madara, Params};

struct ManhuaRmTL;

impl Impl for ManhuaRmTL {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://manhuarmtl.com".into(),
			source_path: "manga".into(),
			use_new_chapter_endpoint: true,
			default_viewer: Viewer::Webtoon,
			datetime_format: "MMMM d, yyyy".into(),
			datetime_locale: "en_US_POSIX".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<ManhuaRmTL>,
	Home,
	ListingProvider,
	DynamicFilters,
	ImageRequestProvider,
	DeepLinkHandler,
	MigrationHandler
);
