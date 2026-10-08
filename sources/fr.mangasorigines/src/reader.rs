//! Pure-Rust page extractor for Mangas Origines.

use aidoku::{Page, PageContent, Result};

pub fn get_pages(base_url: &str, chapter_url: &str, html: &str) -> Result<Vec<Page>> {
    let doc = Html::parse(html)?;

    // Select images within .reading-content div
    let pages = doc.select("div.reading-content img.wp-manga-chapter-img, \
                             div.reading-content img[src*='WP-manga/data/'], \
                             div.page-break img")
        .into_iter()
        .filter_map(|el| {
            el.attr("abs:src").and_then(|src| {
                if src.contains("ad") || src.contains("pub") || src.contains("banner") || 
                   (src.contains("/images/") && !src.contains("WP-manga")) {
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
