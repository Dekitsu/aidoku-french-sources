use aidoku::alloc::{String, string::ToString};
use serde::Deserialize;

pub const BASE_URL: &str = "https://mangas-origines.fr";

#[derive(Deserialize)]
pub struct Dto {
    pub success: bool,
}

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

pub fn first_number(s: &str) -> Option<i32> {
    s.find(|c: char| c.is_ascii_digit()).and_then(|idx| {
        s[idx..].find(|c: char| !c.is_ascii_digit()).map(|end| {
            s[idx..idx + end].parse().ok()
        })
    }).flatten()
}

pub fn parse_relative_date(date_str: &str) -> Option<i64> {
    let parts: aidoku::alloc::vec::Vec<&str> = date_str.split('/').collect();
    if parts.len() >= 3 {
        if let (Ok(day), Ok(month), Ok(year)) = (parts[0].parse::<u16>(), parts[1].parse::<u16>(), parts[2].parse::<u16>()) {
            return Some((year as i64) * 10000 + (month as i64) * 100 + (day as i64));
        }
    }
    
    let parts: aidoku::alloc::vec::Vec<&str> = date_str.split('-').collect();
    if parts.len() >= 3 {
        if let (Ok(day), Ok(month), Ok(year)) = (parts[0].parse::<u16>(), parts[1].parse::<u16>(), parts[2].parse::<u16>()) {
            return Some((year as i64) * 10000 + (month as i64) * 100 + (day as i64));
        }
    }
    
    None
}

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

pub fn get_current_date() -> String {
    "2026-10-08".to_string()
}
