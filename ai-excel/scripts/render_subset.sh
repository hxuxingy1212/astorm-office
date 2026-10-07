#!/usr/bin/env bash
# 小语料渲染回归（CI 友好）：对 tests/corpus/*.xlsx 做 生成→解析→LibreOffice
# 渲染像素比对，仅当出现「不在基线白名单内的新差异」或「回环渲染失败」时失败。
# 原始文件本身无法渲染的样本（NOSRC）不计为失败。
# 用法：./scripts/render_subset.sh   （更新基线：见 tests/corpus/render_baseline.txt）
set -uo pipefail
cd "$(dirname "$0")/.."
if ! command -v soffice >/dev/null 2>&1 && [ ! -x /Applications/LibreOffice.app/Contents/MacOS/soffice ]; then
  echo "跳过：未安装 LibreOffice(soffice)"; exit 0
fi
python3 scripts/visual_diff_lo.py tests/corpus
results=/tmp/vlo/results.tsv
[ -f "$results" ] || { echo "无结果文件"; exit 1; }

baseline="tests/corpus/render_baseline.txt"
new=$(awk -F'\t' '$4!="ok" && $4!="NOSRC" {print $1}' "$results" | sort -u)
missing=0
[ -f "$baseline" ] && missing=$(comm -23 <(grep -vE '^\s*#|^\s*$' "$baseline" | sort -u) <(printf '%s\n' "$new" | sort -u) | wc -l | tr -d ' ')

fail=0
while IFS= read -r name; do
  [ -z "$name" ] && continue
  if ! grep -qxF "$name" "$baseline" 2>/dev/null; then
    echo "新增渲染差异（未在基线内）：$name"; fail=1
  fi
done <<< "$new"

if [ "$fail" -eq 0 ]; then
  echo "渲染子集通过：无新增差异（基线内 $(grep -cvE '^\s*#|^\s*$' "$baseline") 项）。"
else
  echo "提示：确认无回归后，可将该文件加入 $baseline。"
fi
exit "$fail"
