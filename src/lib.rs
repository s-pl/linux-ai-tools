use std::path::Path;

use once_cell::sync::Lazy;
use tiktoken_rs::CoreBPE;

pub fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
}

pub fn skip_heavy_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| matches!(n, ".git" | "node_modules" | "target" | ".venv" | "venv"))
        .unwrap_or(false)
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut idx = 0usize;
    while value >= 1024.0 && idx < UNITS.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{}{}", bytes, UNITS[idx])
    } else {
        format!("{value:.1}{}", UNITS[idx])
    }
}

pub fn to_base36(mut value: u64) -> String {
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while value > 0 {
        let idx = (value % 36) as usize;
        out.push(ALPHABET[idx] as char);
        value /= 36;
    }
    out.iter().rev().collect()
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let max = ab.len().min(bb.len());
    let mut len = 0usize;
    while len < max && ab[len] == bb[len] {
        len += 1;
    }
    if a.is_ascii() && b.is_ascii() {
        return len;
    }
    while len > 0 && !a.is_char_boundary(len) {
        len -= 1;
    }
    len
}

#[derive(Default)]
pub struct PathPacker {
    prev: String,
}

impl PathPacker {
    pub fn pack(&mut self, path: &str) -> String {
        let prefix_len = common_prefix_len(&self.prev, path);
        let suffix = &path[prefix_len..];
        let packed = format!("~{}|{}", to_base36(prefix_len as u64), suffix);
        self.prev = path.to_string();
        if packed.len() < path.len() {
            packed
        } else {
            path.to_string()
        }
    }
}

pub fn compact_text_for_ai(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for (idx, token) in input.split_whitespace().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        if let Some(rep) = mapped_token(token) {
            if token
                .chars()
                .all(|c| !c.is_ascii_alphabetic() || c.is_ascii_uppercase())
            {
                out.push_str(&rep.to_ascii_uppercase());
            } else {
                out.push_str(rep);
            }
        } else {
            out.push_str(token);
        }
    }

    compact_large_numbers(&out)
}

fn mapped_token(token: &str) -> Option<&'static str> {
    if token.eq_ignore_ascii_case("function") {
        Some("fn")
    } else if token.eq_ignore_ascii_case("directory") {
        Some("dir")
    } else if token.eq_ignore_ascii_case("process") {
        Some("proc")
    } else if token.eq_ignore_ascii_case("application") {
        Some("app")
    } else if token.eq_ignore_ascii_case("command") {
        Some("cmd")
    } else if token.eq_ignore_ascii_case("argument") {
        Some("arg")
    } else if token.eq_ignore_ascii_case("variable") {
        Some("var")
    } else if token.eq_ignore_ascii_case("string") {
        Some("str")
    } else if token.eq_ignore_ascii_case("javascript") {
        Some("js")
    } else if token.eq_ignore_ascii_case("typescript") {
        Some("ts")
    } else if token.eq_ignore_ascii_case("python") {
        Some("py")
    } else if token.eq_ignore_ascii_case("return") {
        Some("ret")
    } else if token.eq_ignore_ascii_case("error") {
        Some("err")
    } else if token.eq_ignore_ascii_case("warning") {
        Some("warn")
    } else if token.eq_ignore_ascii_case("information") {
        Some("info")
    } else if token.eq_ignore_ascii_case("configuration") {
        Some("cfg")
    } else if token.eq_ignore_ascii_case("parameter") {
        Some("param")
    } else if token.eq_ignore_ascii_case("message") {
        Some("msg")
    } else if token.eq_ignore_ascii_case("response") {
        Some("resp")
    } else {
        None
    }
}

