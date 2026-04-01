use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use ai_linux_tools::{
    PathPacker, TextPacker, compact_text_for_ai, compact_text_light, is_hidden, skip_heavy_dir,
    to_base36, truncate_for_ai,
};

fn collect_files(root: &Path, include_hidden: bool, files: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(root) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !include_hidden && is_hidden(&path) {
            continue;
        }
        let Ok(md) = entry.metadata() else {
            continue;
        };
        if md.is_dir() {
            if skip_heavy_dir(&path) {
                continue;
            }
            collect_files(&path, include_hidden, files);
        } else if md.is_file() {
            files.push(path);
        }
    }
}

fn main() {
    let mut include_hidden = false;
    let mut ignore_case = false;
    let mut pack = false;
    let mut aggressive = false;
    let mut max_results: usize = 500;
    let mut free = Vec::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--hidden" => include_hidden = true,
            "-i" | "--ignore-case" => ignore_case = true,
            "--pack" => pack = true,
            "--aggressive" => aggressive = true,
            "--max" => {
                if let Some(v) = args.next() {
                    max_results = v.parse().unwrap_or(500);
                }
            }
            _ => free.push(arg),
        }
    }

    if free.is_empty() {
        eprintln!("uso: agrep [--hidden] [-i] [--max N] <patron> [ruta]");
        std::process::exit(2);
    }

    let pattern = free[0].clone();
    let root = free
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let needle = if ignore_case {
        pattern.to_lowercase()
    } else {
        pattern
    };

    let mut files = Vec::new();
    collect_files(&root, include_hidden, &mut files);
    let mut path_packer = PathPacker::default();
    let mut text_packer = TextPacker::default();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    if pack {
        let _ = writeln!(out, "@ap2\tagrep\tfields=pd,l36,txtp");
    }

    let mut emitted = 0usize;
    for file in files {
        if emitted >= max_results {
            continue;
        }
        let file_path = file.display().to_string();
        let Ok(handle) = fs::File::open(&file) else {
            continue;
        };
        let mut reader = BufReader::new(handle);
        // Quick binary guard without double-opening files.
        if let Ok(prefix) = reader.fill_buf()
            && prefix.contains(&0)
        {
            continue;
        }
        for (idx, line) in reader.lines().enumerate() {
            let Ok(line) = line else {
                continue;
            };
            let hay = if ignore_case {
                line.to_lowercase()
            } else {
                line.clone()
            };
            if hay.contains(&needle) {
                let clean = line.replace('\t', " ");
                if pack {
                    let p = path_packer.pack(&file_path);
                    let l = to_base36((idx + 1) as u64);
                    let compact = if aggressive {
                        compact_text_for_ai(&clean)
                    } else {
                        compact_text_light(&clean)
                    };
                    let t = truncate_for_ai(&compact, 180);
                    let packed_t = if aggressive { text_packer.pack(&t) } else { t };
                    let _ = writeln!(out, "{}\t{}\t{}", p, l, packed_t);
                } else {
                    let _ = writeln!(out, "{}\t{}\t{}", file_path, idx + 1, clean);
                }
                emitted += 1;
                if emitted >= max_results {
                    break;
                }
            }
        }
        if emitted >= max_results {
            break;
        }
    }
}
