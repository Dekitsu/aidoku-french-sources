#![no_std]

use aidoku::{
    Chapter, FilterValue, Home, HomeComponent, HomeLayout, Listing, ListingKind, ListingProvider,
    Manga, MangaPageResult, Page, PageContent, Result, Source,
    alloc::{String, Vec, format, string::ToString, vec},
    imports::{net::Request, std::send_partial_result},
    prelude::*,
};

mod models;
use models::{
    ChapterItem, LatestResponse, MangaPageData, PageData, map_status, parse_iso8601,
};

const BASE_URL: &str = "https://poseidon-scans.net";
const PAGE_LIMIT: usize = 16;

fn cover_url(slug: &str) -> String {
    format!("{}/api/covers/{}.webp", BASE_URL, slug)
}

fn rsc_header(req: Request) -> Request {
    req.header("RSC", "1")
        .header("Next-Router-Prefetch", "1")
        .header(
            "User-Agent",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
        )
}

/// Parse a Next.js RSC stream and find the first row that deserialises into T.
/// RSC format: each line is `{id}:{json-value}` or `{id}:I[...]` (module ref).
fn extract_rsc<T: serde::de::DeserializeOwned>(body: &str) -> Option<T> {
    for line in body.lines() {
        // Skip module-reference lines and very short lines
        let colon = line.find(':')?;
        let json_part = &line[colon + 1..];
        if json_part.starts_with('I') || json_part.starts_with("HL") {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<T>(json_part) {
            return Some(val);
        }
    }
    None
}

fn chapter_from_item(item: ChapterItem, slug: &str) -> Chapter {
    let num_str = {
        let n = item.number;
        if n % 1.0 == 0.0 {
            format!("{}", n as i32)
        } else {
            format!("{}", n)
        }
    };
    let key = format!("{}/chapter/{}", slug, num_str);
    let date = item
        .created_at
        .as_deref()
        .map(parse_iso8601)
        .filter(|&t| t > 0);

    let is_volume = item.is_volume.unwrap_or(false);
    let base_name = if is_volume {
        format!("Volume {}", num_str)
    } else {
        format!("Chapitre {}", num_str)
    };
    let title = match item.title {
        Some(ref t) if !t.is_empty() => format!("{} - {}", base_name, t),
        _ => base_name,
    };

    Chapter {
        key,
        title: Some(title),
        chapter_number: Some(item.number),
        date_uploaded: date,
        ..Default::default()
    }
}

struct PoseidonScans;

impl Source for PoseidonScans {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        // Search uses HTML CSS selectors on /series
        let url = if let Some(ref q) = query {
            if page > 1 {
                format!("{}/series?search={}&page={}", BASE_URL, q, page)
            } else {
                format!("{}/series?search={}", BASE_URL, q)
            }
        } else if page > 1 {
            format!("{}/series?page={}", BASE_URL, page)
        } else {
            format!("{}/series", BASE_URL)
        };

        let html = Request::get(&url)?
            .header(
                "User-Agent",
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
                 AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
            )
            .html()?;

        let entries: Vec<Manga> = html
            .select("div.grid a.block.group")
            .map(|els| {
                els.filter_map(|el| {
                    let href = el.attr("abs:href")?;
                    // extract slug from /serie/{slug}
                    let slug = href.rsplit('/').next()?.to_string();
                    let title = el.select_first("h2").and_then(|e| e.text())?;
                    Some(Manga {
                        key: slug.clone(),
                        title,
                        cover: Some(cover_url(&slug)),
                        url: Some(format!("{}/serie/{}", BASE_URL, slug)),
                        ..Default::default()
                    })
                })
                .collect()
            })
            .unwrap_or_default();

        let has_next = html
            .select("nav[aria-label=Pagination] a")
            .map(|mut els| {
                els.any(|el| {
                    el.text()
                        .map(|t| t.to_lowercase().contains("suivant"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        Ok(MangaPageResult { entries, has_next_page: has_next })
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let slug = manga.key.clone();
        let url = format!("{}/serie/{}", BASE_URL, slug);

        let body = rsc_header(Request::get(&url)?).string()?;

        if let Some(page_data) = extract_rsc::<MangaPageData>(&body) {
            let detail = page_data.manga;

            if needs_details {
                manga.title = detail.title.clone();
                manga.cover = Some(cover_url(&detail.slug));
                manga.url = Some(format!("{}/serie/{}", BASE_URL, detail.slug));
                manga.description = detail.description;
                manga.status = detail
                    .status
                    .as_deref()
                    .map(map_status)
                    .unwrap_or_default();
                manga.authors = detail.author.map(|a| vec![a]);
                manga.artists = detail.artist.map(|a| vec![a]);
                manga.tags = detail.categories.map(|cats| {
                    cats.into_iter().map(|c| c.name).collect()
                });

                if needs_chapters {
                    send_partial_result(&manga);
                }
            }

            if needs_chapters {
                if let Some(chapters) = detail.chapters {
                    let mut chaps: Vec<Chapter> = chapters
                        .into_iter()
                        .map(|c| chapter_from_item(c, &slug))
                        .collect();
                    chaps.sort_by(|a, b| {
                        b.chapter_number
                            .unwrap_or(0.0)
                            .partial_cmp(&a.chapter_number.unwrap_or(0.0))
                            .unwrap_or(core::cmp::Ordering::Equal)
                    });
                    manga.chapters = Some(chaps);
                }
            }
        }

        Ok(manga)
    }

    fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        // chapter.key = "{slug}/chapter/{number}"
        let url = format!("{}/serie/{}", BASE_URL, chapter.key);
        let body = rsc_header(Request::get(&url)?).string()?;

        let pages: Vec<Page> = if let Some(data) = extract_rsc::<PageData>(&body) {
            if let Some(initial) = data.initial_data {
                let mut imgs: Vec<(i32, String)> = initial
                    .images
                    .into_iter()
                    .map(|img| {
                        let abs = if img.original_url.starts_with("http") {
                            img.original_url
                        } else {
                            format!("{}{}", BASE_URL, img.original_url)
                        };
                        (img.order, abs)
                    })
                    .collect();
                imgs.sort_by_key(|&(order, _)| order);
                imgs.into_iter()
                    .map(|(_, url)| Page {
                        content: PageContent::url(url),
                        ..Default::default()
                    })
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        Ok(pages)
    }
}

impl ListingProvider for PoseidonScans {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let url = format!(
            "{}/api/manga/lastchapters?limit={}&page={}",
            BASE_URL, PAGE_LIMIT, page
        );
        let resp: LatestResponse = Request::get(&url)?.json_owned()?;
        let has_next = resp.data.len() >= PAGE_LIMIT
            || resp.total.map(|t| (page as u32 * PAGE_LIMIT as u32) < t).unwrap_or(false);
        let entries = resp
            .data
            .into_iter()
            .map(|m| Manga {
                key: m.slug.clone(),
                title: m.title,
                cover: Some(cover_url(&m.slug)),
                url: Some(format!("{}/serie/{}", BASE_URL, m.slug)),
                ..Default::default()
            })
            .collect();
        let _ = listing;
        Ok(MangaPageResult { entries, has_next_page: has_next })
    }
}

impl Home for PoseidonScans {
    fn get_home(&self) -> Result<HomeLayout> {
        let url = format!(
            "{}/api/manga/lastchapters?limit=20&page=1",
            BASE_URL
        );
        let resp: LatestResponse = Request::get(&url)?.json_owned()?;
        let latest: Vec<Manga> = resp
            .data
            .into_iter()
            .map(|m| Manga {
                key: m.slug.clone(),
                title: m.title,
                cover: Some(cover_url(&m.slug)),
                url: Some(format!("{}/serie/{}", BASE_URL, m.slug)),
                ..Default::default()
            })
            .collect();

        Ok(HomeLayout {
            components: vec![
                HomeComponent {
                    title: Some(String::from("Dernières sorties")),
                    subtitle: None,
                    value: aidoku::HomeComponentValue::Scroller {
                        entries: latest.into_iter().map(Into::into).collect(),
                        listing: Some(Listing {
                            id: String::from("latest"),
                            name: String::from("Dernières sorties"),
                            kind: ListingKind::Default,
                        }),
                    },
                },
            ],
        })
    }
}

register_source!(PoseidonScans, Home, ListingProvider);
