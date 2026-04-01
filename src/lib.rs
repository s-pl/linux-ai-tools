use std::path::Path;

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
    let mut len = 0usize;
    let mut ai = a.chars();
    let mut bi = b.chars();
    loop {
        match (ai.next(), bi.next()) {
            (Some(ca), Some(cb)) if ca == cb => len += ca.len_utf8(),
            _ => break,
        }
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
        self.prev = path.to_string();
        format!("{}|{}", to_base36(prefix_len as u64), suffix)
    }
}

pub fn compact_text_for_ai(input: &str) -> String {
    let collapsed = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = collapsed;
    let replacements = [
        ("function", "fn"),
        ("directory", "dir"),
        ("process", "proc"),
        ("application", "app"),
        ("command", "cmd"),
        ("argument", "arg"),
        ("variable", "var"),
        ("string", "str"),
        ("javascript", "js"),
        ("typescript", "ts"),
        ("python", "py"),
        ("return", "ret"),
        ("error", "err"),
        ("warning", "warn"),
    ];
    for (from, to) in replacements {
        out = out.replace(from, to);
        out = out.replace(&from.to_uppercase(), &to.to_uppercase());
    }
    out
}

pub fn truncate_for_ai(input: &str, max_chars: usize) -> String {
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
    input.chars().count().div_ceil(4)
}
