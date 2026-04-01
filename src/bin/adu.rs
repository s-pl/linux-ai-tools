use std::env;
use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use ai_linux_tools::{PathPacker, human_bytes, is_hidden, skip_heavy_dir, to_base36};

fn dir_size(path: &Path, include_hidden: bool) -> u64 {
    let Ok(read_dir) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in read_dir.flatten() {
        let p = entry.path();
        if !include_hidden && is_hidden(&p) {
            continue;
        }
        let Ok(md) = entry.metadata() else {
            continue;
        };
        if md.is_file() {
            total = total.saturating_add(md.len());
        } else if md.is_dir() {
            if skip_heavy_dir(&p) {
                continue;
            }
            total = total.saturating_add(dir_size(&p, include_hidden));
        }
    }
    total
}

fn main() {
    let mut include_hidden = false;
    let mut pack = false;
    let mut max_results: usize = 50;
    let mut root = PathBuf::from(".");

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--hidden" => include_hidden = true,
            "--pack" => pack = true,
            "--max" => {
                if let Some(v) = args.next() {
                    max_results = v.parse().unwrap_or(50);
                }
            }
            _ => root = PathBuf::from(arg),
        }
    }

    let Ok(read_dir) = fs::read_dir(&root) else {
        eprintln!("ERR\tread_dir\t{}", root.display());
        std::process::exit(1);
    };

    let mut rows: Vec<(u64, String)> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !include_hidden && is_hidden(&path) {
            continue;
        }
        let Ok(md) = entry.metadata() else {
            continue;
        };

        let size = if md.is_file() {
            md.len()
        } else if md.is_dir() {
            if skip_heavy_dir(&path) {
                continue;
            }
            dir_size(&path, include_hidden)
        } else {
            0
        };
        rows.push((size, path.display().to_string()));
    }

    rows.sort_by(|a, b| b.0.cmp(&a.0));
    let mut path_packer = PathPacker::default();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    if pack {
        writeln!(out, "@ap1\tadu\tfields=s36,sh,pd").unwrap();
    }
    for (size, path) in rows.into_iter().take(max_results) {
        if pack {
            writeln!(
                out,
                "{}\t{}\t{}",
                to_base36(size),
                human_bytes(size),
                path_packer.pack(&path)
            ).unwrap();
        } else {
            writeln!(out, "{}\t{}\t{}", size, human_bytes(size), path).unwrap();
        }
    }
    out.flush().unwrap();
}
