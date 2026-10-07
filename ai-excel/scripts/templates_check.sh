#!/usr/bin/env bash
# 校验内置模板库：每个 skill/templates/*/skeleton 都能 repack 成功。
# 用法：./scripts/templates_check.sh
set -uo pipefail
cd "$(dirname "$0")/.."
cargo build --release -q -p json2xlsx 2>/dev/null
BIN=target/release/json2xlsx
ok=0; bad=0; miss=0
for tdir in skill/templates/*/; do
  name=$(basename "$tdir")
  [ -f "$tdir/template.json" ] || continue
  if [ ! -d "$tdir/skeleton" ]; then
    echo "缺少 skeleton：$name（可运行 python3 scripts/gen_template_skeleton.py $tdir）"
    miss=$((miss+1)); continue
  fi
  out=$(mktemp -d)/t.xlsx
  if "$BIN" repack "$tdir/skeleton" -o "$out" >/dev/null 2>err.txt; then
    ok=$((ok+1))
  else
    bad=$((bad+1)); echo "repack 失败：$name — $(head -1 err.txt)"
  fi
  rm -rf "$(dirname "$out")"
done
rm -f err.txt
echo "模板：通过 ${ok}，失败 ${bad}，缺 skeleton ${miss}"
[ "$bad" -eq 0 ] && [ "$miss" -eq 0 ]
