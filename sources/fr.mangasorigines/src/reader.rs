//! Pure-Rust page extractor for Mangas Origines.

use aidoku::{Page, PageContent, Result};

pub fn get_pages(_chapter_url: &str, html: &str) -> Result<Vec<Page>> {
    bail!("No pages found. Open the chapter in the WebView.");
}
