//! Pure-Rust page extractor for Mangas Origines (WordPress + custom theme).
//!
//! Extracts images from .reading-content div which contains page-break wrappers.

use aidoku::{Page, PageContent, Result, alloc::vec};
use core::str;

pub fn get_pages(base_url: &str, chapter_url: &str, html: &str) -> Result<aidoku::alloc::vec::Vec<Page>> {
    let doc = aidoku::imports::html::Html::parse(html)?;

    // Select images within .reading-content div
    // Images are wrapped in .page-break divs with id="image-N"
    let pages = doc.select("div.reading-content img.wp-manga-chapter-img, \
                             div.reading-content img[src*='WP-manga/data/'], \
                             div.page-break img")
        .into_iter()
        .filter_map(|el| {
            el.attr("abs:src").and_then(|src| {
                // Skip non-image or ad content
                if src.contains("ad") || src.contains("pub") || src.contains("banner") || 
                   src.contains("/images/") && !src.contains("WP-manga") {
                    return None;
                }
                Some(Page {
                    content: PageContent::Image(src),
                    ..Default::default()
                })
            })
        })
        .collect();

    if pages.is_empty() {
        bail!("No pages found. Open the chapter in the WebView.");
    }

    Ok(pages)
}
