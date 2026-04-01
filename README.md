# ai-linux-tools

![Rust](https://img.shields.io/badge/rust-1.75%2B-black)
![Platform](https://img.shields.io/badge/platform-linux-lightgrey)
![License](https://img.shields.io/badge/license-MIT-blue)
![Tools](https://img.shields.io/badge/tools-6-success)
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

---

## Output Modes

### Standard mode

Human-readable compact output. Suitable for interactive use or when readability matters.

### Packed mode (`--pack`)

Optimised for LLM context windows. Packed mode applies a series of compression transforms to minimise token count while preserving semantic content.

Each packed stream begins with a schema header so the consumer knows how to parse the output:

```
@ap1	<tool>	fields=...
```

**Transforms applied:**

- **Base-36 numeric encoding** — integers are re-encoded in base 36, reducing digit count by ~39% on average
- **Delta path encoding** — consecutive paths share a common prefix; only the changed suffix is emitted
- **Semantic text compaction** — whitespace normalisation and common abbreviations (`function → fn`, `command → cmd`, `process → proc`, …)
- **Controlled truncation** — very long lines are trimmed to a configurable limit with a `...` marker

---

## Mathematical Model

### Base-36 encoding

A base-10 integer `n` is re-expressed in base 36, reducing digit count approximately as:

```
len₃₆(n) ≈ len₁₀(n) × log(10) / log(36)
```

This yields roughly a **38–39% reduction** in digit characters.

### Delta path encoding

For two consecutive paths `p_(i-1)` and `p_i`:

- `c_i` = length of their longest common prefix (LCP)
- `s_i` = the remaining suffix of `p_i` after the shared prefix

The packed representation is:

```
packed(p_i) = base36(c_i) + "|" + s_i
```

Paths in sorted order (e.g. directory listings) benefit most from this transform.

### Token estimate

Token count is approximated as:

```
T(x) = ceil(|x| / 4)
```

where `|x|` is character count. Savings are reported as:

```
savings(%) = 100 × (1 − T_new / T_old)
```

---

## Benchmark

Results below are averages across 8 runs on a Linux environment. Time is wall-clock elapsed; tokens are estimated with `ceil(chars / 4)`.

| Command pair | Old time | New time | Time Δ | Old tokens | New tokens | Token Δ |
|---|---|---|---|---|---|---|
| `cat` → `acat --pack` | 1.54 ms | 2.72 ms | −77% | 907 | 763 | **−16%** |
| `ls -la` → `als --pack` | 3.38 ms | 1.71 ms | +49% | 99 | 40 | **−60%** |
| `grep -RIn` → `agrep --pack` | 2.92 ms | 1.97 ms | +32% | 254 | 203 | **−20%** |
| `find\|grep` → `afind --pack` | 3.65 ms | 1.76 ms | +52% | 26 | 24 | **−8%** |
| `du -ah\|sort` → `adu --pack` | 7.49 ms | 1.79 ms | +76% | 234 | 32 | **−86%** |
| `ps -eo` → `aps --pack` | 19.92 ms | 14.09 ms | +29% | 1863 | 1020 | **−45%** |

> Packed mode consistently reduces token payload. Runtime may increase for small workloads (e.g. `acat`) where transform overhead outweighs I/O cost; the primary optimisation target is **LLM context efficiency**, not wall-clock speed.

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

### Basic examples

```bash
als . --pack
acat src/lib.rs --pack --max 40
agrep TODO src --max 30 --pack
afind config . --type f --pack
adu . --max 20 --pack
aps --max 30 --pack
```

### Command reference

| Traditional command | AI-native equivalent |
|---|---|
| `ls` | `als` |
| `ls -la --time-style=+%s .` | `als . --pack` |
| `cat` | `acat` |
| `cat src/bin/aps.rs` | `acat src/bin/aps.rs --pack --max 200` |
| `grep -RIn <pattern> <path>` | `agrep <pattern> <path>` |
| `grep -RIn --binary-files=without-match use src` | `agrep use src --max 200 --pack` |
| `find <path> -type f \| grep <pattern>` | `afind <pattern> <path> --type f` |
| `find src -type f \| grep rs` | `afind rs src --type f --max 200 --pack` |
| `du -ah <path> \| sort -rh \| head -n N` | `adu <path> --max N` |
| `du -ah . \| sort -rh \| head -n 20` | `adu . --max 20 --pack` |
| `ps -eo pid,ppid,state,rss,comm,args --sort=-rss \| head -n N` | `aps --max N` |
| `ps -eo pid,ppid,state,rss,comm,args --sort=-rss \| head -n 30` | `aps --max 30 --pack` |

---

## Running the benchmark

Compare traditional commands against their AI-native replacements. The script reports average elapsed time, estimated token count, and percentage savings for each pair.

```bash
# Full run (8 iterations)
RUNS=8 scripts/benchmark_old_vs_new.sh

# Quick run (4 iterations)
RUNS=4 scripts/benchmark_old_vs_new.sh
```

---

## License

MIT