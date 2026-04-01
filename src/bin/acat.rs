use std::env;
use std::fs;
use std::io::{BufRead, BufReader};

use ai_linux_tools::{compact_text_for_ai, to_base36, truncate_for_ai};

fn main() {
    let mut pack = false;
    let mut max_lines: usize = usize::MAX;
    let mut file = String::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pack" => pack = true,
            "--max" => {
                if let Some(v) = args.next() {
                    max_lines = v.parse().unwrap_or(usize::MAX);
                }
            }
            _ => file = arg,
        }
    }

    if file.is_empty() {
        eprintln!("usage: acat [--pack] [--max N] <file>");
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

    if pack {
        println!("@ap1\tacat\tfields=l36,txtc");
    }

    for (idx, line) in reader.lines().enumerate() {
        if idx >= max_lines {
            break;
        }
        let Ok(line) = line else {
            continue;
        };

        if pack {
            let l36 = to_base36((idx + 1) as u64);
            let txt = truncate_for_ai(&compact_text_for_ai(&line), 220);
            println!("{}\t{}", l36, txt);
        } else {
            println!("{}", line);
        }
    }
}
