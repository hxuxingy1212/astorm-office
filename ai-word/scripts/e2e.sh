#!/usr/bin/env bash
# 端到端测试：构建 → 生成示例 → 校验 XML → CLI 全链路（unpack/view/edit/repack/render）
set -euo pipefail
cd "$(dirname "$0")/.."

BIN="$(pwd)/target/release/json2docx"
PASS=0
FAIL=0

step() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }
ok()   { echo "  ✔ $1"; PASS=$((PASS+1)); }
bad()  { echo "  ✘ $1"; FAIL=$((FAIL+1)); }

command -v xmllint >/dev/null 2>&1 || { echo "需要 xmllint"; exit 2; }

step "构建"
cargo build --release --quiet
ok "release 构建完成"

step "生成示例"
cargo run --quiet --example simple   >/dev/null && ok "simple.docx"
cargo run --quiet --example paper    >/dev/null && ok "paper.docx"
cargo run --quiet --example features >/dev/null && ok "features.docx"

# 校验 docx 内所有 XML/rels 结构良好
check_docx() {
  local f="$1" tmp
  tmp="$(mktemp -d)"
  (cd "$tmp" && unzip -qq "$f")
  local bad=0
  while IFS= read -r p; do
    xmllint --noout "$p" 2>/dev/null || { echo "  非法 XML: $p"; bad=1; }
  done < <(find "$tmp" \( -name '*.xml' -o -name '*.rels' \) -type f)
  rm -rf "$tmp"
  return $bad
}

step "XML 校验"
for f in examples/out/simple.docx examples/out/paper.docx examples/out/features.docx; do
  if check_docx "$(pwd)/$f"; then ok "$f 结构良好"; else bad "$f 存在非法 XML"; fi
done

step "可读性校验（macOS textutil，可选）"
if command -v textutil >/dev/null 2>&1; then
  if textutil -convert txt -stdout examples/out/features.docx >/dev/null 2>&1; then
    ok "textutil 可打开 features.docx"
  else
    bad "textutil 无法打开 features.docx"
  fi
else
  echo "  (跳过：非 macOS)"
fi

step "CLI 全链路：unpack → view → edit → repack → render"
WORK="$(mktemp -d)"
$BIN unpack examples/out/features.docx -o "$WORK/prod" >/dev/null && ok "unpack"
[ -f "$WORK/prod/document.json" ] && ok "产物目录 document.json 存在" || bad "缺少 document.json"
$BIN view "$WORK/prod" /part[1] layout >/dev/null && ok "view layout"
$BIN view "$WORK/prod" /part[1] text   >/dev/null && ok "view text"
$BIN edit "$WORK/prod" /part[1]/paragraph[1] set --prop text="e2e 修改后的正文" >/dev/null && ok "edit set"
$BIN edit "$WORK/prod" /part[1] add --type textbox --prop text="新增文本框" --prop fill=FFF2CC >/dev/null && ok "edit add textbox"
$BIN repack "$WORK/prod" -o "$WORK/out.docx" >/dev/null && ok "repack"
check_docx "$WORK/out.docx" && ok "repack 结果结构良好" || bad "repack 结果非法"
$BIN render "$WORK/out.docx" /part[1] -o "$WORK/p.html" >/dev/null && ok "render html"
[ -f "$WORK/p.html" ] && grep -q "e2e 修改后的正文" "$WORK/p.html" && ok "HTML 含编辑后的文本" || bad "HTML 内容不符"
$BIN render "$WORK/out.docx" / -o "$WORK/all.html" >/dev/null && ok "render 整篇 html"

step "整篇图像/PDF 渲染（需要 Chrome 或 LibreOffice，best-effort）"
if $BIN render "$WORK/out.docx" / -o "$WORK/all.png" >/dev/null 2>&1 && [ -s "$WORK/all.png" ]; then
  dim="$(file "$WORK/all.png" | grep -o '[0-9]* x [0-9]*' | head -1)"
  ok "PNG 渲染成功 ($dim)"
else
  echo "  (跳过：无 Chrome/LibreOffice)"
fi
if $BIN render "$WORK/out.docx" / -o "$WORK/all.pdf" >/dev/null 2>&1 && [ -s "$WORK/all.pdf" ]; then
  ok "PDF 导出成功"
else
  echo "  (跳过：无 Chrome/LibreOffice)"
fi
rm -rf "$WORK"

step "结果"
echo "  通过 $PASS 项，失败 $FAIL 项"
[ "$FAIL" -eq 0 ]
