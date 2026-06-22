//! Pure-Rust port of Raijin Scans' page-list reader.
//!
//! The site hides chapter image urls behind an obfuscated manifest embedded in a
//! `<script>` tag (anchored by the `rjfr_` marker) plus a paginated `admin-ajax.php`
//! endpoint. This mirrors the bundled descrambler the Mihon extension runs in a
//! WebView (Keiyoushi `ReaderScriptManager.DEFAULT_SCRIPT`, parser version 1), but
//! done entirely in Rust: extract the manifest, base64-decode + un-permute its
//! config, then loop the multipart AJAX call collecting image urls.

use aidoku::{
    Page, PageContent, Result,
    alloc::{String, Vec, format, string::ToString, vec},
    imports::{html::Html, net::Request},
    prelude::*,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;

const ADMIN_AJAX: &str = "/wp-admin/admin-ajax.php";
const MAX_PAGE_REQUESTS: usize = 100;
const BOUNDARY: &str = "----AidokuRaijinScansBoundary7MA4YWxkTrZu0gW";

pub fn get_pages(base_url: &str, chapter_url: &str, html: &str) -> Result<Vec<Page>> {
    let doc = Html::parse(html)?;

    // 1. Locate the manifest script (the only one mentioning `rjfr_`).
    let mut script_opt: Option<String> = None;
    if let Some(scripts) = doc.select("script") {
        for e in scripts {
            if let Some(t) = e.data()
                && t.contains("rjfr_") {
                    script_opt = Some(t);
                    break;
                }
        }
    }
    let Some(script) = script_opt else {
        bail!("No reader manifest found. Open the chapter in the WebView.");
    };

    // 2. Extract the `{ … "m": … }` manifest object and parse it.
    let Some(m_key) = find_m_key(&script) else {
        bail!("Invalid manifest format");
    };
    let Some(start) = script[..m_key].rfind('{') else {
        bail!("Invalid manifest format");
    };
    let Some(obj) = extract_object(&script[start..]) else {
        bail!("Unterminated manifest object");
    };
    let manifest: Value = serde_json::from_str(&obj).map_err(|_| error!("Bad manifest JSON"))?;

    // 3. config = base64(join(m.split('|').map(k => c[k]))) -> JSON
    let Some(order_keys) = manifest.get("m").and_then(Value::as_str) else {
        bail!("Invalid manifest format");
    };
    let c = manifest.get("c").unwrap_or(&Value::Null);
    let b64: String = order_keys
        .split('|')
        .map(|k| c.get(k).and_then(Value::as_str).unwrap_or(""))
        .collect();
    let Some(decoded) = b64_decode(&b64) else {
        bail!("Failed to decode manifest config");
    };
    let config: Value = serde_json::from_slice(&decoded).map_err(|_| error!("Bad config JSON"))?;

    // 4. Two permutations: ordered[m[i]] = d[i]; then vals[i] = ordered[l[i]].
    let shuffled = config.get("d").and_then(Value::as_array).cloned().unwrap_or_default();
    let perm = config.get("m").and_then(Value::as_array).cloned().unwrap_or_default();
    let order = config.get("l").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut ordered: Vec<Value> = vec![Value::Null; shuffled.len()];
    for (i, p) in perm.iter().enumerate() {
        let p = p.as_u64().unwrap_or(u64::MAX) as usize;
        if p < ordered.len() && i < shuffled.len() {
            ordered[p] = shuffled[i].clone();
        }
    }
    let vals: Vec<Value> = order
        .iter()
        .map(|o| {
            let o = o.as_u64().unwrap_or(u64::MAX) as usize;
            ordered.get(o).cloned().unwrap_or(Value::Null)
        })
        .collect();

    // 5. action = the only `rjfr_` string; keyArr = vals[13]; contentValues = vals[1..=6].
    let Some(action) = vals
        .iter()
        .filter_map(Value::as_str)
        .find(|s| s.starts_with("rjfr_"))
    else {
        bail!("Reader manifest layout changed. Open the chapter in the WebView.");
    };
    let action = action.to_string();

    let key_arr: Vec<String> = vals
        .get(13)
        .and_then(Value::as_array)
        .map(|a| a.iter().map(js_string).collect())
        .unwrap_or_default();
    if key_arr.len() < 10 {
        bail!("Reader manifest layout changed. Open the chapter in the WebView.");
    }
    let content_values: Vec<String> = (1..=6).map(|i| js_string(vals.get(i).unwrap_or(&Value::Null))).collect();

    // 6. The reader root attribute is echoed back in every request.
    let rjfr_value = doc
        .select_first("[data-rj-free-reader-root]")
        .and_then(|e| e.attr("data-rj-free-reader-root"))
        .unwrap_or_default();

    // 7. Paginate the AJAX endpoint, collecting image urls until `run` flips false.
    let ajax_url = format!("{base_url}{ADMIN_AJAX}");
    let mut pages: Vec<Page> = Vec::new();
    let mut cursor = String::new();
    let mut guard = 0;

    loop {
        if guard >= MAX_PAGE_REQUESTS {
            break;
        }
        guard += 1;

        let mut fields: Vec<(&str, String)> = Vec::new();
        fields.push(("action", action.clone()));
        for (j, v) in content_values.iter().enumerate() {
            fields.push((key_arr[j].as_str(), v.clone()));
        }
        fields.push((key_arr[6].as_str(), pages.len().to_string()));
        fields.push((key_arr[7].as_str(), String::from("0")));
        fields.push((key_arr[8].as_str(), rjfr_value.clone()));
        fields.push((key_arr[9].as_str(), cursor.clone()));

        let body = build_multipart(&fields);
        let content_type = format!("multipart/form-data; boundary={BOUNDARY}");
        let resp = Request::post(&ajax_url)?
            .header("Content-Type", content_type.as_str())
            .header("Referer", chapter_url)
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Accept", "*/*")
            .header("Origin", base_url)
            .body(body)
            .string()?;

        let root: Value = serde_json::from_str(&resp).map_err(|_| error!("Reader response format changed. Open the chapter in the WebView."))?;
        let Some((payload, images)) = find_images(&root) else {
            bail!("Reader response format changed. Open the chapter in the WebView.");
        };

        for img in &images {
            if let Some(url) = image_url(img) {
                pages.push(Page {
                    content: PageContent::url(url),
                    ..Default::default()
                });
            }
        }

        // cursor = the payload's only string primitive; run = its only boolean.
        let mut next_cursor = String::new();
        let mut run = false;
        if let Some(map) = payload.as_object() {
            for v in map.values() {
                if next_cursor.is_empty()
                    && let Some(s) = v.as_str() {
                        next_cursor = s.to_string();
                    }
                if let Some(b) = v.as_bool() {
                    run = b;
                }
            }
        }
        cursor = next_cursor;
        if !run {
            break;
        }
    }

    Ok(pages)
}

/// `String(v)` semantics for a JSON value used as a form field.
fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => String::from("null"),
        other => other.to_string(),
    }
}

/// Byte index of the `"m"` key (matching `"m"\s*:`).
fn find_m_key(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut from = 0;
    while let Some(rel) = s[from..].find("\"m\"") {
        let pos = from + rel;
        let mut j = pos + 3;
        while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\n' | b'\r') {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b':' {
            return Some(pos);
        }
        from = pos + 3;
    }
    None
}

/// Brace-match a JSON object starting at index 0, ignoring braces inside strings.
fn extract_object(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &ch) in bytes.iter().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if ch == b'\\' {
                esc = true;
            } else if ch == b'"' {
                in_str = false;
            }
        } else if ch == b'"' {
            in_str = true;
        } else if ch == b'{' {
            depth += 1;
        } else if ch == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(s[..=i].to_string());
            }
        }
    }
    None
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    while t.ends_with('=') {
        t.pop();
    }
    while !t.len().is_multiple_of(4) {
        t.push('=');
    }
    STANDARD.decode(t.as_bytes()).ok()
}

fn has_image_ext(s: &str) -> bool {
    let lc = s.to_lowercase();
    [".webp", ".jpg", ".jpeg", ".png", ".gif", ".avif"]
        .iter()
        .any(|ext| lc.contains(ext))
}

/// The real image url in an object = its first http(s) string with an image extension
/// (a decoy admin-ajax url without an image extension also lives in each object).
fn image_url(obj: &Value) -> Option<String> {
    let map = obj.as_object()?;
    for v in map.values() {
        if let Some(s) = v.as_str()
            && s.starts_with("http") && has_image_ext(s) {
                return Some(s.to_string());
            }
    }
    None
}

fn is_image_array(v: &Value) -> bool {
    v.as_array()
        .map(|a| !a.is_empty() && image_url(&a[0]).is_some())
        .unwrap_or(false)
}

/// Find the image-object array anywhere in the tree, with its parent object as payload.
fn find_images(el: &Value) -> Option<(Value, Vec<Value>)> {
    match el {
        Value::Object(map) => {
            for v in map.values() {
                if is_image_array(v) {
                    return Some((el.clone(), v.as_array().cloned().unwrap_or_default()));
                }
            }
            for v in map.values() {
                if let Some(r) = find_images(v) {
                    return Some(r);
                }
            }
            None
        }
        Value::Array(arr) => {
            for v in arr {
                if let Some(r) = find_images(v) {
                    return Some(r);
                }
            }
            None
        }
        _ => None,
    }
}

fn build_multipart(fields: &[(&str, String)]) -> Vec<u8> {
    let mut body = String::new();
    for (name, value) in fields {
        body.push_str(&format!("--{BOUNDARY}\r\n"));
        body.push_str(&format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n"));
        body.push_str(value);
        body.push_str("\r\n");
    }
    body.push_str(&format!("--{BOUNDARY}--\r\n"));
    body.into_bytes()
}
