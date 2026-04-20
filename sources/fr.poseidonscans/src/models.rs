use aidoku::alloc::{String, Vec};
use serde::Deserialize;

// ── Latest chapters listing ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LatestResponse {
    pub data: Vec<LatestManga>,
    pub total: Option<u32>,
}

#[derive(Deserialize)]
pub struct LatestManga {
    pub title: String,
    pub slug: String,
}

// ── Manga detail + chapter list (RSC row) ────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaPageData {
    pub manga: MangaDetail,
    pub is_premium_user: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaDetail {
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub categories: Option<Vec<Category>>,
    pub chapters: Option<Vec<ChapterItem>>,
}

#[derive(Deserialize)]
pub struct Category {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterItem {
    pub number: f32,
    pub title: Option<String>,
    pub created_at: Option<String>,
    pub is_premium: Option<bool>,
    pub is_volume: Option<bool>,
}

// ── Chapter pages (RSC row) ──────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageData {
    pub initial_data: Option<InitialData>,
}

#[derive(Deserialize)]
pub struct InitialData {
    pub images: Vec<ImageItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageItem {
    pub original_url: String,
    pub order: i32,
}

// ── Status mapping ────────────────────────────────────────────────────────────

use aidoku::MangaStatus;

pub fn map_status(s: &str) -> MangaStatus {
    match s.to_lowercase().as_str() {
        "en cours" => MangaStatus::Ongoing,
        "terminé" | "termine" => MangaStatus::Completed,
        "en pause" | "hiatus" => MangaStatus::Hiatus,
        "annulé" | "abandonne" | "annule" => MangaStatus::Cancelled,
        _ => MangaStatus::Unknown,
    }
}

// ── ISO 8601 date parser ──────────────────────────────────────────────────────

pub fn parse_iso8601(s: &str) -> i64 {
    // "2024-01-15T10:30:00.000Z" → unix seconds
    let parts: Vec<&str> = s.splitn(2, 'T').collect();
    if parts.len() < 2 {
        return 0;
    }
    let date: Vec<u32> = parts[0]
        .split('-')
        .filter_map(|p| p.parse().ok())
        .collect();
    let time_str = parts[1].trim_end_matches('Z').trim_end_matches(|c: char| c == '+' || c == '-' || c.is_ascii_digit());
    let time: Vec<u32> = time_str
        .splitn(3, ':')
        .filter_map(|p| p.parse::<f64>().ok().map(|v| v as u32))
        .collect();
    if date.len() < 3 {
        return 0;
    }
    let (y, m, d) = (date[0] as i64, date[1] as i64, date[2] as i64);
    let h = time.first().copied().unwrap_or(0) as i64;
    let mi = time.get(1).copied().unwrap_or(0) as i64;
    let se = time.get(2).copied().unwrap_or(0) as i64;
    // Days since epoch (Julian day calculation)
    let a = (14 - m) / 12;
    let yr = y + 4800 - a;
    let mo = m + 12 * a - 3;
    let jdn = d + (153 * mo + 2) / 5 + 365 * yr + yr / 4 - yr / 100 + yr / 400 - 32045;
    let unix_days = jdn - 2440588;
    unix_days * 86400 + h * 3600 + mi * 60 + se
}
