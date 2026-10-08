use aidoku::{MangaStatus, alloc::string::ToString};
use serde::Deserialize;

pub const BASE_URL: &str = "https://mangas-origines.fr";

/// Response for potential AJAX endpoints (if needed in future)
#[derive(Deserialize)]
pub struct Dto {
    pub success: bool,
}

/// Removes the scheme + host so manga/chapter keys are stored as site-relative paths.
pub fn strip_base(url: &str) -> String {
    if let Some(rest) = url.strip_prefix(BASE_URL) {
        if rest.is_empty() {
            String::from("/")
        } else {
            rest.to_string()
        }
    } else if let Some(idx) = url.find("://").map(|i| i + 3) {
        match url[idx..].find('/') {
            Some(slash) => url[idx + slash..].to_string(),
            None => String::from("/"),
        }
    } else {
        url.to_string()
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

/// Parse French relative dates to Unix timestamp.
pub fn parse_relative_date(date: Option<&str>) -> Option<i64> {
    let raw = date?;
    let lc = raw.to_lowercase();
    let now = aidoku::alloc::std::current_date();

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
