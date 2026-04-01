#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

RUNS="${RUNS:-8}"
NEW_BIN_DIR="${NEW_BIN_DIR:-$ROOT_DIR/target/release}"
BUILD_IF_MISSING="${BUILD_IF_MISSING:-1}"

need_bin() {
  local p="$1"
  [[ -x "$p" ]]
}

if [[ "$BUILD_IF_MISSING" == "1" ]]; then
  if ! need_bin "$NEW_BIN_DIR/als" || ! need_bin "$NEW_BIN_DIR/acat" || ! need_bin "$NEW_BIN_DIR/agrep" || ! need_bin "$NEW_BIN_DIR/afind" || ! need_bin "$NEW_BIN_DIR/adu" || ! need_bin "$NEW_BIN_DIR/aps"; then
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

add_float() {
  awk -v a="$1" -v b="$2" 'BEGIN { printf "%.6f", a + b }'
}

norm_decimal() {
  echo "$1" | tr ',' '.'
}

div_float() {
  awk -v a="$1" -v b="$2" 'BEGIN { if (b == 0) print "0.000000"; else printf "%.6f", a / b }'
}

pct_delta() {
  awk -v old="$1" -v new="$2" 'BEGIN { if (old == 0) print "0.0"; else printf "%.1f", (old - new) * 100.0 / old }'
}

estimate_tokens() {
  local chars="$1"
  echo $(((chars + 3) / 4))
}

run_case() {
  local label="$1"
  local cmd="$2"

  local sum_elapsed="0.0"
  local sum_cpu="0.0"
  local sum_rss_kb="0"
  local sum_chars="0"
  local sum_tokens="0"

  local i=1
  while [[ "$i" -le "$RUNS" ]]; do
    local out_file
    local time_file
    out_file="$(mktemp)"
    time_file="$(mktemp)"

    if [[ "$HAS_GNU_TIME" == "1" ]]; then
      "$TIME_BIN" -f "elapsed=%e\ncpu=%P\nrss_kb=%M\nuser=%U\nsys=%S" -o "$time_file" bash -lc "$cmd" >"$out_file" 2>/dev/null || true
    else
      bash -lc 'TIMEFORMAT="real=%R user=%U sys=%S"; { time eval "$1" >"$2" 2>/dev/null; } 2>"$3"' _ "$cmd" "$out_file" "$time_file" || true
    fi

    local elapsed cpu_pct rss_kb chars tokens
    if [[ "$HAS_GNU_TIME" == "1" ]]; then
      elapsed="$(grep '^elapsed=' "$time_file" | cut -d'=' -f2)"
      cpu_pct="$(grep '^cpu=' "$time_file" | cut -d'=' -f2 | tr -d '%')"
      rss_kb="$(grep '^rss_kb=' "$time_file" | cut -d'=' -f2)"
      elapsed="$(norm_decimal "$elapsed")"
      cpu_pct="$(norm_decimal "$cpu_pct")"
    else
      local real_s user_s sys_s line
      line="$(tr -d '\n' < "$time_file")"
      real_s="$(echo "$line" | sed -E 's/.*real=([0-9.,]+).*/\1/')"
      user_s="$(echo "$line" | sed -E 's/.*user=([0-9.,]+).*/\1/')"
      sys_s="$(echo "$line" | sed -E 's/.*sys=([0-9.,]+).*/\1/')"
      real_s="$(norm_decimal "$real_s")"
      user_s="$(norm_decimal "$user_s")"
      sys_s="$(norm_decimal "$sys_s")"
      elapsed="${real_s:-0}"
      cpu_pct="$(awk -v r="${real_s:-0}" -v u="${user_s:-0}" -v s="${sys_s:-0}" 'BEGIN { if (r == 0) print "0.0"; else printf "%.3f", ((u+s)*100.0)/r }')"
      rss_kb="-1"
    fi

    elapsed="${elapsed:-0}"
    cpu_pct="${cpu_pct:-0}"
    rss_kb="${rss_kb:-0}"

    chars="$(wc -c < "$out_file" | tr -d ' ')"
    tokens="$(estimate_tokens "$chars")"

    sum_elapsed="$(add_float "$sum_elapsed" "$elapsed")"
    sum_cpu="$(add_float "$sum_cpu" "$cpu_pct")"
    sum_rss_kb=$((sum_rss_kb + rss_kb))
    sum_chars=$((sum_chars + chars))
    sum_tokens=$((sum_tokens + tokens))

    rm -f "$out_file" "$time_file"
    i=$((i + 1))
  done

  local avg_elapsed avg_cpu avg_rss_kb avg_chars avg_tokens
  avg_elapsed="$(div_float "$sum_elapsed" "$RUNS")"
  avg_cpu="$(div_float "$sum_cpu" "$RUNS")"
  avg_rss_kb=$((sum_rss_kb / RUNS))
  avg_chars=$((sum_chars / RUNS))
  avg_tokens=$((sum_tokens / RUNS))

  echo "$label|$avg_elapsed|$avg_cpu|$avg_rss_kb|$avg_chars|$avg_tokens"
}

pairs=(
  "ls -la --time-style=+%s .|||$NEW_BIN_DIR/als . --pack|||ls -> als"
  "cat src/bin/aps.rs|||$NEW_BIN_DIR/acat src/bin/aps.rs --pack --max 200|||cat -> acat"
  "grep -RIn --binary-files=without-match --exclude-dir=.git --exclude-dir=target use src|||$NEW_BIN_DIR/agrep use src --max 200 --pack|||grep -> agrep"
  "find src -type f | grep rs|||$NEW_BIN_DIR/afind rs src --type f --max 200 --pack|||find|grep -> afind"
  "du -ah . | sort -rh | head -n 20|||$NEW_BIN_DIR/adu . --max 20 --pack|||du|sort -> adu"
  "ps -eo pid,ppid,state,rss,comm,args --sort=-rss | head -n 30|||$NEW_BIN_DIR/aps --max 30 --pack|||ps -> aps"
)

printf "\nBenchmark runs per command: %s\n\n" "$RUNS"
printf "%-18s | %8s | %8s | %7s | %10s | %10s | %11s\n" \
  "Command Pair" "Old s" "New s" "Save %" "Old tokens" "New tokens" "Token save%"
printf "%s\n" "--------------------------------------------------------------------------------------"

for p in "${pairs[@]}"; do
  old_cmd="${p%%|||*}"
  rest="${p#*|||}"
  new_cmd="${rest%%|||*}"
  label="${rest##*|||}"

  old_res="$(run_case "old" "$old_cmd")"
  new_res="$(run_case "new" "$new_cmd")"

  old_time="$(echo "$old_res" | cut -d'|' -f2)"
  old_cpu="$(echo "$old_res" | cut -d'|' -f3)"
  old_rss="$(echo "$old_res" | cut -d'|' -f4)"
  old_tok="$(echo "$old_res" | cut -d'|' -f6)"

  new_time="$(echo "$new_res" | cut -d'|' -f2)"
  new_cpu="$(echo "$new_res" | cut -d'|' -f3)"
  new_rss="$(echo "$new_res" | cut -d'|' -f4)"
  new_tok="$(echo "$new_res" | cut -d'|' -f6)"

  save_time="$(pct_delta "$old_time" "$new_time")"
  save_tok="$(pct_delta "$old_tok" "$new_tok")"

  printf "%-18s | %8s | %8s | %7s | %10s | %10s | %10s%%\n" \
    "$label" "$old_time" "$new_time" "$save_time" "$old_tok" "$new_tok" "$save_tok"

done
