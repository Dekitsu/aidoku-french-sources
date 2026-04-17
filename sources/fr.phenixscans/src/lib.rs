#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeLayout,
    Listing, ListingKind, ListingProvider, Manga, MangaPageResult, MangaWithChapter, Page,
    PageContent, Result, Source,
    alloc::{String, Vec, format, string::ToString, vec},
    imports::{net::Request, std::send_partial_result},
    prelude::*,
};

mod models;
use models::{ApiResponse, ChapterDetail, ChapterItem, MangaItem, map_status, parse_iso8601};

const API_URL: &str = "https://api.phenix-scans.co";
const SITE_URL: &str = "https://phenix-scans.co";
const PAGE_LIMIT: usize = 24;

// The manga key encodes both the API id and the slug: "{id}|{slug}".
// id  → needed for GET /api/manga/{id}/chapters
// slug → needed for GET /api/manga/{slug} (details) and for the site URL
fn make_key(id: &str, slug: &str) -> String {
    format!("{}|{}", id, slug)
}

fn key_id(key: &str) -> &str {
    key.split('|').next().unwrap_or(key)
}

fn key_slug(key: &str) -> &str {
    key.splitn(2, '|').nth(1).unwrap_or(key)
}

fn manga_from_item(item: MangaItem) -> Manga {
    let cover = format!("{}/{}", API_URL, item.cover_image);
    let url = format!("{}/manga/{}", SITE_URL, item.slug);
    let key = make_key(&item.id, &item.slug);

    let tags: Option<Vec<String>> = {
        let mut t: Vec<String> = item
            .genres
            .unwrap_or_default()
            .into_iter()
            .map(|g| g.name)
            .collect();
        if let Some(mt) = item.manga_type {
            if !mt.is_empty() {
                t.push(mt);
            }
        }
        if t.is_empty() { None } else { Some(t) }
    };

    Manga {
        key,
        title: item.title,
        cover: Some(cover),
        url: Some(url),
        description: item.synopsis,
        status: item.status.as_deref().map(map_status).unwrap_or_default(),
        tags,
        ..Default::default()
    }
}

fn chapter_from_item(item: ChapterItem) -> Chapter {
    let date = item
        .created_at
        .as_deref()
        .map(parse_iso8601)
        .filter(|&t| t > 0);

    Chapter {
        key: item.id,
        chapter_number: Some(item.number as f32),
        date_uploaded: date,
        ..Default::default()
    }
}

struct PhenixScans;

impl Source for PhenixScans {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let url = if let Some(ref q) = query {
            format!(
                "{}/api/manga?limit={}&page={}&search={}",
                API_URL, PAGE_LIMIT, page, q
            )
        } else {
            format!(
                "{}/api/manga?limit={}&page={}&sort=updatedAt",
                API_URL, PAGE_LIMIT, page
            )
        };

        let resp: ApiResponse<Vec<MangaItem>> = Request::get(&url)?.json_owned()?;
        let has_next = resp.data.len() >= PAGE_LIMIT;
        let entries = resp.data.into_iter().map(manga_from_item).collect();

        Ok(MangaPageResult { entries, has_next_page: has_next })
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let slug = key_slug(&manga.key).to_string();
        let id = key_id(&manga.key).to_string();

        if needs_details {
            let url = format!("{}/api/manga/{}", API_URL, slug);
            let resp: ApiResponse<MangaItem> = Request::get(&url)?.json_owned()?;
            let item = resp.data;

            manga.title = item.title;
            manga.cover = Some(format!("{}/{}", API_URL, item.cover_image));
            manga.url = Some(format!("{}/manga/{}", SITE_URL, item.slug));
            manga.description = item.synopsis;
            manga.status = item.status.as_deref().map(map_status).unwrap_or_default();

            let mut tags: Vec<String> = item
                .genres
                .unwrap_or_default()
                .into_iter()
                .map(|g| g.name)
                .collect();
            if let Some(mt) = item.manga_type {
                if !mt.is_empty() {
                    tags.push(mt);
                }
            }
            manga.tags = if tags.is_empty() { None } else { Some(tags) };

            if needs_chapters {
                send_partial_result(&manga);
            }
        }

        if needs_chapters {
            let url = format!("{}/api/manga/{}/chapters?limit=2000", API_URL, id);
            let resp: ApiResponse<Vec<ChapterItem>> = Request::get(&url)?.json_owned()?;
            let mut chapters: Vec<Chapter> =
                resp.data.into_iter().map(chapter_from_item).collect();
            // Sort descending by chapter number (newest first)
            chapters.sort_by(|a, b| {
                b.chapter_number
                    .unwrap_or(0.0)
                    .partial_cmp(&a.chapter_number.unwrap_or(0.0))
                    .unwrap_or(core::cmp::Ordering::Equal)
            });
            manga.chapters = Some(chapters);
        }

        Ok(manga)
    }

    fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let url = format!("{}/api/chapter/{}", API_URL, chapter.key);
        let resp: ApiResponse<ChapterDetail> = Request::get(&url)?.json_owned()?;

        Ok(resp
            .data
            .images
            .into_iter()
            .map(|path| Page {
                content: PageContent::url(format!("{}/{}", API_URL, path)),
                ..Default::default()
            })
            .collect())
    }
}

