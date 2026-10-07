#!/usr/bin/env bash
# 大规模 docx 采集：通过 codeload 下载若干"docx 富集"仓库的 zip，
# 解出全部 .docx，去重后汇集到 /tmp/docx_dataset。
# 不走 GitHub API，规避限流；原始数据不纳入版本库。
set -uo pipefail
OUT="${1:-/tmp/docx_dataset}"
mkdir -p "$OUT" "$OUT/_tmp"
export OUT

REPOS=(
  apache/poi
  jgm/pandoc
  plutext/docx4j
  Sayi/poi-tl
  open-xml-templating/docxtemplater
  dotnetcore/Magicodes.IE
  bokuweb/docx-rs
  python-openxml/python-docx
  elapouya/python-docx-template
  Achuan-2/pandoc_docx_template
  OfficeDev/Open-XML-SDK
  dolanmiu/docx
  thinkgem/jeesite
  ming1016/word
)

dl() {
  local repo="$1" branch="$2" zip="$OUT/_tmp/${repo//\//__}.zip"
  curl -fsSL -m 300 -o "$zip" "https://codeload.github.com/$repo/zip/refs/heads/$branch" && { echo "$zip"; return 0; }
  return 1
}

total=0
for repo in "${REPOS[@]}"; do
  zip=""
  for b in main master; do
    if dl "$repo" "$b" >/dev/null 2>&1; then zip="$OUT/_tmp/${repo//\//__}.zip"; break; fi
  done
  if [ -z "$zip" ] || [ ! -s "$zip" ]; then echo "SKIP $repo"; continue; fi
  d="$OUT/_tmp/${repo//\//__}"; mkdir -p "$d"
  unzip -qq -o "$zip" -d "$d" >/dev/null 2>&1 || true
  n=$(find "$d" -iname '*.docx' -type f ! -path '*/__MACOSX/*' | wc -l | tr -d ' ')
  # 复制（文件名加仓库前缀避免冲突；同名不同内容由 sha 去重）
  while IFS= read -r f; do
    h=$(shasum -a 256 "$f" | awk '{print $1}')
    dst="$OUT/${repo//\//__}__$h.docx"
    [ -f "$dst" ] || cp "$f" "$dst"
  done < <(find "$d" -iname '*.docx' -type f ! -path '*/__MACOSX/*')
  echo "$repo: docx=$n"
  rm -rf "$d" "$zip"
done
rm -rf "$OUT/_tmp"
echo "总计 docx: $(find "$OUT" -name '*.docx' | wc -l | tr -d ' ')"
