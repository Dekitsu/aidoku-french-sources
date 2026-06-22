use aidoku::{
    MangaStatus,
    alloc::{String, string::ToString},
    imports::std::current_date,
};
use serde::Deserialize;

pub const BASE_URL: &str = "https://raijin-scans.fr";

/// Response of the `load_manga` admin-ajax action used to paginate "Recent".
#[derive(Deserialize)]
pub struct LatestUpdatesDto {
    pub success: bool,
    pub data: LatestUpdatesDataDto,
}

#[derive(Deserialize)]
pub struct LatestUpdatesDataDto {
    #[serde(rename = "manga_html")]
    pub manga_html: String,
    #[serde(rename = "current_page")]
    pub current_page: i32,
    #[serde(rename = "total_pages")]
    pub total_pages: i32,
}

/// Removes the scheme + host so manga/chapter keys are stored as site-relative
/// paths (e.g. `/manga/foo`), mirroring Mihon's `setUrlWithoutDomain`.
pub fn strip_base(url: &str) -> String {
    if let Some(rest) = url.strip_prefix(BASE_URL) {
        if rest.is_empty() {
            String::from("/")
        } else {
            rest.to_string()
        }
    } else if let Some(idx) = url.find("://").map(|i| i + 3) {
        // Fallback for an unexpected host: drop scheme://host, keep the path.
        match url[idx..].find('/') {
            Some(slash) => url[idx + slash..].to_string(),
            None => String::from("/"),
        }
    } else {
        url.to_string()
    }
}

pub fn parse_status(status: Option<&str>) -> MangaStatus {
    let lc = match status {
        Some(s) => s.to_lowercase(),
        None => return MangaStatus::Unknown,
    };
    if lc.contains("en cours") {
        MangaStatus::Ongoing
    } else if lc.contains("terminé") || lc.contains("termine") {
        MangaStatus::Completed
    } else {
        MangaStatus::Unknown
    }
}

/// First run of ASCII digits in `s`, parsed as an integer.
pub fn first_number(s: &str) -> Option<i32> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    s[start..i].parse::<i32>().ok()
}

/// French relative dates ("aujourd'hui", "il y a 3 jours", "2h", "5 min", …)
/// resolved against the current time. Returns a Unix timestamp in seconds.
pub fn parse_relative_date(date: Option<&str>) -> Option<i64> {
    let raw = date?;
    let lc = raw.to_lowercase();
    let lc = lc.trim();
    let now = current_date();

    if lc.contains("aujourd") {
        return Some(now);
    }
    if lc.contains("hier") {
        return Some(now - 86_400);
    }

    let n = first_number(lc)? as i64;
    let has = |kw: &str| lc.contains(kw);

    let seconds = if (has("h") || has("heure")) && !has("chapitre") && !has("min") {
        n * 3_600
    } else if has("min") {
        n * 60
    } else if has("jour") || lc.ends_with('j') {
        n * 86_400
    } else if has("semaine") {
        n * 604_800
    } else if has("mois") || (lc.ends_with('m') && !has("min")) {
        n * 2_592_000
    } else if has("an") {
        n * 31_536_000
    } else {
        return None;
    };

    Some(now - seconds)
}
