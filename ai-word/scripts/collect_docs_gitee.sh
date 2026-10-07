#!/usr/bin/env bash
# 从 Gitee 镜像下载"docx 富集"仓库的归档 zip，解出全部 .docx，
# 按 sha256 去重后汇集到数据集目录。原始数据不纳入版本库。
# 用法: scripts/collect_docs_gitee.sh [OUT]  (默认 /tmp/docx_dataset)
set -uo pipefail
OUT="${1:-/tmp/docx_dataset}"
mkdir -p "$OUT" "$OUT/_tmp"

# Gitee 镜像名:分支（镜像分支与上游不同）
REPOS=(
  "pandoc:main"
  "Open-XML-SDK:main"
  "docx4j:master"
  "poi-tl:master"
  "python-docx:master"
)

total=0
for entry in "${REPOS[@]}"; do
  repo="${entry%%:*}"
  branch="${entry##*:}"
  zip="$OUT/_tmp/${repo}.zip"
  url="https://gitee.com/mirrors/$repo/repository/archive/$branch.zip"
  if ! curl -fsSL -m 600 -o "$zip" "$url"; then
    echo "SKIP $repo (download failed)"
    continue
  fi
  if [ ! -s "$zip" ]; then echo "SKIP $repo (empty)"; continue; fi
  d="$OUT/_tmp/$repo"
  mkdir -p "$d"
  unzip -qq -o "$zip" -d "$d" >/dev/null 2>&1 || true
  n=0
  while IFS= read -r f; do
    h=$(shasum -a 256 "$f" | awk '{print $1}')
    dst="$OUT/${repo}__$h.docx"
    if [ ! -f "$dst" ]; then cp "$f" "$dst"; n=$((n+1)); fi
  done < <(find "$d" -iname '*.docx' -type f ! -path '*/__MACOSX/*')
  total=$((total + n))
  echo "$repo: +$n docx"
  rm -rf "$d" "$zip"
done
rm -rf "$OUT/_tmp"
echo "数据集 docx 总数: $(find "$OUT" -maxdepth 1 -name '*.docx' | wc -l | tr -d ' ')"
