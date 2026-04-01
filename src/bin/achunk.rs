use std::env;
use std::io::{self, BufRead, BufWriter, Write};
use tiktoken_rs::cl100k_base;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut max_tokens = 4000;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--max" && i + 1 < args.len() {
            if let Ok(v) = args[i + 1].parse() {
                max_tokens = v;
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = BufWriter::new(stdout.lock());

    let mut lines = Vec::new();
    for line in reader.lines() {
        if let Ok(l) = line {
            lines.push(l);
        }
    }

    if lines.is_empty() {
        return;
    }

    let bpe = cl100k_base().unwrap();
    let mut line_tokens = Vec::with_capacity(lines.len());
    let mut total_tokens = 0;

    for line in &lines {
        // Using approximate token count per line or exact; let's use exact
        let count = bpe.encode_ordinary(line).len() + 1; // +1 for newline
        line_tokens.push(count);
        total_tokens += count;
    }

    if total_tokens <= max_tokens {
        for line in lines {
            let _ = writeln!(writer, "{}", line);
        }
    } else {
        let half_budget = max_tokens / 2;
        
        let mut front_count = 0;
        let mut front_end = 0;
        while front_end < lines.len() && front_count + line_tokens[front_end] <= half_budget {
            front_count += line_tokens[front_end];
            front_end += 1;
        }

        let mut back_count = 0;
        let mut back_start = lines.len();
        while back_start > front_end && back_count + line_tokens[back_start - 1] <= half_budget {
            back_start -= 1;
            back_count += line_tokens[back_start];
        }

        for i in 0..front_end {
            let _ = writeln!(writer, "{}", lines[i]);
        }

        let omitted_tokens = total_tokens - front_count - back_count;
        let omitted_lines = back_start - front_end;
        let _ = writeln!(
            writer,
            "\n... [ {} lines omitted ({} tokens) ] ...\n",
            omitted_lines, omitted_tokens
        );

        for i in back_start..lines.len() {
            let _ = writeln!(writer, "{}", lines[i]);
        }
    }
}