pub fn compact_text_light(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = false;
    for ch in input.trim().chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
            continue;
        }
        prev_space = false;
        if ch == '\t' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn compact_large_numbers(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let num_str = &input[start..i];
            if num_str.len() >= 4 {
                if let Ok(v) = num_str.parse::<u64>() {
                    let encoded = to_base36(v);
                    if encoded.len() + 1 < num_str.len() {
                        out.push('#');
                        out.push_str(&encoded);
                    } else {
                        out.push_str(num_str);
                    }
                } else {
                    out.push_str(num_str);
                }
            } else {
                out.push_str(num_str);
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

pub fn truncate_for_ai(input: &str, max_chars: usize) -> String {
    if input.is_ascii() {
        if input.len() <= max_chars {
            return input.to_string();
        }
        let mut out = input[..max_chars].to_string();
        out.push_str(" ...");
        return out;
    }

    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = String::new();
    for (i, ch) in input.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.push_str(" ...");
    out
}

pub fn estimate_tokens(input: &str) -> usize {
    estimate_tokens_realistic(input)
}

fn heuristic_token_estimate(input: &str) -> usize {
    // Better than chars/4: account for words, punctuation, and symbols separately.
    let chars = input.chars().count();
    if chars == 0 {
        return 0;
    }
    let words = input.split_whitespace().count();
    let punct = input
        .chars()
        .filter(|c| c.is_ascii_punctuation() || c.is_ascii_digit())
        .count();
    let base = chars.div_ceil(5);
    base.max(words) + punct / 6
}

static TOKENIZER: Lazy<Option<CoreBPE>> = Lazy::new(|| tiktoken_rs::cl100k_base().ok());

pub fn estimate_tokens_realistic(input: &str) -> usize {
    match TOKENIZER.as_ref() {
        Some(tok) => tok.encode_ordinary(input).len(),
        None => heuristic_token_estimate(input),
    }
}

#[derive(Default)]
pub struct TextPacker {
    prev: String,
}

impl TextPacker {
    pub fn pack(&mut self, text: &str) -> String {
        let prefix_len = common_prefix_len(&self.prev, text);
        let out = if prefix_len >= 8 {
            let candidate = format!("~{}|{}", to_base36(prefix_len as u64), &text[prefix_len..]);
            if candidate.len() < text.len() {
                candidate
            } else {
                text.to_string()
            }
        } else {
            text.to_string()
        };
        self.prev.clear();
        self.prev.push_str(text);
        out
    }
}

pub fn from_base36(encoded: &str) -> u64 {
    let mut value: u64 = 0;
    for c in encoded.chars() {
        let digit = match c {
            '0'..='9' => c as u64 - '0' as u64,
            'a'..='z' => c as u64 - 'a' as u64 + 10,
            _ => return 0, // Fallback on invalid format
        };
        value = value.saturating_mul(36).saturating_add(digit);
    }
    value
}

#[derive(Default)]
pub struct PathUnpacker {
    prev: String,
}

impl PathUnpacker {
    pub fn unpack(&mut self, packed: &str) -> String {
        let out = if packed.starts_with('~') {
            if let Some(idx) = packed.find('|') {
                let prefix_len = from_base36(&packed[1..idx]) as usize;
                let suffix = &packed[idx + 1..];
                if prefix_len <= self.prev.len() {
                    format!("{}{}", &self.prev[..prefix_len], suffix)
                } else {
                    packed.to_string() // Fallback
                }
            } else {
                packed.to_string()
            }
        } else {
            packed.to_string()
        };
        self.prev = out.clone();
        out
    }
}

// TextUnpacker is functionally identical but kept for parity.
#[derive(Default)]
pub struct TextUnpacker {
    prev: String,
}

impl TextUnpacker {
    pub fn unpack(&mut self, packed: &str) -> String {
        let out = if packed.starts_with('~') {
            if let Some(idx) = packed.find('|') {
                let prefix_len = from_base36(&packed[1..idx]) as usize;
                let suffix = &packed[idx + 1..];
                if prefix_len <= self.prev.len() {
                    format!("{}{}", &self.prev[..prefix_len], suffix)
                } else {
                    packed.to_string()
                }
            } else {
                packed.to_string()
            }
        } else {
            packed.to_string()
        };
        self.prev.clear();
        self.prev.push_str(&out);
        out
    }
}

pub fn expand_text_for_ai(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'#' {
            // Find end of base36 number
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_alphanumeric() {
                j += 1;
            }
            if j > i + 1 {
                let num_str = std::str::from_utf8(&bytes[i + 1..j]).unwrap_or("");
                let decoded = from_base36(num_str);
                out.push_str(&decoded.to_string());
                i = j;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base36_roundtrip() {
        assert_eq!(to_base36(123456789), to_base36(from_base36(&to_base36(123456789))));
    }

    #[test]
    fn test_path_packer_roundtrip() {
        let mut packer = PathPacker::default();
        let mut unpacker = PathUnpacker::default();

        let p1 = "/var/log/syslog";
        let p2 = "/var/log/auth.log";
        
        let c1 = packer.pack(p1);
        let c2 = packer.pack(p2);
        
        assert_eq!(unpacker.unpack(&c1), p1);
        assert_eq!(unpacker.unpack(&c2), p2);
    }
}
