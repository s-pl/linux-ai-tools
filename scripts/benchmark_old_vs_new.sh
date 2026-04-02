#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

RUNS="${RUNS:-8}"
WARMUP="${WARMUP:-1}"
BUILD_IF_MISSING="${BUILD_IF_MISSING:-1}"
NEW_BIN_DIR="${NEW_BIN_DIR:-$ROOT_DIR/target/release}"
BENCH_DATA_DIR="${BENCH_DATA_DIR:-$ROOT_DIR/.benchdata}"
REPORT_DIR="${REPORT_DIR:-$ROOT_DIR/reports/benchmark}"
CASE_TIMEOUT_SEC="${CASE_TIMEOUT_SEC:-45}"
INCLUDE_HEAVY="${INCLUDE_HEAVY:-0}"
SCENARIO_FILTER="${SCENARIO_FILTER:-}"

need_bin() {
  local p="$1"
  [[ -x "$p" ]]
}

if [[ "$BUILD_IF_MISSING" == "1" ]]; then
  if ! need_bin "$NEW_BIN_DIR/als" || ! need_bin "$NEW_BIN_DIR/acat" || ! need_bin "$NEW_BIN_DIR/agrep" || ! need_bin "$NEW_BIN_DIR/afind" || ! need_bin "$NEW_BIN_DIR/adu" || ! need_bin "$NEW_BIN_DIR/aps" || ! need_bin "$NEW_BIN_DIR/atok"; then
    cargo build --release >/dev/null
  fi
fi

TIME_BIN=""
HAS_GNU_TIME="0"
if [[ -x "/usr/bin/time" ]]; then
  TIME_BIN="/usr/bin/time"
elif [[ -x "/bin/time" ]]; then
  TIME_BIN="/bin/time"
elif command -v gtime >/dev/null 2>&1; then
  TIME_BIN="$(command -v gtime)"
fi

if [[ -n "$TIME_BIN" ]]; then
  probe_file="$(mktemp)"
  if "$TIME_BIN" -f "elapsed=%e" -o "$probe_file" true >/dev/null 2>&1; then
    HAS_GNU_TIME="1"
  fi
  rm -f "$probe_file"
fi

mkdir -p "$BENCH_DATA_DIR" "$REPORT_DIR"

norm_decimal() {
  echo "$1" | tr ',' '.'
}

pct_delta() {
  awk -v old="$1" -v new="$2" 'BEGIN { if (old == 0) print "0.0"; else printf "%.1f", (old - new) * 100.0 / old }'
}

estimate_tokens_file() {
  local file="$1"
  if [[ -x "$NEW_BIN_DIR/atok" ]]; then
    "$NEW_BIN_DIR/atok" < "$file" 2>/dev/null || echo 0
  else
    local chars
    chars="$(wc -c < "$file" | tr -d ' ')"
    echo $(((chars + 3) / 4))
  fi
}

avg_file() {
  local file="$1"
  awk '{ s+=$1; n+=1 } END { if (n==0) print "0.000000"; else printf "%.6f", s/n }' "$file"
}

pctl_file() {
  local file="$1"
  local p="$2"
  sort -n "$file" | awk -v p="$p" '
    { a[NR]=$1 }
    END {
      if (NR==0) { print "0.000000"; exit }
      idx=int((p/100.0)*NR + 0.5)
      if (idx < 1) idx=1
      if (idx > NR) idx=NR
      printf "%.6f", a[idx]
    }
  '
}

