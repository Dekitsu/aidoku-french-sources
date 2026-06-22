#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
    HomeComponentValue, HomeLayout, Listing, ListingKind, ListingProvider, Manga, MangaPageResult,
    Page, Result, Source,
    alloc::{String, Vec, format, string::ToString, vec},
    imports::{
        html::{Document, Element, Html},
        net::Request,
        std::send_partial_result,
    },
    prelude::*,
};
use core::cell::RefCell;

mod models;
mod reader;

use models::{
    BASE_URL, LatestUpdatesDto, first_number, parse_relative_date, parse_status, strip_base,
};

struct RaijinScans {
    // The "Recent" AJAX pagination needs a nonce scraped from the homepage.
    nonce: RefCell<Option<String>>,
}

impl Source for RaijinScans {
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
            format!(
                "{BASE_URL}/page/{page}/?s={}&post_type=wp-manga",
                urlencode(&q)
            )
        } else {
            format!("{BASE_URL}/?s={}&post_type=wp-manga", urlencode(&q))
        };
        let doc = Request::get(&url)?.html()?;
        let entries = doc
            .select("div.original.card-lg div.unit")
            .map(|els| els.filter_map(|e| search_manga_from_element(&e)).collect())
            .unwrap_or_default();
        let has_next_page = doc
            .select_first("li.page-item:not(.disabled) a[rel=next]")
            .is_some();
        Ok(MangaPageResult {
            entries,
            has_next_page,
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
            if let Some(title) = doc.select_first("h1.serie-title").and_then(|e| e.text()) {
                manga.title = title;
            }
            manga.url = Some(url.clone());
            manga.cover = doc
                .select_first("img.cover")
                .and_then(|e| e.attr("abs:src"))
                .or(manga.cover);

            if let Some(author) = doc
                .select_first("div.stat-item:has(span:contains(Auteur)) span.stat-value")
                .and_then(|e| e.text())
                .filter(|t| !t.is_empty())
            {
                manga.authors = Some(vec![author]);
            }
            if let Some(artist) = doc
                .select_first("div.stat-item:has(span:contains(Artiste)) span.stat-value")
                .and_then(|e| e.text())
                .filter(|t| !t.is_empty())
            {
                manga.artists = Some(vec![artist]);
            }

            manga.description = parse_description(&doc);

            let tags: Vec<String> = doc
                .select("div.genre-list div.genre-link")
                .map(|els| els.filter_map(|e| e.text()).filter(|t| !t.is_empty()).collect())
                .unwrap_or_default();
            if !tags.is_empty() {
                manga.tags = Some(tags);
            }

            manga.status = parse_status(
                doc.select_first(
                    "div.stat-item:has(span:contains(État du titre)) span.manga",
                )
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

        // Premium chapters resolve to the paywall or a login redirect; surface that
        // instead of failing obscurely in the descrambler.
        let is_premium = url.contains("connexion")
            || Html::parse(html.as_bytes())
                .ok()
                .and_then(|d| d.select_first(".subscription-required-message"))
                .is_some();
        if is_premium {
            bail!("Ce chapitre est premium. Connectez-vous via la WebView pour le lire.");
        }

        reader::get_pages(BASE_URL, &url, &html)
    }
}

impl ListingProvider for RaijinScans {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        match listing.id.as_str() {
            "popular" => {
                let doc = Request::get(BASE_URL)?.html()?;
                self.store_nonce(&doc);
                let entries = doc
                    .select("section#most-viewed div.swiper-slide.unit")
                    .map(|els| els.filter_map(|e| popular_manga_from_element(&e)).collect())
                    .unwrap_or_default();
                Ok(MangaPageResult {
                    entries,
                    has_next_page: false,
                })
            }
            _ => self.latest(page),
        }
    }
}

