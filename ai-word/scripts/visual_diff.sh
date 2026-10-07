#!/usr/bin/env bash
# 视觉对比：把「原始 docx」与「unpack→repack 回环 docx」渲染为 PNG 后逐张比较。
# 优先使用系统渲染器 QuickLook（qlmanage，更保真），回退 textutil+Chrome。
#   ./scripts/visual_diff.sh                # /tmp/docx_corpus 下全部
#   ./scripts/visual_diff.sh a.docx b.docx  # 指定文件
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="$(pwd)/target/release/json2docx"
cargo build --release --quiet
OUT=/tmp/vcmp
rm -rf "$OUT"; mkdir -p "$OUT"

CHROME=""
for c in "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
         "$(command -v google-chrome || true)" "$(command -v chromium || true)"; do
  [ -n "$c" ] && [ -x "$c" ] && CHROME="$c" && break
done
[ -z "$CHROME" ] && { echo "需要 Chrome/Chromium"; exit 2; }
command -v textutil >/dev/null || { echo "需要 macOS textutil"; exit 2; }

# 默认用 textutil(html)+Chrome：同一渲染管线，输出确定，可做逐字节/像素比较。
# 更保真的系统渲染器（首次页面）可用 VISUAL_RENDERER=qlmanage。
RENDERER="${VISUAL_RENDERER:-textutil}"
have_ql=false; command -v qlmanage >/dev/null 2>&1 && have_ql=true

render() {
  local f="$1" out_png="$2"
  if [ "$RENDERER" = "qlmanage" ] && $have_ql; then
    local od; od="$(mktemp -d)"
    ( qlmanage -t -s 1600 -o "$od" "$f" >/dev/null 2>&1 & p=$!; sleep 12; kill "$p" 2>/dev/null; wait "$p" 2>/dev/null ) || true
    local png; png="$(find "$od" -name '*.png' | head -1)"
    if [ -n "$png" ] && [ -s "$png" ]; then cp "$png" "$out_png"; rm -rf "$od"; return 0; fi
    rm -rf "$od"; return 1
  fi
  local h; h="$(mktemp -u).html"
  textutil -convert html "$f" -output "$h" 2>/dev/null || return 1
  "$CHROME" --headless --disable-gpu --hide-scrollbars --window-size=1400,2600 \
    --screenshot="$out_png" "file://$h" >/dev/null 2>&1 || true
  rm -f "$h"
  [ -s "$out_png" ]
}

files=("$@")
if [ ${#files[@]} -eq 0 ]; then
  while IFS= read -r f; do files+=("$f"); done < <(find /tmp/docx_corpus -name '*.docx' -type f | sort)
fi

same=0; diff=0; failed=0; diffs=()
for f in "${files[@]}"; do
  name="$(basename "$f" .docx)"; d="$OUT/$name"; mkdir -p "$d"
  rm -rf /tmp/vd_prod /tmp/vd_rt.docx
  if ! "$BIN" unpack "$f" -o /tmp/vd_prod >/dev/null 2>&1 || \
     ! "$BIN" repack /tmp/vd_prod -o /tmp/vd_rt.docx >/dev/null 2>&1; then
    echo "SKIP(rt)   $name"; failed=$((failed+1)); continue
  fi
  if ! render "$f" "$d/orig.png" || ! render /tmp/vd_rt.docx "$d/rt.png"; then
    echo "SKIP(png)  $name"; failed=$((failed+1)); continue
  fi
  if cmp -s "$d/orig.png" "$d/rt.png"; then
    same=$((same+1))
  else
    diff=$((diff+1)); diffs+=("$name"); echo "DIFF       $name"
  fi
done

renderer="textutil+Chrome"
[ "$RENDERER" = "qlmanage" ] && renderer="qlmanage(QuickLook)"
echo "----"
echo "渲染器：$renderer"
echo "视觉对比：完全一致 ${same}，有差异 ${diff}，跳过 ${failed}（共 ${#files[@]}）"
[ ${#diffs[@]} -gt 0 ] && echo "有差异：${diffs[*]}（PNG 在 $OUT/<名称>/）"
