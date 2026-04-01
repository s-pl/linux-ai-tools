use std::io::{self, BufRead, BufWriter, Write};
use ai_linux_tools::{from_base36, expand_text_for_ai, PathUnpacker, TextUnpacker};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = BufWriter::new(stdout.lock());

    let mut first_line = String::new();
    if reader.read_line(&mut first_line).is_err() || first_line.is_empty() {
        return;
    }
    let first_line = first_line.trim_end();

    if !first_line.starts_with("@ap1") && !first_line.starts_with("@ap2") {
        // Not packed, passthrough
        let _ = writeln!(writer, "{}", first_line);
        for line in reader.lines() {
            let _ = writeln!(writer, "{}", line.unwrap_or_default());
        }
        return;
    }

    let parts: Vec<&str> = first_line.split('\t').collect();
    let mut fields: Vec<&str> = Vec::new();
    if parts.len() >= 3 && parts[2].starts_with("fields=") {
        fields = parts[2]["fields=".len()..].split(',').collect();
    }

    let mut path_unpacker = PathUnpacker::default();
    let mut text_unpacker = TextUnpacker::default();

    for line in reader.lines() {
        let line = line.unwrap_or_default();
        let cols: Vec<&str> = line.split('\t').collect();
        let mut out_cols = Vec::with_capacity(cols.len());

        for (i, &col) in cols.iter().enumerate() {
            let field_type = fields.get(i).unwrap_or(&"");
            let decoded = match *field_type {
                "p36" | "pp36" | "s36" | "t36" | "l36" | "r36" | "v36" => {
                    from_base36(col).to_string()
                }
                "pd" => path_unpacker.unpack(col),
                "txtp" | "cmdp" => {
                    let text = text_unpacker.unpack(col);
                    expand_text_for_ai(&text)
                },
                _ => col.to_string(), // k, st, n, cmdc, sh, u, g, l
            };
            out_cols.push(decoded);
        }
        let _ = writeln!(writer, "{}", out_cols.join("\t"));
    }
}
