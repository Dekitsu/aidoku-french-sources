/// Next.js RSC (React Server Components) stream parser.
///
/// RSC stream format:
///   {hex_id}:{payload}\n
///
/// Payload kinds:
///   - `T{hex_byte_len},{utf8_content}` — inline text chunk
///   - `I[...]` / `HL[...]`             — module-reference / hint (skip)
///   - any other JSON value             — parsed by bracket depth
///
/// After collecting all JsonElements from the stream we recursively walk
/// every Object/Array to find the first element that deserialises into T.

use aidoku::alloc::Vec;
use serde::de::DeserializeOwned;
use serde_json::Value;

// ── Public entry point ────────────────────────────────────────────────────────

pub fn extract_rsc<T: DeserializeOwned>(body: &str) -> Option<T> {
    let payloads = rsc_payloads(body);
    for payload in &payloads {
        if let Some(v) = find_matching(payload) {
            return Some(v);
        }
    }
    None
}

// ── Recursive search ──────────────────────────────────────────────────────────

fn find_matching<T: DeserializeOwned>(value: &Value) -> Option<T> {
    match value {
        Value::Object(_) | Value::Array(_) => {
            // Try to deserialise this node first
            if let Ok(v) = serde_json::from_value::<T>(value.clone()) {
                return Some(v);
            }
            // Recurse into children
            match value {
                Value::Object(map) => {
                    for child in map.values() {
                        if let Some(v) = find_matching::<T>(child) {
                            return Some(v);
                        }
                    }
                }
                Value::Array(arr) => {
                    for child in arr {
                        if let Some(v) = find_matching::<T>(child) {
                            return Some(v);
                        }
                    }
                }
                _ => {}
            }
            None
        }
        _ => None,
    }
}

// ── RSC stream parser ─────────────────────────────────────────────────────────

fn rsc_payloads(body: &str) -> Vec<Value> {
    let mut results = Vec::new();
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Find the `:{` separator — id is all hex digits before it
        let colon = match find_colon(bytes, pos) {
            Some(c) => c,
            None => break,
        };

        let id_bytes = &bytes[pos..colon];
        if id_bytes.is_empty() || !id_bytes.iter().all(|b| b.is_ascii_hexdigit()) {
            pos += 1;
            continue;
        }

        pos = colon + 1;
        if pos >= len {
            break;
        }

        match bytes[pos] {
            b'T' => {
                // Binary text chunk: T{hex_byte_len},{content}
                pos += 1;
                let comma = match find_byte(bytes, pos, b',') {
                    Some(c) => c,
                    None => break,
                };
                let hex_str = match core::str::from_utf8(&bytes[pos..comma]) {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let byte_len = usize::from_str_radix(hex_str, 16).unwrap_or(0);
                pos = comma + 1;

                // Advance exactly byte_len UTF-8 bytes
                let start = pos;
                let mut consumed = 0usize;
                while pos < len && consumed < byte_len {
                    let b = bytes[pos];
                    let char_bytes = if b < 0x80 {
                        1
                    } else if b < 0xE0 {
                        2
                    } else if b < 0xF0 {
                        3
                    } else {
                        4
                    };
                    consumed += char_bytes;
                    pos += char_bytes;
                }

                if let Ok(s) = core::str::from_utf8(&bytes[start..pos.min(len)]) {
                    if let Ok(v) = serde_json::from_str::<Value>(s) {
                        results.push(v);
                    }
                }
            }
            b'I' | b'H' => {
                // Module reference / hint — skip to end of line
                pos = skip_to_newline(bytes, pos);
            }
            _ => {
                // JSON chunk — parse by bracket depth
                let (maybe_val, end) = parse_json_at(body, pos);
                if let Some(v) = maybe_val {
                    results.push(v);
                }
                pos = end;
            }
        }

        // Skip newline separator
        if pos < len && bytes[pos] == b'\n' {
            pos += 1;
        }
    }

    results
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn find_colon(bytes: &[u8], from: usize) -> Option<usize> {
    for i in from..bytes.len() {
        if bytes[i] == b':' {
            return Some(i);
        }
        if bytes[i] == b'\n' {
            return None;
        }
    }
    None
}

fn find_byte(bytes: &[u8], from: usize, target: u8) -> Option<usize> {
    for i in from..bytes.len() {
        if bytes[i] == target {
            return Some(i);
        }
    }
    None
}

fn skip_to_newline(bytes: &[u8], from: usize) -> usize {
    for i in from..bytes.len() {
        if bytes[i] == b'\n' {
            return i;
        }
    }
    bytes.len()
}

/// Parse a single JSON value at `start` in `body`, returning (value, end_pos).
/// Uses bracket depth to find the end without a full JSON tokeniser.
fn parse_json_at(body: &str, start: usize) -> (Option<Value>, usize) {
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut i = start;

    while i < len {
        let b = bytes[i];
        i += 1;

        if escape {
            escape = false;
            continue;
        }
        if b == b'\\' && in_string {
            escape = true;
            continue;
        }
        if b == b'"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }

        match b {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    let slice = &body[start..i];
                    let val = serde_json::from_str::<Value>(slice).ok();
                    return (val, i);
                }
            }
            b'\n' if depth == 0 => {
                // Scalar on its own line (null, bool, number)
                let slice = body[start..i - 1].trim();
                let val = serde_json::from_str::<Value>(slice).ok();
                return (val, i);
            }
            _ => {}
        }
    }

    (None, i)
}
