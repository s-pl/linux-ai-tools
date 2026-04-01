---
name: ai-linux-tools
description: "Use when working with als/acat/agrep/afind/adu/aps, packed output (--pack), benchmark comparisons, README/index documentation updates, or alias setup for Linux AI-oriented CLI workflows."
---

# ai-linux-tools Skill

## Purpose

Standard workflow for maintaining this repository and validating that AI-oriented command replacements stay measurable and publish-ready.

## Use When

- Adding or modifying `als`, `acat`, `agrep`, `afind`, `adu`, or `aps`.
- Updating compression logic in packed mode.
- Running old-vs-new benchmarks and interpreting time/token deltas.
- Updating `README.md`, `index.html`, or shell alias mappings.

## Workflow

1. Build and verify binaries.
2. Run benchmark script with at least 4 runs.
3. Check docs for consistency with measured output.
4. Keep command mappings old -> new explicit.

## Commands

```bash
cargo build --release
RUNS=4 scripts/benchmark_old_vs_new.sh
```

## Quality Gates

- New behavior must be reflected in `README.md`.
- Benchmark tables should report time and token metrics clearly.
- Do not reintroduce wrappers around external classic tools for core command logic.
- Keep output deterministic and easy to parse for agents.
