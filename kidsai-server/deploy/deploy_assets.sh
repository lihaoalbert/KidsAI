#!/usr/bin/env bash
# deploy_assets.sh — W6 D5 (在 Mac 本地跑)
#
# 把本地 assets/ (200 张预生成 PNG) 一键部署到 ECS:
#   1) rsync 到 /var/www/assets/ (nginx 静态根)
#   2) 装 nginx vhost (conf.d/...)
#   3) certbot 申请 DigiCert/Let's Encrypt DV 证书
#   4) nginx -t && systemctl reload
#   5) curl 验证 https://assets.kids.ibi.ren/asset-manifest.json
#
# 幂等: 重跑不会重复装, 也不会断传 (rsync --delete 同步增量)
#
# 前置:
#   - assets/ 已存在 (跑过: cd kidsai-server && python tools/generate_assets.py --execute)
#   - ECS 跑过 install.sh (kidsai user / systemd / /var/www/assets 目录已创建)
#   - 域名 assets.kids.ibi.ren 已 DNS 解析到 8.133.241.103
#   - SSH key 在 ~/Downloads/intfocus-albert.pem (见 RUNBOOK §1)
#
# 用法:
#   cd kidsai-server
#   ./deploy/deploy_assets.sh
#
# 环境变量覆盖:
#   ECS_HOST=root@8.133.241.103  ECS_SSH_KEY=~/path/to/key  ./deploy/deploy_assets.sh

set -euo pipefail

ECS_HOST="${ECS_HOST:-root@8.133.241.103}"
ECS_SSH_KEY="${ECS_SSH_KEY:-${HOME}/Downloads/intfocus-albert.pem}"
DEPLOY_DIR="$(cd "$(dirname "$0")" && pwd)"
# deploy/ 在 kidsai-server/deploy/, 上两级到 KidsAI repo 根 (assets/ 在那)
ROOT_DIR="$(cd "$DEPLOY_DIR/../.." && pwd)"

# assets/ 在仓库根 (kidsai-server/ 同级)
ASSETS_SRC="${ROOT_DIR}/assets"
ASSETS_KIND_DIRS=(character style bg storyboard icon home face_action bgm_thumb reserved)
ASSET_MANIFEST="$ASSETS_SRC/asset-manifest.json"

# 远程路径
REMOTE_TMP="/tmp/kidsai-assets-upload"
REMOTE_NGINX_DIR="/etc/nginx/conf.d"
REMOTE_ASSETS_ROOT="/var/www/assets"
REMOTE_VHOST="$DEPLOY_DIR/nginx-assets.kids.ibi.ren.conf"

# ─── 1. 前置检查 ────────────────────────────────────────
echo "==> [1/5] 前置检查"

if [[ ! -f "$ECS_SSH_KEY" ]]; then
  echo "ERROR: 找不到 SSH key: $ECS_SSH_KEY (默认 ~/Downloads/intfocus-albert.pem)" >&2
  exit 1
fi

if [[ ! -d "$ASSETS_SRC" ]]; then
  echo "ERROR: $ASSETS_SRC 不存在, 先跑 cd kidsai-server && python tools/generate_assets.py --execute" >&2
  exit 1
fi

if [[ ! -f "$ASSET_MANIFEST" ]]; then
  echo "ERROR: $ASSET_MANIFEST 不存在, 跑批脚本没成功" >&2
  exit 1
fi

if [[ ! -f "$REMOTE_VHOST" ]]; then
  echo "ERROR: $REMOTE_VHOST 不存在" >&2
  exit 1
fi

SSH_KEY_OPT=(-i "$ECS_SSH_KEY")
SSH_BASE=(ssh "${SSH_KEY_OPT[@]}" -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/dev/null)
SCP_BASE=(scp "${SSH_KEY_OPT[@]}" -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/dev/null)

# ─── 2. 资产打包上传 ──────────────────────────────────────
# 用 tar + scp (服务器端通常没装 rsync, 走 tar 更通用)
echo "==> [2/5] tar+scp ${ASSETS_SRC}/ → ${ECS_HOST}:${REMOTE_ASSETS_ROOT}/"
"${SSH_BASE[@]}" "$ECS_HOST" "sudo mkdir -p '$REMOTE_ASSETS_ROOT' && sudo chown root:nginx '$REMOTE_ASSETS_ROOT' && sudo chmod 755 '$REMOTE_ASSETS_ROOT'"

