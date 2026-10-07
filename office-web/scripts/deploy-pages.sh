#!/usr/bin/env bash
# 构建 office-viewer 演示站并发布到 Pages 分支（Gitee Pages / GitHub Pages 通用）。
#
# 用法：在仓库根目录执行  office-web/scripts/deploy-pages.sh
# 产出：分支 pages（内容 = office-web/dist-demo + README.md）
# 线上：https://hxuxiny.gitee.io/astorm-office/（Gitee 首次需在 仓库服务 → Gitee Pages
#        启动：部署分支 pages、目录 /；Gitee 免费版每次更新后需手动点"重新部署"）
#
# 辅助脚本失败即停（set -e），worktree 状态脏时会强制重建。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WT=/private/tmp/astorm-pages

cd "$ROOT/office-web"
npm run build:demo

cd "$ROOT"
git worktree remove --force "$WT" 2>/dev/null || true
git worktree prune
git worktree add -b pages "$WT" 2>/dev/null || git worktree add "$WT" pages

rm -rf "$WT"/*
rsync -a office-web/dist-demo/ "$WT"/
cat > "$WT"/README.md <<'EOF'
# office-viewer 演示站（Pages 发布分支）

由 `office-web/dist-demo` 构建发布；本地更新：

```bash
office-web/scripts/deploy-pages.sh
```
EOF

cd "$WT"
git add -A
if git diff --cached --quiet; then
  echo "pages 内容无变化，跳过提交"
else
  git commit -q -m "deploy: office-viewer 演示站更新（office-web dist-demo）"
  echo "已提交 pages 分支"
fi
cd "$ROOT"
echo "确认无误后执行：git push origin pages"
