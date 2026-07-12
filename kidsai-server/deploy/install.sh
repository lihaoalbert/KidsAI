#!/usr/bin/env bash
# KidsAI Server 一键部署 (W4.5 B3 v2 — RHEL/Anolis 风格)
#
# 适配 Aliyun Linux 4 / Anolis 12 (dnf 而非 apt-get, /etc/nginx/conf.d 而非 sites-available)
# 幂等: 多次运行结果一致
# 需要 root (sudo) 权限 — 装 systemd unit, 创建 user, 写 /etc 目录
#
# 用法:
#   sudo bash install.sh
#
# 假设:
# - 仓库已 git clone 到 /opt/kidsai-server (或 rsync 同步)
# - /etc/kidsai-server/.env 已由 lihao 编辑好 (见 deploy/.env.production.example)
# - nginx 已装 (ibiren 项目已用 dnf install nginx 装过)
# - 已上传证书到 /etc/nginx/ssl/{kids,api}.kids.ibi.ren.{pem,key} (见 deploy/RUNBOOK.md §5)

set -euo pipefail

APP_DIR=/opt/kidsai-server
ENV_FILE=/etc/kidsai-server/.env
SERVICE_FILE=/etc/systemd/system/kidsai-server.service
RUN_USER=kidsai

echo "==> [1/7] 检查 root 权限"
if [[ $EUID -ne 0 ]]; then
  echo "ERROR: 需要 root, 用 'sudo $0'" >&2
  exit 1
fi

echo "==> [2/7] 检测 OS (dnf 走 RHEL/Anolis 路径)"
if ! command -v dnf >/dev/null 2>&1; then
  echo "ERROR: 找不到 dnf, 只支持 RHEL/Aliyun/Anolis 系" >&2
  exit 1
fi

echo "==> [3/7] 创建系统用户 (${RUN_USER})"
if ! id -u "${RUN_USER}" >/dev/null 2>&1; then
  useradd --system --home "${APP_DIR}" --shell /usr/sbin/nologin "${RUN_USER}"
fi

echo "==> [4/7] 准备目录"
mkdir -p "${APP_DIR}/data" /var/log/kidsai-server /etc/kidsai-server /var/www/assets
chown -R "${RUN_USER}:${RUN_USER}" "${APP_DIR}/data" /var/log/kidsai-server
chmod 750 /etc/kidsai-server  # parent dir 让 kidsai user 可读 (它要读 .env)
# W6 B2: 资产静态托管 root (nginx 读, 不需要 server user)
chmod 755 /var/www/assets

echo "==> [5/7] 检查 .env"
if [[ ! -f "${ENV_FILE}" ]]; then
  echo "ERROR: ${ENV_FILE} 不存在"
  echo "  1) sudo cp deploy/.env.production.example ${ENV_FILE}"
  echo "  2) sudo chmod 600 ${ENV_FILE}"
  echo "  3) sudo \${EDITOR:-nano} ${ENV_FILE}  # 填 JWT_SECRET / ADMIN_TOKEN / 真实 API keys"
  echo "  4) 重跑本脚本"
  exit 1
fi
chmod 600 "${ENV_FILE}"
chown root:"${RUN_USER}" "${ENV_FILE}" 2>/dev/null || chown root:root "${ENV_FILE}"

echo "==> [6/7] 装 systemd unit"
cp "$(dirname "$0")/kidsai-server.service" "${SERVICE_FILE}"
chmod 644 "${SERVICE_FILE}"
systemctl daemon-reload

echo "==> [7/7] 启动服务"
systemctl enable kidsai-server
systemctl restart kidsai-server
sleep 2
systemctl status kidsai-server --no-pager -l || true

echo
echo "==> 验证"
if curl -fsS http://127.0.0.1:8080/healthz >/dev/null; then
  echo "  ✅ /healthz 通过 (127.0.0.1:8080)"
else
  echo "  ❌ /healthz 失败, journalctl -u kidsai-server -n 50 --no-pager"
  exit 1
fi

echo
echo "==> 下一步"
echo "  1) 配置 nginx (见 deploy/RUNBOOK.md §5) — 现成 ibiren 项目已 nginx, 增量加 conf.d"
echo "  2) 验证 HTTPS:  curl https://kids.ibi.ren/healthz"
echo "  3) 用 deploy/kidsai-admin.py 给设备 grant 学币"
echo "  4) W6 B2: 上传证书到 /etc/nginx/ssl/assets.kids.ibi.ren.{pem,key}"
echo "     然后 cp deploy/nginx-assets.kids.ibi.ren.conf /etc/nginx/conf.d/ && nginx -t && systemctl reload nginx"
echo "  5) W6 B1: 用 tools/generate_assets.py 跑批, 产物 scp 到 /var/www/assets/"
