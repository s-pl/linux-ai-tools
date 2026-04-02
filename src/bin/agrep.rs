use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use ai_linux_tools::{
    PathPacker, TextPacker, compact_text_for_ai, compact_text_light, is_hidden, skip_heavy_dir,
    to_base36, truncate_for_ai,
};
use memchr::memmem;

// 64 KB I/O buffer: reduces read() syscalls for large files (~8x vs default 8 KB).
const IO_BUF: usize = 64 * 1024;

fn search_file(
    path: &Path,
    file_path: &str,
    needle: &str,
    // Some(finder) = case-sensitive SIMD path; None = ignore-case fallback.
    finder: Option<&memmem::Finder<'static>>,
    max_results: usize,
    pack: bool,
    aggressive: bool,
    path_packer: &mut PathPacker,
    text_packer: &mut TextPacker,
    out: &mut BufWriter<impl Write>,
    emitted: &mut usize,
) {
    let Ok(handle) = fs::File::open(path) else {
        return;
    };
    let mut reader = BufReader::with_capacity(IO_BUF, handle);
    // Quick binary guard: null byte in the first read buffer → skip file.
    if let Ok(prefix) = reader.fill_buf()
        && prefix.contains(&0)
    {
        return;
    }
    for (idx, line) in reader.lines().enumerate() {
        let Ok(line) = line else {
            continue;
        };
        // Case-sensitive: SIMD memmem scan over raw bytes.
        // Case-insensitive: lowercase the line, then plain contains.
        let matched = if let Some(f) = finder {
            f.find(line.as_bytes()).is_some()
        } else {
            line.to_lowercase().contains(needle)
        };
        if matched {
            let clean = line.replace('\t', " ");
            if pack {
                let p = path_packer.pack(file_path);
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
            *emitted += 1;
            if *emitted >= max_results {
                return;
            }
        }
    }
}

fn search_dir(
    root: &Path,
    include_hidden: bool,
    needle: &str,
    finder: Option<&memmem::Finder<'static>>,
    max_results: usize,
    pack: bool,
    aggressive: bool,
    path_packer: &mut PathPacker,
    text_packer: &mut TextPacker,
    out: &mut BufWriter<impl Write>,
    emitted: &mut usize,
) {
    if *emitted >= max_results {
        return;
    }
    let Ok(read_dir) = fs::read_dir(root) else {
        return;
    };
    let mut subdirs: Vec<PathBuf> = Vec::new();
    for entry in read_dir.flatten() {
        if *emitted >= max_results {
            return;
        }
        let path = entry.path();
        if !include_hidden && is_hidden(&path) {
            continue;
        }
        // file_type() reads d_type from the dirent struct returned by getdents64 on Linux —
        // no extra fstatat() syscall (unlike metadata()).
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            if !skip_heavy_dir(&path) {
                subdirs.push(path);
            }
        } else if file_type.is_file() {
            let file_path = path.display().to_string();
            search_file(
                &path,
                &file_path,
                needle,
                finder,
                max_results,
                pack,
                aggressive,
                path_packer,
                text_packer,
                out,
                emitted,
            );
        }
    }
    for dir in subdirs {
        if *emitted >= max_results {
            return;
        }
        search_dir(
            &dir,
            include_hidden,
            needle,
            finder,
            max_results,
            pack,
            aggressive,
            path_packer,
            text_packer,
            out,
            emitted,
        );
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

    // When ignore_case=true, needle is lowercased and we skip the SIMD finder.
    // When ignore_case=false, build a Finder<'static> (into_owned copies the needle bytes)
    // so it can be passed through recursive calls without lifetime constraints.
    let needle = if ignore_case {
        pattern.to_lowercase()
    } else {
        pattern
    };
    let finder: Option<memmem::Finder<'static>> = if ignore_case {
        None
    } else {
        Some(memmem::Finder::new(needle.as_bytes()).into_owned())
    };

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut path_packer = PathPacker::default();
    let mut text_packer = TextPacker::default();
    let mut emitted = 0usize;

    if pack {
        let _ = writeln!(out, "@ap2\tagrep\tfields=pd,l36,txtp");
    }

    search_dir(
        &root,
        include_hidden,
        &needle,
        finder.as_ref(),
        max_results,
        pack,
        aggressive,
        &mut path_packer,
        &mut text_packer,
        &mut out,
        &mut emitted,
    );
}
