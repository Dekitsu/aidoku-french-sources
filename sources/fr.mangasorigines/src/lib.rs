#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
    HomeComponentValue, HomeLayout, Listing, ListingKind, ListingProvider, Manga, MangaPageResult,
    Page, Result, Source,
    alloc::{String, Vec, format, string::ToString, vec},
    imports::{
        html::{Document, Html},
        net::Request,
    },
    prelude::*,
};
use core::cell::RefCell;

mod models;
mod reader;

use models::{BASE_URL, first_number, parse_relative_date, strip_base};
use reader::get_pages;

/// Handler for deep links (if needed)
pub struct DeepLinkHandler;

impl DeepLinkHandler {
    pub fn handle(&self, _filter: &FilterValue) -> Option<DeepLinkResult> {
        None
    }
}

fn popular_manga_from_element(el: &Document) -> Option<Manga> {
    let href = el.attr("abs:href")?;
    let title = el.select_first(".entry-title a, h1.entry-title a")?.text().unwrap_or_default();
    let cover = el
        .select_first("div.ori-card-cover img, figure.wp-post-image img")
        .and_then(|i| i.attr("abs:src"));

    Some(Manga {
        key: strip_base(&href),
        title,
        cover,
        ..Default::default()
    })
}

fn search_manga_from_element(el: &Document) -> Option<Manga> {
    let href = el.attr("abs:href")?;
    let title = el.select_first(".entry-title a, h1.entry-title a")?.text().unwrap_or_default();
    let cover = el
        .select_first("div.ori-card-cover img, figure.wp-post-image img")
        .and_then(|i| i.attr("abs:src"));

    Some(Manga {
        key: strip_base(&href),
        title,
        cover,
        ..Default::default()
    })
}

pub fn parse_home(doc: &Document, url: &str) -> MangaPageResult {
    let mut manga = Manga {
        key: strip_base(url),
        title: String::from("Mangas Origines"),
        cover: None,
        ..Default::default()
    };

    // Popular manga from .popular-manga div
    if let Some(popular_div) = doc.select_first(".popular-manga") {
        let popular_manga: Vec<Manga> = popular_div
            .select("div.ori-card-content a[href*='/manga/']")
            .into_iter()
            .filter_map(|el| popular_manga_from_element(&el))
            .collect();

        manga.popular_manga = Some(popular_manga);
    }

    if manga.popular_manga.is_some() {
        return MangaPageResult::Home(manga);
    }

    // Search results from .search-results div
    if let Some(search_div) = doc.select_first(".search-results") {
        let search_manga: Vec<Manga> = search_div
            .select("div.ori-card-content a[href*='/manga/']")
            .into_iter()
            .filter_map(|el| search_manga_from_element(&el))
            .collect();

        manga.manga = Some(search_manga);
    }

    if manga.manga.is_some() {
        return MangaPageResult::Home(manga);
    }

    // Regular listing from .listing div
    if let Some(listing_div) = doc.select_first(".listing") {
        let manga_list: Vec<Manga> = listing_div
            .select("div.ori-card-content a[href*='/manga/']")
            .into_iter()
            .filter_map(|el| popular_manga_from_element(&el))
            .collect();

        manga.manga = Some(manga_list);
    }

    if manga.manga.is_some() {
        return MangaPageResult::Home(manga);
    }

    // Fallback: try to extract from any manga links found
    let manga_links: Vec<Manga> = doc
        .select("a[href*='/manga/']")
        .into_iter()
        .filter_map(|el| popular_manga_from_element(&el))
        .collect();

    if !manga_links.is_empty() {
        manga.manga = Some(manga_links);
        return MangaPageResult::Home(manga);
    }

    MangaPageResult::Empty
}

fn parse_chapters(doc: &Document, manga_url: &str) -> Vec<Chapter> {
    // Chapters are inside .ori-chl-liste div
    let chapters = doc.select("div.ori-chl-liste div.ori-chl-row").into_iter().filter_map(|el| {
        let link = el.select_first(".ori-chl-corps, a[href*='/chapitre/']")?;
        let href = link.attr("abs:href")?;

        // Get chapter number from data-num or title text
        let num_str = el
            .select_first(".ori-chl-num b, span.ori-chl-nom-court")
            .and_then(|e| e.text());

        let date = el
            .select_first("span.ori-chl-date")
            .and_then(|e| e.text());

        Some(Chapter {
            key: strip_base(&href),
            title: link.text().unwrap_or_default(),
            chapter_number: first_number(num_str.unwrap_or_default().as_str()).map(|n| n as f32),
            date_uploaded: parse_relative_date(date.unwrap_or_default().as_str()),
            url: Some(href),
            ..Default::default()
        })
    }).collect();

    chapters
}

pub fn parse_manga(doc: &Document, url: &str) -> Result<Manga> {
    let mut manga = Manga {
        key: strip_base(url),
        title: String::from("Mangas Origines"),
        cover: None,
        ..Default::default()
    };

    // Metadata - in .entry-meta or .post-meta div
    if let Some(meta_div) = doc.select_first(".entry-meta, .post-meta") {
        manga.title = meta_div
            .select_first(".entry-title a, h1.entry-title a")
            .and_then(|e| e.text())
            .unwrap_or_default();

        // Cover - in .ori-card-cover or .wp-post-image div
        manga.cover = doc
            .select_first("div.ori-card-cover img, figure.wp-post-image img")
            .and_then(|i| i.attr("abs:src"));
    }

    // Description - in .item-summary or .entry-content
    manga.description = parse_description(&doc);

    // Tags (genres) - in .ori-sr-genres
    let tags: Vec<String> = doc
        .select("div.ori-sr-genres a, div.genres-content a")
        .map(|els| els.filter_map(|e| e.text()).filter(|t| !t.is_empty()).collect())
        .unwrap_or_default();

    manga.tags = Some(tags);

    // Authors - in .ori-sr-signature
    if let Some(author) = doc.select("div.ori-sr-signature a[href*='/manga-auteurs/']").into_iter().next() {
        manga.authors = Some(vec![author.text().filter(|t| !t.is_empty()).unwrap_or_default()]);
    }

    // Chapters - in .ori-chl-liste div
    if let Some(chapters_div) = doc.select_first(".ori-chl-liste") {
        manga.chapters = Some(parse_chapters(&doc, url));
    }

    Ok(manga)
}

fn parse_description(doc: &Document) -> String {
    doc.select_first(".item-summary, .entry-content")
        .and_then(|el| el.text())
        .unwrap_or_default()
}

pub fn get_chapter_pages(chapter_url: &str, html: &str) -> Result<Vec<Page>> {
    get_pages(chapter_url, chapter_url, html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_base() {
        assert_eq!(strip_base("https://mangas-origines.fr/manga/naruto"), "manga/naruto");
        assert_eq!(strip_base("https://mangas-origines.fr/chapitre/1"), "chapitre/1");
    }

    #[test]
    fn test_first_number() {
        assert_eq!(first_number("Chapitre 1").unwrap(), 1);
        assert_eq!(first_number("Chapitre 10").unwrap(), 10);
    }

    #[test]
    fn test_parse_relative_date() {
        assert!(parse_relative_date("10/08/26").is_some());
        assert!(parse_relative_date("10-08-26").is_some());
    }
}