impl ListingProvider for PhenixScans {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let sort = match listing.id.as_str() {
            "popular" => "popularity",
            "trending" => "trending",
            _ => "updatedAt",
        };
        let url = format!(
            "{}/api/manga?limit={}&page={}&sort={}",
            API_URL, PAGE_LIMIT, page, sort
        );
        let resp: ApiResponse<Vec<MangaItem>> = Request::get(&url)?.json_owned()?;
        let has_next = resp.data.len() >= PAGE_LIMIT;
        let entries = resp.data.into_iter().map(manga_from_item).collect();
        Ok(MangaPageResult { entries, has_next_page: has_next })
    }
}

impl Home for PhenixScans {
    fn get_home(&self) -> Result<HomeLayout> {
        let latest_url = format!(
            "{}/api/manga?limit=20&page=1&sort=updatedAt",
            API_URL
        );
        let popular_url = format!(
            "{}/api/manga?limit=20&page=1&sort=popularity",
            API_URL
        );
        let trending_url = format!(
            "{}/api/manga?limit=20&page=1&sort=trending",
            API_URL
        );

        let latest_resp: ApiResponse<Vec<MangaItem>> =
            Request::get(&latest_url)?.json_owned()?;
        let popular_resp: ApiResponse<Vec<MangaItem>> =
            Request::get(&popular_url)?.json_owned()?;
        let trending_resp: ApiResponse<Vec<MangaItem>> =
            Request::get(&trending_url)?.json_owned()?;

        let latest: Vec<Manga> = latest_resp.data.into_iter().map(manga_from_item).collect();
        let popular: Vec<Manga> = popular_resp.data.into_iter().map(manga_from_item).collect();
        let trending: Vec<Manga> = trending_resp.data.into_iter().map(manga_from_item).collect();

        // Latest releases with chapter info — we only have updatedAt on the manga itself,
        // not per-chapter, so display as a manga scroller with subtitle
        let latest_with_chapter: Vec<MangaWithChapter> = latest
            .iter()
            .map(|m| MangaWithChapter {
                manga: m.clone(),
                chapter: Chapter {
                    key: String::new(),
                    ..Default::default()
                },
            })
            .collect();

        Ok(HomeLayout {
            components: vec![
                HomeComponent {
                    title: Some(String::from("Tendances")),
                    subtitle: None,
                    value: aidoku::HomeComponentValue::BigScroller {
                        entries: trending,
                        auto_scroll_interval: Some(6.0),
                    },
                },
                HomeComponent {
                    title: Some(String::from("Dernières sorties")),
                    subtitle: None,
                    value: aidoku::HomeComponentValue::MangaChapterList {
                        page_size: None,
                        entries: latest_with_chapter,
                        listing: Some(Listing {
                            id: String::from("latest"),
                            name: String::from("Dernières sorties"),
                            kind: ListingKind::Default,
                        }),
                    },
                },
                HomeComponent {
                    title: Some(String::from("Populaire")),
                    subtitle: None,
                    value: aidoku::HomeComponentValue::Scroller {
                        entries: popular.into_iter().map(Into::into).collect(),
                        listing: Some(Listing {
                            id: String::from("popular"),
                            name: String::from("Populaire"),
                            kind: ListingKind::Default,
                        }),
                    },
                },
            ],
        })
    }
}

impl DeepLinkHandler for PhenixScans {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        // https://phenix-scans.co/manga/{slug}
        // https://phenix-scans.co/manga/{slug}/chapter/{number}
        let path = match url.split(SITE_URL).nth(1) {
            Some(p) => p.trim_matches('/'),
            None => return Ok(None),
        };

        let parts: Vec<&str> = path.split('/').collect();

        match parts.as_slice() {
            ["manga", slug] => {
                // We only have the slug — fetch the id to build a proper key
                let resp: ApiResponse<MangaItem> =
                    Request::get(&format!("{}/api/manga/{}", API_URL, slug))?.json_owned()?;
                let key = make_key(&resp.data.id, &resp.data.slug);
                Ok(Some(DeepLinkResult::Manga { key }))
            }
            ["manga", slug, "chapter", number] => {
                let resp: ApiResponse<MangaItem> =
                    Request::get(&format!("{}/api/manga/{}", API_URL, slug))?.json_owned()?;
                let manga_key = make_key(&resp.data.id, &resp.data.slug);
                // Fetch chapters to find the one matching this number
                let chap_url = format!("{}/api/manga/{}/chapters", API_URL, resp.data.id);
                let chap_resp: ApiResponse<Vec<ChapterItem>> =
                    Request::get(&chap_url)?.json_owned()?;
                let chapter_key = chap_resp
                    .data
                    .iter()
                    .find(|c| {
                        let n = c.number as u32;
                        number.parse::<u32>().map(|num| num == n).unwrap_or(false)
                    })
                    .map(|c| c.id.clone())
                    .unwrap_or_else(|| number.to_string());
                Ok(Some(DeepLinkResult::Chapter {
                    manga_key,
                    key: chapter_key,
                }))
            }
            _ => Ok(None),
        }
    }
}

register_source!(PhenixScans, Home, ListingProvider, DeepLinkHandler);
