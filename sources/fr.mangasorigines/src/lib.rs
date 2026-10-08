#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler, Home, ListingProvider, Manga, MangaPageResult, Page, Result, Source,
    alloc::{String, Vec},
    imports::{net::Request, html::Html},
    prelude::*,
};

mod models;
mod reader;

use models::{BASE_URL, first_number, parse_relative_date, strip_base};
use reader::get_pages;

pub struct MangaOrigines;

impl Source for MangaOrigines {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        _query: Option<String>,
        _page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        Ok(MangaPageResult { entries: vec![], has_next_page: false })
    }

    fn get_manga_update(&self, _manga: Manga, _notify: bool, _replace: bool) -> Result<Manga> {
        Ok(_manga)
    }

    fn get_page_list(&self, _manga: Manga, _chapter: Chapter) -> Result<Vec<Page>> {
        let doc = Html::parse(&_chapter.url.unwrap_or_default());
        get_pages(&_chapter.url.unwrap_or_default(), &_chapter.url.unwrap_or_default(), &doc.to_string())
    }
}

impl ListingProvider for MangaOrigines {
    fn get_manga_list(&self, _listing: Listing, _page: i32) -> Result<MangaPageResult> {
        Ok(MangaPageResult { entries: vec![], has_next_page: false })
    }
}

impl Home for MangaOrigines {
    fn get_home(&self) -> Result<HomeLayout> {
        let doc = Html::parse("");
        
        // Extract popular manga
        let popular_manga: Vec<Manga> = doc
            .select("div.popular-manga div.ori-card-content a[href*='/manga/']")
            .into_iter()
            .filter_map(|el| {
                let href = el.attr("abs:href")?;
                let title = el.select_first(".entry-title a, h1.entry-title a, div.ori-card-title")?.text()?;
                let cover = el
                    .select_first("div.ori-card-cover img, figure.wp-post-image img")
                    .and_then(|i| i.attr("abs:src"));

                Some(Manga {
                    key: strip_base(&href),
                    title,
                    cover,
                    ..Default::default()
                })
            })
            .collect();

        if !popular_manga.is_empty() {
            Ok(HomeLayout {
                manga: Some(popular_manga),
                ..Default::default()
            })
        } else {
            Ok(HomeLayout {
                latest_releases: vec![],
                popular_manga: vec![],
                trending_manga: vec![],
                ..Default::default()
            })
        }
    }
}

impl DeepLinkHandler for MangaOrigines {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let path = match url.split(BASE_URL).nth(1) {
            Some(p) => p.trim_matches('/'),
            None => return Ok(None),
        };

        let parts: Vec<&str> = path.split('/').collect();

        match parts.as_slice() {
            ["manga", slug] => {
                // Build manga URL from slug
                let manga_url = format!("{}/manga/{}", BASE_URL, slug);
                
                let doc = Html::parse(&manga_url);
                
                if let Some(meta_div) = doc.select_first(".entry-meta, .post-meta") {
                    let title = meta_div
                        .select_first(".entry-title a, h1.entry-title a")
                        .and_then(|e| e.text())
                        .unwrap_or_default();

                    let cover = doc
                        .select_first("div.ori-card-cover img, figure.wp-post-image img")
                        .and_then(|i| i.attr("abs:src"));

                    return Ok(Some(DeepLinkResult {
                        manga: Some(Manga {
                            key: strip_base(&manga_url),
                            title,
                            cover,
                            ..Default::default()
                        }),
                        chapter: None,
                    }));
                }

                Ok(None)
            }
            ["manga", slug, "chapter", chap_num] => {
                // Build chapter URL from slug and number
                let chapter_url = format!("{}/chapitre/{}", BASE_URL, chap_num);
                
                return Ok(Some(DeepLinkResult {
                    manga: None,
                    chapter: Some(Chapter {
                        key: strip_base(&chapter_url),
                        title: format!("Chapitre {}", chap_num),
                        ..Default::default()
                    }),
                }));
            }
            _ => Ok(None),
        }
    }
}
