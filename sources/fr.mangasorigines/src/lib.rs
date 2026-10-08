#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
    HomeComponentValue, HomeLayout, Listing, ListingKind, ListingProvider, Manga, MangaPageResult,
    Page, Result, Source, send_partial_result,
    alloc::{String, Vec, format, string::ToString, vec},
    imports::{
        html::{Document, Element, Html},
        net::Request,
        std::send_partial_result as sp,
    },
    prelude::*,
};

mod models;
mod reader;

use models::{BASE_URL, first_number, parse_relative_date, strip_base};
use reader::get_pages;

pub struct MangaOrigines;

impl Source for MangaOrigines {
    fn new() -> Self {
        MangaOrigines
    }

    fn get_home(&self, url: &str, filter: Option<&FilterValue>) -> Result<MangaPageResult> {
        let doc = Html::parse(filter.map(|f| f.as_str()).unwrap_or(url));

        // Extract manga links from popular/listing sections
        let manga_links = doc
            .select("div.ori-card-content a[href*='/manga/'], div.popular-manga a, div.listing a")
            .into_iter()
            .filter_map(|el| {
                let href = el.attr("abs:href")?;
                let title = el.select_first(".entry-title a, h1.entry-title a, div.ori-card-title")?.text()?;
                let cover = el
                    .select_first("div.ori-card-cover img, figure.wp-post-image img, div.ori-card-cover img")
                    .and_then(|i| i.attr("abs:src"));

                Some(Manga {
                    key: strip_base(&href),
                    title,
                    cover,
                    ..Default::default()
                })
            })
            .collect();

        if !manga_links.is_empty() {
            Ok(MangaPageResult::Home(Manga {
                key: strip_base(url),
                title: String::from("Mangas Origines"),
                manga: Some(manga_links),
                ..Default::default()
            }))
        } else {
            Ok(MangaPageResult::Empty)
        }
    }

    fn get_manga(&self, url: &str) -> Result<Manga> {
        let doc = Html::parse(url);

        let mut manga = Manga {
            key: strip_base(url),
            title: String::from("Mangas Origines"),
            cover: None,
            ..Default::default()
        };

        // Extract title and cover from metadata section
        if let Some(meta_div) = doc.select_first(".entry-meta, .post-meta, div.ori-card-content") {
            manga.title = meta_div
                .select_first(".entry-title a, h1.entry-title a, div.ori-card-title")
                .and_then(|e| e.text())
                .unwrap_or_default();
            manga.cover = doc
                .select_first("div.ori-card-cover img, figure.wp-post-image img")
                .and_then(|i| i.attr("abs:src"));
        }

        // Extract tags (genres)
        if let Some(genres_div) = doc.select_first(".ori-sr-genres, div.genres-content") {
            manga.tags = Some(
                genres_div
                    .select("a")
                    .into_iter()
                    .filter_map(|e| e.text())
                    .filter(|t| !t.is_empty())
                    .collect(),
            );
        }

        // Extract authors
        if let Some(author) = doc.select("div.ori-sr-signature a[href*='/manga-auteurs/']").into_iter().next() {
            manga.authors = Some(vec![author.text().filter(|t| !t.is_empty()).unwrap_or_default()]);
        }

        // Extract chapters
        if let Some(chapters_div) = doc.select_first(".ori-chl-liste") {
            manga.chapters = Some(
                chapters_div
                    .select("div.ori-chl-row")
                    .into_iter()
                    .filter_map(|el| {
                        let link = el.select_first(".ori-chl-corps, a[href*='/chapitre/']")?;
                        let href = link.attr("abs:href")?;

                        let num_str = el
                            .select_first(".ori-chl-num b, span.ori-chl-nom-court")
                            .and_then(|e| e.text());

                        let date = el.select_first("span.ori-chl-date").and_then(|e| e.text());

                        Some(Chapter {
                            key: strip_base(&href),
                            title: link.text().unwrap_or_default(),
                            chapter_number: first_number(num_str.unwrap_or_default().as_str()).map(|n| n as f32),
                            date_uploaded: parse_relative_date(date.unwrap_or_default().as_str()),
                            url: Some(href),
                            ..Default::default()
                        })
                    })
                    .collect(),
            );
        }

        Ok(manga)
    }

    fn get_chapter(&self, chapter_url: &str, html: &str) -> Result<Vec<Page>> {
        get_pages(chapter_url, chapter_url, html)
    }
}

impl DeepLinkHandler for MangaOrigines {
    fn handle(&self, _filter: &FilterValue) -> Option<DeepLinkResult> {
        None
    }
}
