use aidoku::{alloc::{String, format}, MangaStatus};
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

/// Extracts the first number from a chapter title (e.g., "Chapitre 1" → 1).
pub fn first_number(s: &str) -> Option<i32> {
    s.find(|c: char| c.is_ascii_digit()).and_then(|idx| {
        s[idx..].find(|c: char| !c.is_ascii_digit()).map(|end| {
            s[idx..idx + end].parse().ok()
        })
    }).flatten()
}

/// Parses a relative date string (e.g., "10/08/26" or "10-08-26") to a date.
pub fn parse_relative_date(date_str: &str) -> Option<String> {
    // Try DD/MM/YY format first
    if let Some(year) = date_str.split('/').nth(2).and_then(|s| s.parse::<u16>().ok()) {
        return Some(format!("{year:04}-{}", date_str.split('/').next().unwrap_or("01"), date_str.split('/').nth(1).unwrap_or("01")));
    }
    
    // Try DD-MM-YY format
    if let Some(year) = date_str.split('-').nth(2).and_then(|s| s.parse::<u16>().ok()) {
        return Some(format!("{year:04}-{}", date_str.split('-').next().unwrap_or("01"), date_str.split('-').nth(1).unwrap_or("01")));
    }
    
    None
}

/// Strips the domain from a URL, keeping only the path.
pub fn strip_domain(url: &str) -> String {
    if let Some(idx) = url.find("://").map(|i| i + 3) {
        match url[idx..].find('/') {
            Some(slash) => url[idx + slash..].to_string(),
            None => String::from("/"),
        }
    } else {
        url.to_string()
    }
}

/// Gets the current date as a string in YYYY-MM-DD format.
pub fn get_current_date() -> String {
    // Use aidoku's alloc::std for time functions
    use aidoku::alloc::std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = duration.as_secs() as u64;
    
    // Calculate year, month, day manually (simplified)
    let mut year = (seconds / 31536000) as u32 + 2000;
    let remaining = (seconds % 31536000) as u32;
    
    // Rough approximation for months
    if remaining >= 31536000 { year += 1; remaining -= 31536000; }
    let mut month = ((remaining / 2592000) + 1) as u8;
    if month > 12 { month = 1; }
    let day = ((remaining % 2592000) / 86400) as u8 + 1;
    
    format!("{year:04}-{month:02}-{day:02}")
}
