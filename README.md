# ai-linux-tools

![Rust](https://img.shields.io/badge/rust-1.75%2B-black)
![Platform](https://img.shields.io/badge/platform-linux-lightgrey)
![License](https://img.shields.io/badge/license-MIT-blue)
![Tools](https://img.shields.io/badge/tools-7-success)
![Pack Mode](https://img.shields.io/badge/pack%20mode-enabled-informational)

Rust-native replacements for common Linux commands, built for AI-oriented workflows.

Standard tools (`ls`, `cat`, `grep`, ...) were designed for humans: verbose output, relative timestamps, variable-width formatting. In LLM pipelines, that verbosity wastes context tokens and increases noise. `ai-linux-tools` produces **compact, machine-readable output** with a `--pack` mode that applies encoding transforms to reduce token payload further — without sacrificing the information an agent actually needs.

---

## Tools

| Command | Replaces | Description |
|---------|----------|-------------|
| `als`   | `ls`     | Directory listing |
| `acat`  | `cat`    | File viewer |
| `agrep` | `grep -RIn` | Recursive text search |
| `afind` | `find \| grep` | Recursive file/path search |
| `adu`   | `du -ah \| sort -rh` | Disk usage summary |
| `aps`   | `ps -eo ...` | Process listing via `/proc` |
| `atok`  | n/a | Realistic token counter (`cl100k_base`) from stdin |

---

## Output Modes

### Standard mode

Human-readable compact output. Suitable for interactive use or when readability matters.

### Packed mode (`--pack`)

Optimised for LLM context windows. Packed mode applies a series of compression transforms to minimise token count while preserving semantic content.

Most packed streams begin with a schema header so the consumer knows how to parse the output:

```
@ap*	<tool>	fields=...
```

**Transforms applied:**

- **Base-36 numeric encoding** — integers are re-encoded in base 36, reducing digit count by ~39% on average
- **Delta path encoding** — consecutive paths share a common prefix; only the changed suffix is emitted
- **Semantic text compaction** — whitespace normalisation and common abbreviations (`function → fn`, `command → cmd`, `process → proc`, …)
- **Controlled truncation** — very long lines are trimmed to a configurable limit with a `...` marker
- **Delta text packing** — repeated line prefixes are compacted with `~<prefix_len36>|<suffix>`

---

## Mathematical Model

### 1) Base-36 encoding (integers)

Any non-negative integer `n` can be represented in base 10 or base 36.
The expected digit-length ratio is:

```
len36(n) ~= len10(n) * log(10) / log(36)
```

Since `log(10)/log(36) ~= 0.64`, base36 usually needs about 64% of the digits,
which is about 36% shorter (often observed as ~38-39% depending on ranges).

Example:

```
1_000_000 (base10) -> "lfls" (base36)
7 digits -> 4 digits
```

### 2) Delta path encoding

For consecutive sorted paths `p(i-1)` and `p(i)`:

- `lcp(i) = LCP(p(i-1), p(i))`  (longest common prefix length)
- `suffix(i) = p(i)[lcp(i)..]`

Packed path:

```
pack_path(p(i)) = "~" + base36(lcp(i)) + "|" + suffix(i)
```

If packed text is not shorter than the original path, the original path is emitted.
This keeps compression adaptive and avoids regressions on short paths.

Example:

```
p(i-1) = "src/bin/acat.rs"
p(i)   = "src/bin/aps.rs"
lcp(i) = 9 ("src/bin/")
pack   = "~9|aps.rs"
```

### 3) Delta text encoding

For consecutive text lines `x(i-1)` and `x(i)`:

- `lp(i) = LCP(x(i-1), x(i))`
- `tail(i) = x(i)[lp(i)..]`

Packed line:

```
pack_text(x(i)) = "~" + base36(lp(i)) + "|" + tail(i)
```

Applied only when packed line is shorter than original line.

### 4) Semantic compaction and truncation

Text compaction applies deterministic transforms before packing:

- Whitespace normalization
- Domain abbreviations (for example `function -> fn`, `process -> proc`)
- Optional numeric compaction to base36 when it strictly shortens the token
- Truncation to a max length with `...` when configured

This can be modeled as:

```
x0 = input
x1 = normalize_spaces(x0)
x2 = abbreviate_terms(x1)
x3 = compact_numbers_if_shorter(x2)
x4 = truncate(x3, k)
```

### 5) Realistic token counting

Primary token metric uses BPE compatible with modern GPT-style models:

```
T(x) = BPE_cl100k(x)
```

Fallback (when tokenizer is unavailable):

```
T_fallback(x) ~= max(ceil(|x| / 5), words(x)) + punct(x)/6
```

Where:

- `|x|` = character count
- `words(x)` = whitespace-separated words
- `punct(x)` = punctuation and digits count

### 6) Benchmark equations

Token savings:

```
token_save(%) = 100 * (old_tokens - new_tokens) / old_tokens
```

Time savings:

```
time_save(%) = 100 * (old_time - new_time) / old_time
```

Positive values mean improvement. Negative values mean regression.

### 7) Quick verification with atok

`atok` reports realistic token counts from stdin:

```bash
cat src/bin/aps.rs | atok
target/release/acat src/bin/aps.rs --pack --max 200 | atok
```

---

## Benchmark

Heavy benchmark profile (latest): `RUNS=2 WARMUP=1 INCLUDE_HEAVY=1 ./scripts/benchmark_old_vs_new.sh`

Time is wall-clock elapsed; tokens are measured with `atok` (`cl100k_base` BPE). Values below are averages from the latest hyper-optimized run (with Buffered I/O).

| Scenario | Type | Old s | New s | Time save % | Old tokens | New tokens | Token save % |
|---|---|---:|---:|---:|---:|---:|---:|
| `ls -> als (workspace)` | workspace | 0.003000 | 0.001000 | 66.7 | 319 | 105 | 67.1 |
| `ls -> als (synthetic tree)` | synthetic | 0.004500 | 0.001000 | 77.8 | 1047 | 493 | 52.9 |
| `cat -> acat (workspace file)` | workspace | 0.001000 | 0.001000 | 0.0 | 1061 | 945 | 10.9 |
| `cat -> acat (large log)` | synthetic | 0.002000 | 0.001000 | 50.0 | 600120 | 49810 | 91.7 |
| `grep -> agrep (workspace)` | workspace | 0.002500 | 0.001000 | 60.0 | 647 | 569 | 12.1 |
| `grep -> agrep (synthetic logs)` | synthetic | 0.003000 | 0.002500 | 16.7 | 42480 | 13376 | 68.5 |
| `find\|grep -> afind (workspace)` | workspace | 0.002500 | 0.001000 | 60.0 | 51 | 47 | 7.8 |
| `find\|grep -> afind (synthetic tree)` | synthetic | 0.004000 | 0.003000 | 25.0 | 1080 | 519 | 51.9 |
| `du\|sort -> adu (workspace src)` | workspace | 0.002500 | 0.001000 | 60.0 | 96 | 48 | 50.0 |
| `du\|sort -> adu (synthetic)` | synthetic | 0.005000 | 0.003000 | 40.0 | 657 | 67 | 89.8 |
| `du\|sort -> adu (workspace full)` | workspace | 0.014500 | 0.001000 | 93.1 | 1080 | 92 | 91.5 |
| `ps -> aps (top 30)` | system | 0.021500 | 0.013500 | 37.2 | 2283 | 1310 | 42.6 |
| `ps -> aps (top 80)` | system | 0.020000 | 0.014500 | 27.5 | 6024 | 3051 | 49.4 |
| `cat -> acat (huge aggressive)` | synthetic | 0.001000 | 0.003500 | -250.0 | 600120 | 125120 | 79.2 |

_Summary: avg time save=26.0%, avg token save=54.7%_

### Trade-offs: Latency vs. Tokens

While most tools (`als`, `adu`, `afind`, `aps`) are strictly *faster* than their classical counterparts because they natively combine formatting and sorting into a single internal buffer array, some edge-case modes take slightly longer.

Notice the negative time save (`-250%`) in `cat -> acat (huge aggressive)`. This is by design.
In `--aggressive` mode over gigantic text logs:
1. The tool calculates longest-common-prefixes over every single text line to emit delta paths (`~<prefix_len>|<suffix>`).
2. It runs a semantic tokenizer replacement mapping over every whitespace split word.
3. This creates a compute-bound operation.

**Why is it worth it?**
Though CPU latency increases marginally (from `0.001s` to `0.0035s`, perfectly unnoticeable to a human), it drops the payload sent to the LLM agent from **600,000 tokens** down to **125,120 tokens** (an incredible **79.2% token reduction**).
For any LLM generation task, reading 475,000 fewer tokens saves massive amounts of compute time (API latency) and money (API billing). A 2ms local delay guarantees hundreds of milliseconds shaved off the LLM inference round-trip.

Notes:

- Token savings are consistently high in heavy text scenarios.
- Time savings depend on workload shape; aggressive compaction can trade speed for much lower token payload.
- Latest heavy run summary: average time save `24.3%`, average token save `54.1%`, failures `0/0`.
- Full generated reports are stored under `reports/benchmark/` (`latest.md` and `latest.csv`).

---

## Installation

### Prerequisites

- Linux
- Rust toolchain (`rustc` + `cargo` ≥ 1.75)

### Install from source

```bash
cargo install --path .
```

### Build without installing

```bash
cargo build --release
# Binaries are placed in ./target/release/
```

### Shell aliases (zsh)

```bash
source scripts/enable_aliases.zsh
```

To make aliases permanent, add the same line to `~/.zshrc`.

---

## Usage

### Profiles

Use these profiles to choose between latency and token reduction.

| Profile | Goal | Recommended flags | Notes |
|---|---|---|---|
| `fast` | Lowest latency | no `--pack` or small `--max` | Best for quick local inspection. Minimal transform overhead. |
| `balanced` | Good latency + token savings | `--pack --max N` | Default recommended profile for most AI/agent workflows. |
| `aggressive` | Maximum token reduction | `--pack --aggressive --max N` | Stronger compaction. Can be slower on very large text streams. |

Profile examples:

```bash
# fast
acat README.md --max 80
agrep ERROR .benchdata/logs --max 80

# balanced (recommended default)
acat README.md --pack --max 80
agrep ERROR .benchdata/logs --pack --max 200

# aggressive (max compression)
acat README.md --pack --aggressive --max 80
agrep ERROR .benchdata/logs --pack --aggressive --max 200
```

### Basic examples

```bash
als . --pack
acat src/lib.rs --pack --max 40
acat src/lib.rs --pack --aggressive --max 40
agrep TODO src --max 30 --pack
agrep ERROR .benchdata/logs --max 200 --pack --aggressive
afind config . --type f --pack
adu . --max 20 --pack
aps --max 30 --pack
cat src/bin/aps.rs | atok
```

### Command reference

| Traditional command | AI-native equivalent |
|---|---|
| `ls` | `als` |
| `ls -la --time-style=+%s .` | `als . --pack` |
| `cat` | `acat` |
| `cat src/bin/aps.rs` | `acat src/bin/aps.rs --pack --max 200` |
| `cat src/bin/aps.rs` (stronger compression) | `acat src/bin/aps.rs --pack --aggressive --max 200` |
| `grep -RIn <pattern> <path>` | `agrep <pattern> <path>` |
| `grep -RIn --binary-files=without-match use src` | `agrep use src --max 200 --pack` |
| `grep -RIn ERROR .benchdata/logs` | `agrep ERROR .benchdata/logs --max 200 --pack --aggressive` |
| `find <path> -type f \| grep <pattern>` | `afind <pattern> <path> --type f` |
| `find src -type f \| grep rs` | `afind rs src --type f --max 200 --pack` |
| `du -ah <path> \| sort -rh \| head -n N` | `adu <path> --max N` |
| `du -ah . \| sort -rh \| head -n 20` | `adu . --max 20 --pack` |
| `ps -eo pid,ppid,state,rss,comm,args --sort=-rss \| head -n N` | `aps --max N` |
| `ps -eo pid,ppid,state,rss,comm,args --sort=-rss \| head -n 30` | `aps --max 30 --pack` |

---

## Running the benchmark

Compare traditional commands against their AI-native replacements. The script reports average elapsed time, BPE-estimated token count (through `atok`), and percentage savings for each pair.

```bash
# Full run (8 iterations)
RUNS=8 scripts/benchmark_old_vs_new.sh

# Quick run (4 iterations)
RUNS=4 scripts/benchmark_old_vs_new.sh

# Heavy profile (extended scenarios)
RUNS=8 WARMUP=1 INCLUDE_HEAVY=1 CASE_TIMEOUT_SEC=90 ./scripts/benchmark_old_vs_new.sh

# Focus on one family of scenarios only
SCENARIO_FILTER=afind RUNS=8 ./scripts/benchmark_old_vs_new.sh
```

---

## License

MIT