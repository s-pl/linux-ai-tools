use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use ai_linux_tools::{is_hidden, to_base36};

#[derive(Debug)]
struct Entry {
    name: String,
    kind: char,
    size: u64,
    mtime: u64,
}

fn parse_args() -> (PathBuf, bool, bool) {
    let mut path = PathBuf::from(".");
    let mut all = false;
    let mut pack = false;
    for arg in env::args().skip(1) {
        if arg == "-a" || arg == "--all" {
            all = true;
        } else if arg == "--pack" {
            pack = true;
        } else {
            path = PathBuf::from(arg);
        }
    }
    (path, all, pack)
}

fn kind_from_metadata(md: &fs::Metadata) -> char {
    let ft = md.file_type();
    if ft.is_dir() {
        'd'
    } else if ft.is_file() {
        'f'
    } else if ft.is_symlink() {
        'l'
    } else {
        'o'
    }
}

fn main() {
    let (path, all, pack) = parse_args();
    let read_dir = match fs::read_dir(&path) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("ERR\tread_dir\t{}\t{}", path.display(), err);
            std::process::exit(1);
        }
    };

    let mut entries = Vec::new();
    for dir_entry in read_dir.flatten() {
        if !all && is_hidden(&dir_entry.path()) {
            continue;
        }
        let md = match dir_entry.metadata() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let modified = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let name = dir_entry.file_name().to_string_lossy().into_owned();
        entries.push(Entry {
            name,
            kind: kind_from_metadata(&md),
            size: md.len(),
            mtime: modified,
        });
    }

    entries.sort_by(|a, b| match (a.kind == 'd', b.kind == 'd') {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    if pack {
        println!("@ap1\tals\tfields=k,s36,t36,n");
    }

    for e in entries {
        if pack {
            println!(
                "{}\t{}\t{}\t{}",
                e.kind,
                to_base36(e.size),
                to_base36(e.mtime),
                e.name
            );
        } else {
            println!("{}\t{}\t{}\t{}", e.kind, e.size, e.mtime, e.name);
        }
    }
}