prepare_realistic_dataset() {
  local base="$BENCH_DATA_DIR"
  local text_dir="$base/text"
  local tree_dir="$base/tree"
  local logs_dir="$base/logs"
  local generated_flag="$base/.generated"

  if [[ -f "$generated_flag" ]]; then
    return
  fi

  rm -rf "$base"
  mkdir -p "$text_dir" "$tree_dir" "$logs_dir"

  # Log-like corpus with repeated prefixes and varied tokens.
  local i j
  for i in $(seq 1 60); do
    local f="$logs_dir/app_${i}.log"
    : > "$f"
    for j in $(seq 1 220); do
      printf "2026-04-01T12:%02d:%02dZ service=api node=n%02d level=INFO msg=request_ok route=/v1/items/%04d user=u%04d latency_ms=%d\n" \
        "$((j % 60))" "$((j % 60))" "$((i % 17))" "$j" "$((i * 10 + j))" "$((j % 170 + 10))" >> "$f"
      if (( j % 17 == 0 )); then
        printf "2026-04-01T12:%02d:%02dZ service=api node=n%02d level=ERROR msg=db_timeout route=/v1/items/%04d retry=%d\n" \
          "$((j % 60))" "$((j % 60))" "$((i % 17))" "$j" "$((j % 3))" >> "$f"
      fi
      if (( j % 23 == 0 )); then
        printf "2026-04-01T12:%02d:%02dZ service=worker node=n%02d level=WARN msg=queue_backpressure queue=q_%02d size=%d\n" \
          "$((j % 60))" "$((j % 60))" "$((i % 13))" "$((i % 11))" "$((j * 7))" >> "$f"
      fi
    done
  done

  # Source-like tree for find/grep use-cases.
  for i in $(seq 1 40); do
    mkdir -p "$tree_dir/service_${i}/handlers" "$tree_dir/service_${i}/models" "$tree_dir/service_${i}/tests"
    cat > "$tree_dir/service_${i}/handlers/http_handler_${i}.rs" <<EOF
pub fn handle_request_${i}(user_id: u64, item_id: u64) -> Result<String, String> {
    if user_id == 0 { return Err("invalid_user".to_string()); }
    Ok(format!("service_${i}:{}", item_id))
}
EOF
    cat > "$tree_dir/service_${i}/models/model_${i}.rs" <<EOF
pub struct Model${i} {
    pub id: u64,
    pub status: &'static str,
}
EOF
    cat > "$tree_dir/service_${i}/tests/test_${i}.txt" <<EOF
TODO: integration test service_${i}
WARN: flaky network in CI
EOF
  done

  # Large single file for acat throughput scenarios.
  cat "$logs_dir"/*.log > "$text_dir/huge.log"

  echo "generated" > "$generated_flag"
}

run_case_stats() {
  local cmd="$1"
  local mode="$2"
  local label="$3"

  local times_file tokens_file chars_file
  times_file="$(mktemp)"
  tokens_file="$(mktemp)"
  chars_file="$(mktemp)"

  local failures=0
  local run_idx=1

  if [[ "$WARMUP" -gt 0 ]]; then
    if command -v timeout >/dev/null 2>&1; then
      timeout "$CASE_TIMEOUT_SEC" bash -lc "$cmd" >/dev/null 2>/dev/null || true
    else
      bash -lc "$cmd" >/dev/null 2>/dev/null || true
    fi
  fi

  while [[ "$run_idx" -le "$RUNS" ]]; do
    local out_file time_file elapsed chars tokens
    out_file="$(mktemp)"
    time_file="$(mktemp)"

    if [[ "$HAS_GNU_TIME" == "1" ]]; then
      set +e
      if command -v timeout >/dev/null 2>&1; then
        timeout "$CASE_TIMEOUT_SEC" "$TIME_BIN" -f "elapsed=%e" -o "$time_file" bash -lc "$cmd" >"$out_file" 2>/dev/null
      else
        "$TIME_BIN" -f "elapsed=%e" -o "$time_file" bash -lc "$cmd" >"$out_file" 2>/dev/null
      fi
      local rc=$?
      set -e
      elapsed="$(grep '^elapsed=' "$time_file" | cut -d'=' -f2)"
      elapsed="$(norm_decimal "${elapsed:-0}")"
    else
      set +e
      if command -v timeout >/dev/null 2>&1; then
        timeout "$CASE_TIMEOUT_SEC" bash -lc 'TIMEFORMAT="real=%R"; { time eval "$1" >"$2" 2>/dev/null; } 2>"$3"' _ "$cmd" "$out_file" "$time_file"
      else
        bash -lc 'TIMEFORMAT="real=%R"; { time eval "$1" >"$2" 2>/dev/null; } 2>"$3"' _ "$cmd" "$out_file" "$time_file"
      fi
      local rc=$?
      set -e
      elapsed="$(sed -E 's/.*real=([0-9.,]+).*/\1/' "$time_file" | tr -d '\n')"
      elapsed="$(norm_decimal "${elapsed:-0}")"
    fi

    if [[ "$rc" -ne 0 ]]; then
      failures=$((failures + 1))
    fi

    chars="$(wc -c < "$out_file" | tr -d ' ')"
    tokens="$(estimate_tokens_file "$out_file")"

    # Minimal validity checks so benchmarks are not only timing empty/error output.
    if [[ "$chars" -le 0 ]]; then
      failures=$((failures + 1))
    fi
    if [[ "$mode" == "new" ]]; then
      case "$label" in
        *"als"*|*"agrep"*|*"adu"*|*"aps"*|*"acat"*)
          if ! head -n 1 "$out_file" | grep -q '^@ap'; then
            failures=$((failures + 1))
          fi
          ;;
      esac
    fi

    echo "${elapsed:-0}" >> "$times_file"
    echo "${tokens:-0}" >> "$tokens_file"
    echo "${chars:-0}" >> "$chars_file"

    rm -f "$out_file" "$time_file"
    run_idx=$((run_idx + 1))
  done

  local avg_t p50_t p95_t avg_tok avg_chars
  avg_t="$(avg_file "$times_file")"
  p50_t="$(pctl_file "$times_file" 50)"
  p95_t="$(pctl_file "$times_file" 95)"
  avg_tok="$(avg_file "$tokens_file")"
  avg_chars="$(avg_file "$chars_file")"

  rm -f "$times_file" "$tokens_file" "$chars_file"

  echo "$avg_t|$p50_t|$p95_t|$avg_tok|$avg_chars|$failures"
}

