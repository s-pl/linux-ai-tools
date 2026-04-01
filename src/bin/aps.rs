use std::env;
use std::fs;

use ai_linux_tools::{TextPacker, compact_text_for_ai, to_base36, truncate_for_ai};

#[derive(Debug)]
struct ProcRow {
    pid: u32,
    ppid: u32,
    state: String,
    rss_kb: u64,
    name: String,
    cmd: String,
}

fn parse_status(pid: u32) -> Option<(u32, String, u64, String)> {
    let content = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let mut ppid = 0u32;
    let mut state = String::new();
    let mut rss_kb = 0u64;
    let mut name = String::new();
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("Name:\t") {
            name = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("State:\t") {
            state = v.split_whitespace().next().unwrap_or("?").to_string();
        } else if let Some(v) = line.strip_prefix("PPid:\t") {
            ppid = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("VmRSS:\t") {
            rss_kb = v
                .split_whitespace()
                .next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
        }
    }
    Some((ppid, state, rss_kb, name))
}

fn read_cmdline(pid: u32) -> String {
    let Ok(bytes) = fs::read(format!("/proc/{pid}/cmdline")) else {
        return String::new();
    };
    let parts: Vec<String> = bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect();
    parts.join(" ")
}

fn main() {
    let mut max_results: usize = 50;
    let mut pack = false;
    let mut filter = String::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pack" => pack = true,
            "--max" => {
                if let Some(v) = args.next() {
                    max_results = v.parse().unwrap_or(50);
                }
            }
            _ => filter = arg,
        }
    }

    let filter_lower = if filter.is_empty() {
        None
    } else {
        Some(filter.to_lowercase())
    };

    let mut rows = Vec::new();
    let Ok(proc_entries) = fs::read_dir("/proc") else {
        eprintln!("ERR\tread_proc\t/proc");
        std::process::exit(1);
    };

    for entry in proc_entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let Some((ppid, state, rss_kb, proc_name)) = parse_status(pid) else {
            continue;
        };
        let cmd = read_cmdline(pid);
        let hay = format!("{proc_name} {cmd}").to_lowercase();
        if let Some(ref needle) = filter_lower
            && !hay.contains(needle)
        {
            continue;
        }
        rows.push(ProcRow {
            pid,
            ppid,
            state,
            rss_kb,
            name: proc_name,
            cmd,
        });
    }

    rows.sort_by(|a, b| b.rss_kb.cmp(&a.rss_kb));
    let mut text_packer = TextPacker::default();
    if pack {
        println!("@ap2\taps\tfields=p36,pp36,st,r36,n,cmdp");
    }
    for row in rows.into_iter().take(max_results) {
        let cmd = row.cmd.replace('\t', " ");
        if pack {
            let compact_cmd = truncate_for_ai(&compact_text_for_ai(&cmd), 160);
            let packed_cmd = text_packer.pack(&compact_cmd);
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                to_base36(row.pid as u64),
                to_base36(row.ppid as u64),
                row.state,
                to_base36(row.rss_kb),
                row.name,
                packed_cmd
            );
        } else {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                row.pid, row.ppid, row.state, row.rss_kb, row.name, cmd
            );
        }
    }
}
