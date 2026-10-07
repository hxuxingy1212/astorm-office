#!/usr/bin/env bash
# 视觉对比：把「原始 xlsx」与「unpack→repack 回环 xlsx」用 QuickLook(qlmanage)
# 渲染为 PNG，再用图像差比较，找出视觉差异。
#   ./scripts/visual_diff.sh <file...>            # 指定文件
#   ./scripts/visual_diff.sh --list <list.txt>    # 每行一个路径
#   LIMIT=60 ./scripts/visual_diff.sh /tmp/xlsx_more   # 目录下抽样
# 产物：/tmp/vcmp/<name>/{orig.png,rt.png,*_side.png,*_diff.png}
set -uo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
BIN="$ROOT/target/release/json2xlsx"
PY="/var/folders/j_/k9qc2jcs6tx95938m619pgrc0000gn/T/opencode/xlsxvenv/bin/python"
[ -x "$PY" ] || PY=python3
OUT=/tmp/vcmp
LIMIT="${LIMIT:-0}"
rm -rf "$OUT"; mkdir -p "$OUT"
cargo build --release -q -p json2xlsx 2>/dev/null

render() { # <file> <out_png>
  local f="$1" out="$2"
  local od; od="$(mktemp -d)"
  cp "$f" "$od/orig_$(basename "$f")"
  ( qlmanage -t -s 1500 -o "$od" "$od/orig_$(basename "$f")" >/dev/null 2>&1 & \
    p=$!; sleep 14; kill "$p" 2>/dev/null; wait "$p" 2>/dev/null ) || true
  local png; png="$(find "$od" -name '*.png' | head -1)"
  if [ -n "$png" ] && [ -s "$png" ]; then cp "$png" "$out"; rm -rf "$od"; return 0; fi
  rm -rf "$od"; return 1
}

files=()
if [ "${1:-}" = "--list" ]; then
  while IFS= read -r l; do [ -n "$l" ] && files+=("$l"); done < "$2"
elif [ $# -gt 0 ] && [ -d "${1:-}" ]; then
  while IFS= read -r l; do files+=("$l"); done < <(find "$1" -type f -iname '*.xlsx' -size -2M | sort)
else
  files=("$@")
fi

if [ "$LIMIT" -gt 0 ] && [ ${#files[@]} -gt "$LIMIT" ]; then
  step=$(( ${#files[@]} / LIMIT )); [ "$step" -lt 1 ] && step=1
  picked=(); for ((i=0;i<${#files[@]};i+=step)); do picked+=("${files[$i]}"); [ ${#picked[@]} -ge "$LIMIT" ] && break; done
  files=("${picked[@]}")
fi

echo "visual diff: ${#files[@]} files -> $OUT"
same=0; diff=0; skip=0; results="$OUT/results.tsv"; : > "$results"
for f in "${files[@]}"; do
  name="$(basename "$f" .xlsx)"
  d="$OUT/$name"; mkdir -p "$d"
  rm -rf /tmp/vd_prod /tmp/vd_rt.xlsx
  if ! "$BIN" unpack "$f" -o /tmp/vd_prod >/dev/null 2>&1 || \
     ! "$BIN" repack /tmp/vd_prod -o /tmp/vd_rt.xlsx >/dev/null 2>&1; then
    echo "SKIP(rt)   $name"; skip=$((skip+1)); continue
  fi
  if ! render "$f" "$d/orig.png" || ! render /tmp/vd_rt.xlsx "$d/rt.png"; then
    echo "SKIP(png)  $name"; skip=$((skip+1)); continue
  fi
  m=$("$PY" "$ROOT/scripts/img_diff.py" "$d/orig.png" "$d/rt.png" "$d" 2>/dev/null)
  mean=$(printf '%s' "$m" | sed -n 's/.*"mean": *\([0-9.]*\).*/\1/p')
  pct=$(printf '%s' "$m" | sed -n 's/.*"pct": *\([0-9.]*\).*/\1/p')
  printf '%s\t%s\t%s\n' "$name" "${mean:-?}" "${pct:-?}" >> "$results"
  # 阈值
  big=$(awk -v p="${pct:-1}" 'BEGIN{print (p+0>0.01)?1:0}')
  if [ "$big" = "1" ]; then echo "DIFF  $name  mean=${mean} pct=${pct}"; diff=$((diff+1));
  else echo "ok    $name  mean=${mean} pct=${pct}"; same=$((same+1)); fi
done

echo "----"
echo "视觉对比：一致 ${same}，有差异 ${diff}，跳过 ${skip}（共 ${#files[@]}）"
echo "差异明细：${results}（按 pct 排序见下）"
[ -s "$results" ] && sort -t$'\t' -k3 -rn "$results" | head -15
