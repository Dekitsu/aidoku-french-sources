use aidoku::{alloc::{String, Vec, format, string::ToString}, prelude::*};
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

/// Parses a relative date string (e.g., "10/08/26" or "10-08-26") to an integer.
pub fn parse_relative_date(date_str: &str) -> Option<i64> {
    // Try DD/MM/YY format first
    let parts: aidoku::alloc::vec::Vec<&str> = date_str.split('/').collect();
    if parts.len() >= 3 {
        if let (Ok(day), Ok(month), Ok(year)) = (parts[0].parse::<u16>(), parts[1].parse::<u16>(), parts[2].parse::<u16>()) {
            return Some((year as i64) * 10000 + (month as i64) * 100 + (day as i64));
        }
    }
    
    // Try DD-MM-YY format
    let parts: aidoku::alloc::vec::Vec<&str> = date_str.split('-').collect();
    if parts.len() >= 3 {
        if let (Ok(day), Ok(month), Ok(year)) = (parts[0].parse::<u16>(), parts[1].parse::<u16>(), parts[2].parse::<u16>()) {
            return Some((year as i64) * 10000 + (month as i64) * 100 + (day as i64));
        }
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
    // Return a placeholder date - actual implementation would need std::time
    // For no_std, we'll use a fixed or configurable approach
    "2026-10-08".to_string()
}
