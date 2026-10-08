#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
    HomeComponentValue, HomeLayout, Listing, ListingKind, ListingProvider, Manga, MangaPageResult,
    Page, Result, Source,
    alloc::{String, Vec, format, string::ToString, vec},
    imports::{
        html::{Document, Element, Html},
        net::Request,
    },
    prelude::*,
};
use core::cell::RefCell;

mod models;
mod reader;

use models::{BASE_URL, first_number, parse_relative_date, strip_base};
use reader::get_pages;

struct MangasOrigines {
    nonce: RefCell<Option<String>>,
}

impl Source for MangasOrigines {
    fn new() -> Self {
        Self {
            nonce: RefCell::new(None),
        }
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let q = query.unwrap_or_default();
        let url = if page > 1 {
            format!("{BASE_URL}/page/{page}/?s={}&post_type=wp-manga", urlencode(&q))
        } else {
            format!("{BASE_URL}/?s={}&post_type=wp-manga", urlencode(&q))
        };

        let doc = Request::get(&url)?.html()?;
        self.store_nonce(&doc);

        // Search results use .ori-card within .entry-content
        let entries = doc
            .select("div.entry-content div.ori-card")
            .map(|els| els.filter_map(|e| search_manga_from_element(&e)).collect())
            .unwrap_or_default();

        Ok(MangaPageResult {
            entries,
            has_next_page: false,
        })
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let url = format!("{BASE_URL}{}", manga.key);
        let doc = Request::get(&url)?.html()?;

        if needs_details {
            // Title - .ori-sr-title is used for manga details
            if let Some(title) = doc.select_first("div.post-title h1.ori-sr-title")
                .and_then(|e| e.text())
            {
                manga.title = title;
            }

            manga.url = Some(url.clone());

            // Cover image - figure.wp-post-image or div.summary_image
            manga.cover = doc
                .select_first("figure.wp-post-image img, div.summary_image img")
                .and_then(|e| e.attr("abs:src"))
                .or(manga.cover);

            // Authors - in .ori-sr-signature
            if let Some(author) = doc
                .select("div.ori-sr-signature a[href*='/manga-auteurs/']")
                .map(|els| els.filter_map(|e| {
                    e.text().filter(|t| !t.is_empty())
                }).collect::<Vec<_>>())
                .flatten()
                .next()
            {
                manga.authors = Some(vec![author]);
            }

            // Description - in .item-summary or .entry-content
            manga.description = parse_description(&doc);

            // Tags (genres) - in .ori-sr-genres
            let tags: Vec<String> = doc
                .select("div.ori-sr-genres a, div.genres-content a")
                .map(|els| els.filter_map(|e| e.text()).filter(|t| !t.is_empty()).collect())
                .unwrap_or_default();
            if !tags.is_empty() {
                manga.tags = Some(tags);
            }

            // Status - from class attributes (wp-manga-status-xxx) or explicit span
            manga.status = parse_status(
                doc.select_first("div.ori-sr-status, div.manga-status")
                    .and_then(|e| e.text())
                    .as_deref(),
            );

            if needs_chapters {
                send_partial_result(&manga);
            }
        }

        if needs_chapters {
            manga.chapters = Some(parse_chapters(&doc, &url));
        }

        Ok(manga)
    }

    fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let url = format!("{BASE_URL}{}", chapter.key);
        let html = Request::get(&url)?.string()?;

        // Check for premium/paywall
        let is_premium = html.contains("abonnement")
            || html.contains("premium")
            || Html::parse(html.as_bytes())
                .ok()
                .and_then(|d| d.select_first(".subscription-required, .paywall"))
                .is_some();

        if is_premium {
            bail!("Ce chapitre est premium.");
        }

        get_pages(BASE_URL, &url, &html)
    }
}

impl ListingProvider for MangasOrigines {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        match listing.id.as_str() {
            "popular" | "trending" | "most_viewed" => {
                let doc = Request::get(BASE_URL)?.html()?;
                self.store_nonce(&doc);

                // This site uses Vue.js components, so we need to check the catalogues page
                // or use search with orderby=trending
                let url = format!("{BASE_URL}/catalogues/?m_orderby=trending");
                let doc = Request::get(&url)?.html()?;

                // Manga cards in trending list: div.ori-card inside section#most-viewed or similar
                let entries = doc
                    .select("section#most-viewed div.ori-card, \
                              section#tendances div.ori-card")
                    .map(|els| els.filter_map(|e| popular_manga_from_element(&e)).collect())
                    .unwrap_or_default();

                Ok(MangaPageResult {
                    entries,
                    has_next_page: false,
                })
            }
            "latest" | "recent" => self.latest(page),
            _ => self.latest(page),
        }
    }
}

