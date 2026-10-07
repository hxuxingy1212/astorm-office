#!/usr/bin/env bash
# 批量模板提取：对语料中的每份 docx 运行 `extract`，产出各自的 template.json + TEMPLATE.md。
# 用法：./scripts/collect_templates.sh [语料目录] [输出目录]
#   默认 语料=/tmp/docx_corpus  输出=./templates_catalog
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="$(pwd)/target/release/json2docx"
cargo build --release --quiet

CORPUS="${1:-/tmp/docx_corpus}"
OUTDIR="${2:-$(pwd)/templates_catalog}"
[ -d "$CORPUS" ] || { echo "语料目录不存在：$CORPUS（先运行 scripts/corpus.sh）"; exit 2; }
rm -rf "$OUTDIR"; mkdir -p "$OUTDIR"

n=0
while IFS= read -r f; do
  rel="$(basename "$(dirname "$f")")/$(basename "$f" .docx)"
  out="$OUTDIR/$rel"
  if "$BIN" extract "$f" -o "$out" >/dev/null 2>&1; then
    n=$((n+1))
  else
    echo "EXTRACT-FAIL $f"
  fi
done < <(find "$CORPUS" -name '*.docx' -type f | sort)

echo "已提取 $n 个模板 → $OUTDIR"
python3 - "$OUTDIR" <<'PY'
import json, os, sys
root=sys.argv[1]
print(f"{'模板':34} {'页面':6} {'正文字体':22} {'正文':4} {'H1':4} {'H1粗':4}")
for dirpath,_,files in sorted(os.walk(root)):
    if 'template.json' not in files: continue
    d=json.load(open(os.path.join(dirpath,'template.json')))
    name=os.path.relpath(dirpath,root)
    page=(d.get('page') or {}).get('size','-')
    st=d.get('styles') or {}
    n=st.get('Normal') or {}
    h=st.get('Heading1') or {}
    font=(n.get('font_family') or '-')
    print(f"{name:34} {str(page):6} {font:22} {str(n.get('font_size','-')):4} {str(h.get('font_size','-')):4} {str(h.get('bold','-')):4}")
PY