# 打包 (排除 _failed.json / 中间文件, 已 gitignored 但保险起见)
TMP_TAR="/tmp/kidsai-assets.tar.gz"
tar -czf "$TMP_TAR" \
    --exclude='_failed.json' \
    -C "$ASSETS_SRC" .

"${SCP_BASE[@]}" "$TMP_TAR" "$ECS_HOST:/tmp/kidsai-assets.tar.gz"
rm -f "$TMP_TAR"

"${SSH_BASE[@]}" "$ECS_HOST" "\
  set -e; \
  sudo rm -rf '$REMOTE_ASSETS_ROOT'/* '$REMOTE_ASSETS_ROOT'/.[!.]* 2>/dev/null || true; \
  sudo tar -xzf /tmp/kidsai-assets.tar.gz -C '$REMOTE_ASSETS_ROOT' && \
  sudo chown -R root:nginx '$REMOTE_ASSETS_ROOT' && \
  sudo find '$REMOTE_ASSETS_ROOT' -type f -exec chmod 644 {} + && \
  sudo find '$REMOTE_ASSETS_ROOT' -type d -exec chmod 755 {} + && \
  sudo rm -f /tmp/kidsai-assets.tar.gz"
echo "  ✓ 资产已传完 (并清理 _failed.json)"

# ─── 3. nginx vhost ─────────────────────────────────────
echo "==> [3/5] 装 nginx vhost"
"${SCP_BASE[@]}" "$REMOTE_VHOST" "$ECS_HOST:/tmp/nginx-assets.kids.ibi.ren.conf"
"${SSH_BASE[@]}" "$ECS_HOST" "sudo cp /tmp/nginx-assets.kids.ibi.ren.conf ${REMOTE_NGINX_DIR}/ && sudo rm -f /tmp/nginx-assets.kids.ibi.ren.conf"
echo "  ✓ vhost 拷进 /etc/nginx/conf.d/"

# ─── 4. certbot + reload ───────────────────────────────
echo "==> [4/5] certbot 申请证书 (Let's Encrypt DV, --nginx 自动改配置)"

# 先看证书在不在, 决定是 certonly 还是 --nginx (含自动 80→443 重定向)
CERT_PATH="/etc/letsencrypt/live/assets.kids.ibi.ren/fullchain.pem"
CERT_EXISTS=$("${SSH_BASE[@]}" "$ECS_HOST" "sudo test -f '$CERT_PATH' && echo yes || echo no")

if [[ "$CERT_EXISTS" == "yes" ]]; then
  echo "  ✓ 证书已存在, 跳过 certbot (cert: $CERT_PATH)"
else
  echo "  → certbot --nginx (申请 + 自动 80→443)"
  "${SSH_BASE[@]}" "$ECS_HOST" "sudo certbot --nginx -d assets.kids.ibi.ren --non-interactive --agree-tos --email lihao.albert@gmail.com --redirect"
fi
echo "  ✓ 证书就绪"

# ─── 5. reload + 验证 ──────────────────────────────────
echo "==> [5/5] nginx -t && reload && 验证"
"${SSH_BASE[@]}" "$ECS_HOST" "sudo nginx -t && sudo systemctl reload nginx"
sleep 2

# 验证: 不带 -k (严格 HTTPS, 证书错就 fail)
echo
echo "==> curl 验证 https://assets.kids.ibi.ren/asset-manifest.json"
HTTP_CODE=$(curl -s -o /tmp/asset-manifest.head -w '%{http_code}' "https://assets.kids.ibi.ren/asset-manifest.json" || echo "FAIL")
if [[ "$HTTP_CODE" == "200" ]]; then
  echo "  ✅ HTTP 200"
  echo "  header 内容:"
  head -c 200 /tmp/asset-manifest.head
  echo
  echo "  → 后续: 前端 assetStore 自动 GET 这个 URL, 拿到 image 索引"
else
  echo "  ❌ HTTP $HTTP_CODE, 检查: ssh ${ECS_HOST} 'sudo nginx -t && sudo tail -50 /var/log/nginx/error.log'"
  exit 1
fi

echo
echo "==> ✅ 部署完成"
echo "  - 资产 URL: https://assets.kids.ibi.ren/<kind>/<key>.<ext>"
echo "  - Manifest URL: https://assets.kids.ibi.ren/asset-manifest.json"
echo "  - 200 张 PNG 总 ~50MB, 服务器端 nginx 直出, 无需后端 service 介入"
