#!/usr/bin/env bash
# 语料回归：对每个 .xlsx 做 unpack -> repack，统计失败与告警。
# 用法：./scripts/corpus_check.sh [root...]   （默认 /tmp/xlsx_more）
# 产物：/tmp/corpus_check/{fails.txt,summary.txt}
set -uo pipefail
cd "$(dirname "$0")/.."
ROOT_DIR="$(pwd)"
BIN="$ROOT_DIR/target/release/json2xlsx"
cargo build --release --quiet -p json2xlsx 2>/dev/null || cargo build --release -q -p json2xlsx

OUT=/tmp/corpus_check
rm -rf "$OUT"; mkdir -p "$OUT"
roots=("$@"); [ ${#roots[@]} -eq 0 ] && roots=(/tmp/xlsx_more)

list="$OUT/all.txt"; : > "$list"
for r in "${roots[@]}"; do
  find "$r" -type f \( -iname '*.xlsx' -o -iname '*.xlsm' \) -size -5M 2>/dev/null >> "$list"
done
total=$(wc -l < "$list" | tr -d ' ')
echo "checking $total files..."

one() {
  local f="$1" bin="$2" out="$3"
  local name src d rc
  name=$(basename "$f")
  src=$(basename "$(dirname "$f")")
  d=$(mktemp -d)
  if "$bin" unpack "$f" -o "$d/prod" >/dev/null 2>"$d/err1"; then
    if "$bin" repack "$d/prod" -o "$d/rt.xlsx" >/dev/null 2>"$d/err2"; then
      :
    else
      printf 'REPACK\t%s\t%s\t%s\n' "$src" "$name" "$(head -1 "$d/err2")" >> "$out/fails.txt"
    fi
  else
    printf 'UNPACK\t%s\t%s\t%s\n' "$src" "$name" "$(head -1 "$d/err1")" >> "$out/fails.txt"
  fi
  rm -rf "$d"
}
export -f one 2>/dev/null || true

: > "$OUT/fails.txt"
# 并发执行（BSD xargs）
cat "$list" | xargs -P 8 -I{} bash -c 'one "$@"' _ {} "$BIN" "$OUT"

nf=$(wc -l < "$OUT/fails.txt" | tr -d ' ')
echo "=== fails by source ===" | tee "$OUT/summary.txt"
if [ "$nf" -gt 0 ]; then
  cut -f2 "$OUT/fails.txt" | sort | uniq -c | sort -rn | tee -a "$OUT/summary.txt"
  echo "--- by stage ---" | tee -a "$OUT/summary.txt"
  cut -f1 "$OUT/fails.txt" | sort | uniq -c | tee -a "$OUT/summary.txt"
fi
echo "TOTAL: $total checked, $nf FAIL (see $OUT/fails.txt)" | tee -a "$OUT/summary.txt"