impl Home for MangasOrigines {
    fn get_home(&self) -> Result<HomeLayout> {
        let doc = Request::get(BASE_URL)?.html()?;
        self.store_nonce(&doc);

        // Popular / Trending (swiper rail with Vue.js)
        let popular: Vec<Manga> = doc
            .select("section#most-viewed div.ori-card, \
                      section#tendances div.ori-card")
            .map(|els| els.filter_map(|e| popular_manga_from_element(&e)).collect())
            .unwrap_or_default();

        // Recent updates - this site may not have a dedicated "recent" listing on homepage
        // Fall back to search or use the catalogues page
        let latest: Vec<Manga> = doc
            .select("div.entry-content div.ori-card")
            .map(|els| els.filter_map(|e| search_manga_from_element(&e)).collect())
            .unwrap_or_default();

        Ok(HomeLayout {
            components: vec![
                HomeComponent {
                    title: Some(String::from("Populaires")),
                    subtitle: None,
                    value: HomeComponentValue::BigScroller {
                        entries: popular,
                        auto_scroll_interval: Some(6.0),
                    },
                },
                HomeComponent {
                    title: Some(String::from("Dernières sorties")),
                    subtitle: None,
                    value: HomeComponentValue::MangaList {
                        ranking: false,
                        page_size: None,
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

impl DeepLinkHandler for MangasOrigines {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        if !url.starts_with(BASE_URL) {
            return Ok(None);
        }
        let key = strip_base(&url);
        Ok(Some(DeepLinkResult::Manga { key }))
    }
}

impl MangasOrigines {
    fn store_nonce(&self, _doc: &Document) {
        // This site uses Vue.js for dynamic content; nonce extraction may not work
        // but we keep it for potential AJAX requests in the future
    }

    fn latest(&self, page: i32) -> Result<MangaPageResult> {
        if page <= 1 {
            let doc = Request::get(BASE_URL)?.html()?;
            self.store_nonce(&doc);

            // This site's homepage may not show recent manga listings
            // We'll use the catalogues page or search as fallback
            let entries: Vec<Manga> = vec![];

            return Ok(MangaPageResult {
                entries,
                has_next_page: false,
            });
        }

        // For pagination, we need to fetch a specific listing page
        // This site may not have a dedicated "recent" listing endpoint
        bail!("Dernières sorties non disponibles pour cette source.");
    }
}

// ----------------------------- element parsing -----------------------------

fn popular_manga_from_element(el: &Element) -> Option<Manga> {
    let title_el = el.select_first("span.ori-card-title, div.ori-card-title a")?;
    let href = title_el.attr("abs:href")?;

    Some(Manga {
        key: strip_base(&href),
        title: title_el.text()?,
        cover: el
            .select_first("div.ori-card-cover img, span.ori-card-cover img")
            .and_then(|i| i.attr("abs:src")),
        url: Some(href),
        ..Default::default()
    })
}

fn search_manga_from_element(el: &Element) -> Option<Manga> {
    let link = el.select_first("div.ori-card a")?;
    let href = link.attr("abs:href")?;

    Some(Manga {
        key: strip_base(&href),
        title: link.text()?,
        cover: el
            .select_first("div.ori-card-cover img, figure.wp-post-image img")
            .and_then(|i| i.attr("abs:src")),
        url: Some(href),
        ..Default::default()
    })
}

fn parse_chapters(doc: &Document, manga_url: &str) -> Vec<Chapter> {
    // Chapters are inside .ori-chl-liste div
    let chapters = doc
        .select("div.ori-chl-liste div.ori-chl-row")
        .filter_map(|el| {
            let link = el.select_first(".ori-chl-corps, a[href*='/chapitre/']")?;
            let href = link.attr("abs:href")?;

            // Get chapter number from data-num or title text
            let num_str = el
                .select_first(".ori-chl-num b, span.ori-chl-nom-court")
                .and_then(|e| e.text());

            let date = el
                .select_first("span.ori-chl-date")
                .and_then(|s| s.text());

            Some(Chapter {
                key: strip_base(&href),
                title: Some(link.text().clone()),
                chapter_number: first_number(num_str.as_deref()).map(|n| n as f32),
                date_uploaded: parse_relative_date(date.as_deref()),
                url: Some(href),
                ..Default::default()
            })
        })
        .collect();

    chapters
}

fn parse_description(doc: &Document) -> Option<String> {
    // Try multiple selectors for description
    doc.select_first("div.item-summary div.summary__content, \
                       div.entry-content, div.post-content")
        .and_then(|e| e.text())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

fn parse_status(status: Option<&str>) -> aidoku::MangaStatus {
    let lc = match status {
        Some(s) => s.to_lowercase(),
        None => return aidoku::MangaStatus::Unknown,
    };

    if lc.contains("en cours") || lc.contains("continu") || lc.contains("active") || lc.contains("published") {
        aidoku::MangaStatus::Ongoing
    } else if lc.contains("terminé") || lc.contains("termine") || lc.contains("fini") || lc.contains("pause") || lc.contains("completed") {
        aidoku::MangaStatus::Completed
    } else {
        aidoku::MangaStatus::Unknown
    }
}

// ------------------------------- helpers ---------------------------------

fn find_nonce(data: &str) -> Option<String> {
    let idx = data.find("\"nonce\"")?;
    let rest = &data[idx + 7..];
    let colon = rest.find(':')?;
    let after = &rest[colon + 1..];
    let open = after.find('"')?;
    let value = &after[open + 1..];
    let close = value.find('"')?;
    Some(value[..close].to_string())
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

register_source!(MangasOrigines, ListingProvider, Home, DeepLinkHandler);
