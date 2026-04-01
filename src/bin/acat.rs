use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

use ai_linux_tools::{TextPacker, compact_text_for_ai, compact_text_light, truncate_for_ai};

fn main() {
    let mut pack = false;
    let mut aggressive = false;
    let mut max_lines: usize = usize::MAX;
    let mut file = String::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pack" => pack = true,
            "--aggressive" => aggressive = true,
            "--max" => {
                if let Some(v) = args.next() {
                    max_lines = v.parse().unwrap_or(usize::MAX);
                }
            }
            _ => file = arg,
        }
    }

    if file.is_empty() {
        eprintln!("usage: acat [--pack] [--aggressive] [--max N] <file>");
        std::process::exit(2);
    }

    let handle = match fs::File::open(&file) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("ERR\tread_file\t{}\t{}", file, err);
            std::process::exit(1);
        }
    };
    let reader = BufReader::new(handle);
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let mut text_packer = TextPacker::default();
    let use_delta_text = aggressive && max_lines <= 1500;
    if pack {
        let _ = writeln!(out, "@ap2\tacat\tfields=txtp");
    }

    for (idx, line) in reader.lines().enumerate() {
        if idx >= max_lines {
            break;
        }
        let Ok(line) = line else {
            continue;
        };

        if pack {
            let base = if aggressive {
                compact_text_for_ai(&line)
            } else {
                compact_text_light(&line)
            };
            let txt = truncate_for_ai(&base, 220);
            if txt.is_empty() {
                continue;
            }
            let packed = if use_delta_text {
                text_packer.pack(&txt)
            } else {
                txt
            };
            let _ = writeln!(out, "{}", packed);
        } else {
            let _ = writeln!(out, "{}", line);
        }
    }
}