prepare_realistic_dataset

cases=(
  "workspace|||ls -la --time-style=+%s .|||$NEW_BIN_DIR/als . --pack|||ls -> als (workspace)"
  "synthetic|||ls -la --time-style=+%s $BENCH_DATA_DIR/tree|||$NEW_BIN_DIR/als $BENCH_DATA_DIR/tree --pack|||ls -> als (synthetic tree)"

  "workspace|||cat src/bin/aps.rs|||$NEW_BIN_DIR/acat src/bin/aps.rs --pack --max 200|||cat -> acat (workspace file)"
  "synthetic|||cat $BENCH_DATA_DIR/text/huge.log|||$NEW_BIN_DIR/acat $BENCH_DATA_DIR/text/huge.log --pack --max 1200|||cat -> acat (large log)"

  "workspace|||grep -RIn --binary-files=without-match --exclude-dir=.git --exclude-dir=target use src|||$NEW_BIN_DIR/agrep use src --max 300 --pack|||grep -> agrep (workspace)"
  "synthetic|||grep -RIn --binary-files=without-match ERROR $BENCH_DATA_DIR/logs|||$NEW_BIN_DIR/agrep ERROR $BENCH_DATA_DIR/logs --max 300 --pack|||grep -> agrep (synthetic logs)"

  "workspace|||find src -type f | grep rs|||$NEW_BIN_DIR/afind rs src --type f --max 300 --pack|||find|grep -> afind (workspace)"
  "synthetic|||find $BENCH_DATA_DIR/tree -type f | grep handler_|||$NEW_BIN_DIR/afind handler_ $BENCH_DATA_DIR/tree --type f --max 300 --pack|||find|grep -> afind (synthetic tree)"

  "workspace|||du -ah src | sort -rh | head -n 20|||$NEW_BIN_DIR/adu src --max 20 --pack|||du|sort -> adu (workspace src)"
  "synthetic|||du -ah $BENCH_DATA_DIR | sort -rh | head -n 30|||$NEW_BIN_DIR/adu $BENCH_DATA_DIR --max 30 --pack|||du|sort -> adu (synthetic)"

  "system|||ps -eo pid,ppid,state,rss,comm,args --sort=-rss | head -n 30|||$NEW_BIN_DIR/aps --max 30 --pack|||ps -> aps (top 30)"
  "system|||ps -eo pid,ppid,state,rss,comm,args --sort=-rss | head -n 80|||$NEW_BIN_DIR/aps --max 80 --pack|||ps -> aps (top 80)"
)

if [[ "$INCLUDE_HEAVY" == "1" ]]; then
  cases+=(
    "workspace|||du -ah . | sort -rh | head -n 40|||$NEW_BIN_DIR/adu . --max 40 --pack|||du|sort -> adu (workspace full)"
    "synthetic|||cat $BENCH_DATA_DIR/text/huge.log|||$NEW_BIN_DIR/acat $BENCH_DATA_DIR/text/huge.log --pack --aggressive --max 3000|||cat -> acat (huge aggressive)"
  )
fi

if [[ -n "$SCENARIO_FILTER" ]]; then
  filtered=()
  for c in "${cases[@]}"; do
    if echo "$c" | grep -qi "$SCENARIO_FILTER"; then
      filtered+=("$c")
    fi
  done
  cases=("${filtered[@]}")
fi

timestamp="$(date +%Y%m%d-%H%M%S)"
csv_report="$REPORT_DIR/benchmark_${timestamp}.csv"
md_report="$REPORT_DIR/benchmark_${timestamp}.md"
latest_csv="$REPORT_DIR/latest.csv"
latest_md="$REPORT_DIR/latest.md"

printf "profile,label,old_avg_s,new_avg_s,time_save_pct,old_p95_s,new_p95_s,old_tokens,new_tokens,token_save_pct,old_chars,new_chars,old_failures,new_failures\n" > "$csv_report"

printf "\nBenchmark runs per scenario: %s (warmup=%s)\n\n" "$RUNS" "$WARMUP"
printf "%-33s | %5s | %8s | %8s | %7s | %10s | %10s | %11s | %6s\n" \
  "Scenario" "Type" "Old s" "New s" "Save %" "Old tok" "New tok" "Token save%" "Fails"
printf "%s\n" "------------------------------------------------------------------------------------------------------------------------"

