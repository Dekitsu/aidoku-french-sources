use aidoku::MangaStatus;
use aidoku::alloc::{String, Vec};
use serde::Deserialize;

// Generic API envelope: { "success": true, "data": T }
#[derive(Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

#[derive(Deserialize)]
pub struct MangaItem {
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(rename = "coverImage")]
    pub cover_image: String,
    #[serde(rename = "type")]
    pub manga_type: Option<String>,
    pub synopsis: Option<String>,
    pub status: Option<String>,
    pub genres: Option<Vec<Genre>>,
}

#[derive(Deserialize)]
pub struct Genre {
    pub name: String,
}

#[derive(Deserialize)]
pub struct ChapterItem {
    pub id: String,
    pub number: f64,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterDetail {
    pub images: Vec<String>,
}

pub fn map_status(s: &str) -> MangaStatus {
    match s {
        "Ongoing" => MangaStatus::Ongoing,
        "Completed" => MangaStatus::Completed,
        "Hiatus" => MangaStatus::Hiatus,
        "Dropped" | "Abandoned" => MangaStatus::Abandoned,
        _ => MangaStatus::Unknown,
    }
}

// Approximate Unix timestamp from ISO 8601 ("2026-01-03T20:32:33.083Z").
// Accurate to within a day — good enough for chapter ordering.
pub fn parse_iso8601(s: &str) -> i64 {
    let b = s.as_bytes();
    if b.len() < 10 {
        return 0;
    }
    let year = dig4(&b[0..4]) as i64;
    let month = dig2(&b[5..7]) as i64;
    let day = dig2(&b[8..10]) as i64;

    let y = year - 1970;
    let leap = (y + 1) / 4;
    const MONTH_OFFSET: [i64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let doy = MONTH_OFFSET[(month.max(1).min(12) - 1) as usize] + day - 1;
    (y * 365 + leap + doy) * 86400
}

fn dig4(b: &[u8]) -> u32 {
    if b.len() < 4 {
        return 0;
    }
    ((b[0] - b'0') as u32) * 1000
        + ((b[1] - b'0') as u32) * 100
        + ((b[2] - b'0') as u32) * 10
        + (b[3] - b'0') as u32
}

fn dig2(b: &[u8]) -> u32 {
    if b.len() < 2 {
        return 0;
    }
    ((b[0] - b'0') as u32) * 10 + (b[1] - b'0') as u32
}
