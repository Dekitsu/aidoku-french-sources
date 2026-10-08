#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler as DLH, DeepLinkResult, FilterValue, Home, HomeComponent,
    HomeComponentValue, HomeLayout, Listing, ListingKind, ListingProvider, Manga, MangaPageResult,
    Page, Result, Source,
    alloc::{String, Vec, format, string::ToString, vec},
    imports::{
        html::{Document, ElementList, Html},
        net::Request,
    },
    prelude::*,
};

mod models;
mod reader;

use models::{BASE_URL, first_number, parse_relative_date, strip_base};
use reader::get_pages;

/// Minimal implementation of DeepLinkHandler trait
pub struct MangaOriginesDLH;

impl DLH for MangaOriginesDLH {
    fn handle(&self, _filter: &FilterValue) -> Option<DeepLinkResult> {
        None
    }
}

fn popular_manga_from_element(el: &ElementList) -> Vec<Manga> {
    el.into_iter()
        .map(|e| {
            let href = e.attr("abs:href").unwrap_or_default();
            let title = e.select_first(".entry-title a, h1.entry-title a")
                .and_then(|a| a.text())
                .unwrap_or_default();
            let cover = e
                .select_first("div.ori-card-cover img, figure.wp-post-image img")
                .and_then(|i| i.attr("abs:src"));

            Manga {
                key: strip_base(&href),
                title,
                cover,
                ..Default::default()
            }
        })
        .collect()
}

fn search_manga_from_element(el: &ElementList) -> Vec<Manga> {
    popular_manga_from_element(el)
}

pub fn parse_home(doc: &Document, url: &str) -> Result<Vec<Manga>> {
    let manga_links = doc
        .select("div.ori-card-content a[href*='/manga/'], div.popular-manga a[href*='/manga/']")
        .into_iter()
        .filter_map(|el| popular_manga_from_element(&el))
        .collect();

    Ok(manga_links)
}

fn parse_chapters(doc: &Document, manga_url: &str) -> Vec<Chapter> {
    doc.select("div.ori-chl-liste div.ori-chl-row")
        .into_iter()
        .filter_map(|el| {
            let link = el.select_first(".ori-chl-corps, a[href*='/chapitre/']")?;
            let href = link.attr("abs:href")?;

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
        })
        .collect()
}

pub fn parse_manga(doc: &Document, url: &str) -> Result<Manga> {
    let mut manga = Manga {
        key: strip_base(url),
        title: String::from("Mangas Origines"),
        cover: None,
        ..Default::default()
    };

    if let Some(meta_div) = doc.select_first(".entry-meta, .post-meta") {
        manga.title = meta_div
            .select_first(".entry-title a, h1.entry-title a")
            .and_then(|e| e.text())
            .unwrap_or_default();
        manga.cover = doc
            .select_first("div.ori-card-cover img, figure.wp-post-image img")
            .and_then(|i| i.attr("abs:src"));
    }

    manga.description = Some(parse_description(&doc));

    let tags: Vec<String> = doc
        .select("div.ori-sr-genres a, div.genres-content a")
        .map(|els| els.filter_map(|e| e.text()).filter(|t| !t.is_empty()).collect())
        .unwrap_or_default();
    manga.tags = Some(tags);

    if let Some(author) = doc.select("div.ori-sr-signature a[href*='/manga-auteurs/']").into_iter().next() {
        manga.authors = Some(vec![author.text().filter(|t| !t.is_empty()).unwrap_or_default()]);
    }

    if let Some(chapters_div) = doc.select_first(".ori-chl-liste") {
        manga.chapters = Some(parse_chapters(&doc, url));
    }

    Ok(manga)
}

fn parse_description(doc: &Document) -> String {
    doc.select_first(".item-summary, .entry-content").and_then(|el| el.text()).unwrap_or_default()
}

pub fn get_chapter_pages(chapter_url: &str, html: &str) -> Result<Vec<Page>> {
    get_pages(chapter_url, chapter_url, html)
}