impl Home for RaijinScans {
    fn get_home(&self) -> Result<HomeLayout> {
        let doc = Request::get(BASE_URL)?.html()?;
        self.store_nonce(&doc);

        let popular: Vec<Manga> = doc
            .select("section#most-viewed div.swiper-slide.unit")
            .map(|els| els.filter_map(|e| popular_manga_from_element(&e)).collect())
            .unwrap_or_default();
        let latest: Vec<Manga> = doc
            .select("section.recently-updated div.unit")
            .map(|els| els.filter_map(|e| search_manga_from_element(&e)).collect())
            .unwrap_or_default();

        Ok(HomeLayout {
            components: vec![
                HomeComponent {
                    title: Some(String::from("Les plus vus")),
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

impl DeepLinkHandler for RaijinScans {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        if !url.starts_with(BASE_URL) {
            return Ok(None);
        }
        let key = strip_base(&url);
        // We can't cheaply distinguish a series page from a chapter page by url
        // alone, so open everything as a manga entry (the most common shared link).
        Ok(Some(DeepLinkResult::Manga { key }))
    }
}

impl RaijinScans {
    fn store_nonce(&self, doc: &Document) {
        if let Some(data) = doc
            .select_first("script#ajax-sh-js-extra")
            .and_then(|e| e.data())
            && let Some(n) = find_nonce(&data) {
                *self.nonce.borrow_mut() = Some(n);
            }
    }

    fn latest(&self, page: i32) -> Result<MangaPageResult> {
        if page <= 1 {
            let doc = Request::get(BASE_URL)?.html()?;
            self.store_nonce(&doc);
            let entries = doc
                .select("section.recently-updated div.unit")
                .map(|els| els.filter_map(|e| search_manga_from_element(&e)).collect())
                .unwrap_or_default();
            let has_next_page = doc.select_first("a#load-more-manga").is_some();
            return Ok(MangaPageResult {
                entries,
                has_next_page,
            });
        }

        // Ensure we have a nonce (refresh from the homepage if needed).
        if self.nonce.borrow().is_none() {
            let doc = Request::get(BASE_URL)?.html()?;
            self.store_nonce(&doc);
        }
        let nonce = match self.nonce.borrow().clone() {
            Some(n) => n,
            None => bail!("Nonce introuvable. Rafraîchissez la liste « Récent »."),
        };

        let body = format!(
            "action=load_manga&page={}&nonce={}",
            page - 1,
            urlencode(&nonce)
        );
        let dto: LatestUpdatesDto = Request::post(format!("{BASE_URL}/wp-admin/admin-ajax.php"))?
            .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Accept", "*/*")
            .header("Origin", BASE_URL)
            .header("Referer", BASE_URL)
            .body(body.into_bytes())
            .json_owned()?;

        if !dto.success {
            return Ok(MangaPageResult {
                entries: Vec::new(),
                has_next_page: false,
            });
        }

        let fragment = Html::parse_fragment(dto.data.manga_html.as_bytes())?;
        let entries = fragment
            .select("div.unit")
            .map(|els| els.filter_map(|e| search_manga_from_element(&e)).collect())
            .unwrap_or_default();
        let has_next_page = dto.data.current_page < dto.data.total_pages;
        Ok(MangaPageResult {
            entries,
            has_next_page,
        })
    }
}

// ----------------------------- element parsing -----------------------------

fn popular_manga_from_element(el: &Element) -> Option<Manga> {
    let title_el = el.select_first("a.c-title")?;
    let href = title_el.attr("abs:href")?;
    Some(Manga {
        key: strip_base(&href),
        title: title_el.text()?,
        cover: el
            .select_first("a.poster div.poster-image-wrapper > img")
            .and_then(|i| i.attr("abs:src")),
        url: Some(href),
        ..Default::default()
    })
}

fn search_manga_from_element(el: &Element) -> Option<Manga> {
    let link = el.select_first("div.info > a")?;
    let href = link.attr("abs:href")?;
    Some(Manga {
        key: strip_base(&href),
        title: link.text()?,
        cover: el
            .select_first("div.poster-image-wrapper > img")
            .and_then(|i| i.attr("abs:src")),
        url: Some(href),
        ..Default::default()
    })
}

fn parse_chapters(doc: &Document, manga_url: &str) -> Vec<Chapter> {
    let Some(items) = doc.select("ul.scroll-sm li.item") else {
        return Vec::new();
    };
    items
        .filter_map(|el| {
            let premium = el.select_first("a.cairo-premium").is_some();
            let link = el.select_first("a")?;
            let title = link.attr("title").unwrap_or_default();

            let end_url = if !link.outer_html().unwrap_or_default().contains("connexion") {
                link.attr("abs:href")?
            } else {
                // Premium link points at /connexion: derive the chapter url from the
                // title's number token resolved against the manga url.
                let token = title.split(' ').nth(1).unwrap_or("");
                resolve_uri(manga_url, token)
            };

            let date = link
                .select_first("span:nth-of-type(2)")
                .and_then(|s| s.text());

            Some(Chapter {
                key: strip_base(&end_url),
                title: Some(title.clone()),
                chapter_number: first_number(&title).map(|n| n as f32),
                date_uploaded: parse_relative_date(date.as_deref()),
                url: Some(end_url),
                locked: premium,
                ..Default::default()
            })
        })
        .collect()
}

fn parse_description(doc: &Document) -> Option<String> {
    // Preferred: the description is assigned via `content.innerHTML = `…``.
    if let Some(scripts) = doc.select("script") {
        for s in scripts {
            let Some(data) = s.data() else { continue };
            if let Some(desc) = extract_between(&data, "content.innerHTML = `", "`") {
                let trimmed = desc.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    doc.select_first("div.description-content")
        .and_then(|e| e.text())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
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

fn extract_between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let s = haystack.find(start)? + start.len();
    let rest = &haystack[s..];
    let e = rest.find(end)?;
    Some(rest[..e].to_string())
}

fn resolve_uri(base: &str, reference: &str) -> String {
    if reference.contains("://") {
        return reference.to_string();
    }
    let base = base.split(['?', '#']).next().unwrap_or(base);
    match base.rfind('/') {
        Some(idx) => format!("{}/{}", &base[..idx], reference),
        None => format!("{base}/{reference}"),
    }
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

register_source!(RaijinScans, ListingProvider, Home, DeepLinkHandler);
