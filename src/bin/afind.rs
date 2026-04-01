use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ai_linux_tools::{PathPacker, is_hidden, skip_heavy_dir};

fn search(
    root: &Path,
    needle: &str,
    needle_lower: Option<&str>,
    include_hidden: bool,
    ignore_case: bool,
    only_type: Option<char>,
    max_results: usize,
    out: &mut Vec<String>,
) {
    if out.len() >= max_results {
        return;
    }

    let Ok(read_dir) = fs::read_dir(root) else {
        return;
    };
    for entry in read_dir.flatten() {
        if out.len() >= max_results {
            break;
        }
        let path = entry.path();
        if !include_hidden && is_hidden(&path) {
            continue;
        }
        let Ok(md) = entry.metadata() else {
            continue;
        };
        let is_dir = md.is_dir();
        if is_dir && skip_heavy_dir(&path) {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        let matched = if ignore_case {
            name
                .to_lowercase()
                .contains(needle_lower.unwrap_or_default())
        } else {
            name.contains(needle)
        };

        let type_ok = match only_type {
            Some('f') => md.is_file(),
            Some('d') => is_dir,
            _ => true,
        };

        if matched && type_ok {
            out.push(path.display().to_string());
            if out.len() >= max_results {
                break;
            }
        }

        if is_dir {
            search(
                &path,
                needle,
                needle_lower,
                include_hidden,
                ignore_case,
                only_type,
                max_results,
                out,
            );
        }
    }
}

fn main() {
    let mut include_hidden = false;
    let mut ignore_case = false;
    let mut pack = false;
    let mut only_type: Option<char> = None;
    let mut max_results: usize = 500;
    let mut free = Vec::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--hidden" => include_hidden = true,
            "-i" | "--ignore-case" => ignore_case = true,
            "--pack" => pack = true,
            "--type" => {
                if let Some(v) = args.next() {
                    if v == "f" || v == "d" {
                        only_type = v.chars().next();
                    }
                }
            }
            "--max" => {
                if let Some(v) = args.next() {
                    max_results = v.parse().unwrap_or(500);
                }
            }
            _ => free.push(arg),
        }
    }

    if free.is_empty() {
        eprintln!("uso: afind [--hidden] [-i] [--type f|d] [--max N] <patron> [ruta]");
        std::process::exit(2);
    }

    let needle = free[0].clone();
    let needle_lower = if ignore_case {
        Some(needle.to_lowercase())
    } else {
        None
    };
    let root = free
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut results = Vec::new();
    search(
        &root,
        &needle,
        needle_lower.as_deref(),
        include_hidden,
        ignore_case,
        only_type,
        max_results,
        &mut results,
    );

    results.sort();
    if pack {
        let mut path_packer = PathPacker::default();
        let root_prefix = format!("{}/", root.display());
        for path in results.into_iter().take(max_results) {
            let rel = path.strip_prefix(&root_prefix).unwrap_or(&path).to_string();
            println!("{}", path_packer.pack(&rel));
        }
    } else {
        for path in results.into_iter().take(max_results) {
            println!("{}", path);
        }
    }
}