{
  echo "# Benchmark Report"
  echo
  echo "- Generated: $(date -Iseconds)"
  echo "- Runs per scenario: $RUNS"
  echo "- Warmup runs: $WARMUP"
  echo "- Dataset: workspace + synthetic realistic fixtures in $BENCH_DATA_DIR"
  echo
  echo "| Scenario | Type | Old avg s | New avg s | Time save % | Old p95 s | New p95 s | Old tokens | New tokens | Token save % | Old fail | New fail |"
  echo "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
} > "$md_report"

sum_time_save="0"
sum_tok_save="0"
count_rows=0
total_old_fail=0
total_new_fail=0

for c in "${cases[@]}"; do
  profile="${c%%|||*}"
  rest="${c#*|||}"
  old_cmd="${rest%%|||*}"
  rest="${rest#*|||}"
  new_cmd="${rest%%|||*}"
  label="${rest#*|||}"

  echo "[bench] running: $label"

  old_stats="$(run_case_stats "$old_cmd" "old" "$label")"
  new_stats="$(run_case_stats "$new_cmd" "new" "$label")"

  old_avg="$(echo "$old_stats" | cut -d'|' -f1)"
  old_p50="$(echo "$old_stats" | cut -d'|' -f2)"
  old_p95="$(echo "$old_stats" | cut -d'|' -f3)"
  old_tok="$(echo "$old_stats" | cut -d'|' -f4 | awk '{printf "%.0f", $1}')"
  old_chars="$(echo "$old_stats" | cut -d'|' -f5 | awk '{printf "%.0f", $1}')"
  old_fail="$(echo "$old_stats" | cut -d'|' -f6)"

  new_avg="$(echo "$new_stats" | cut -d'|' -f1)"
  new_p50="$(echo "$new_stats" | cut -d'|' -f2)"
  new_p95="$(echo "$new_stats" | cut -d'|' -f3)"
  new_tok="$(echo "$new_stats" | cut -d'|' -f4 | awk '{printf "%.0f", $1}')"
  new_chars="$(echo "$new_stats" | cut -d'|' -f5 | awk '{printf "%.0f", $1}')"
  new_fail="$(echo "$new_stats" | cut -d'|' -f6)"

  save_time="$(pct_delta "$old_avg" "$new_avg")"
  save_tok="$(pct_delta "$old_tok" "$new_tok")"

  printf "%-33s | %5s | %8s | %8s | %7s | %10s | %10s | %10s%% | %2s/%-2s\n" \
    "$label" "$profile" "$old_avg" "$new_avg" "$save_time" "$old_tok" "$new_tok" "$save_tok" "$old_fail" "$new_fail"

  printf "%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s\n" \
    "$profile" "$label" "$old_avg" "$new_avg" "$save_time" "$old_p95" "$new_p95" "$old_tok" "$new_tok" "$save_tok" "$old_chars" "$new_chars" "$old_fail" "$new_fail" >> "$csv_report"

  printf "| %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s |\n" \
    "$label" "$profile" "$old_avg" "$new_avg" "$save_time" "$old_p95" "$new_p95" "$old_tok" "$new_tok" "$save_tok" "$old_fail" "$new_fail" >> "$md_report"

  sum_time_save="$(awk -v a="$sum_time_save" -v b="$save_time" 'BEGIN { printf "%.6f", a+b }')"
  sum_tok_save="$(awk -v a="$sum_tok_save" -v b="$save_tok" 'BEGIN { printf "%.6f", a+b }')"
  total_old_fail=$((total_old_fail + old_fail))
  total_new_fail=$((total_new_fail + new_fail))
  count_rows=$((count_rows + 1))

done

avg_time_save="$(awk -v s="$sum_time_save" -v n="$count_rows" 'BEGIN { if (n==0) print "0.0"; else printf "%.1f", s/n }')"
avg_tok_save="$(awk -v s="$sum_tok_save" -v n="$count_rows" 'BEGIN { if (n==0) print "0.0"; else printf "%.1f", s/n }')"

printf "\nSummary: avg time save=%s%%, avg token save=%s%%, total failures old/new=%s/%s\n" \
  "$avg_time_save" "$avg_tok_save" "$total_old_fail" "$total_new_fail"

{
  echo
  echo "## Summary"
  echo
  echo "- Average time save across scenarios: ${avg_time_save}%"
  echo "- Average token save across scenarios: ${avg_tok_save}%"
  echo "- Total failure count old/new: ${total_old_fail}/${total_new_fail}"
  echo
  echo "Reports:"
  echo "- CSV: $csv_report"
  echo "- Markdown: $md_report"
} >> "$md_report"

cp "$csv_report" "$latest_csv"
cp "$md_report" "$latest_md"

echo "Saved reports:"
echo "- $csv_report"
echo "- $md_report"
echo "- $latest_csv"
echo "- $latest_md"
