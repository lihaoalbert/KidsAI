#!/usr/bin/env bash
# KidsAI Server 一键部署 (W4.5 B3)
#
# 幂等: 多次运行结果一致
# 需要 root (sudo) 权限 — 安装 systemd unit, 创建 user, 写 /etc 目录
#
# 用法:
#   sudo bash install.sh
#
# 假设:
# - 仓库已 git clone 到 /opt/kidsai-server
# - /etc/kidsai-server/.env 已由 lihao 编辑好 (见 deploy/.env.production.example)
# - nginx + ssl 证书已就绪 (见 deploy/nginx-kids.ibi.ren.conf)
# - systemd 已可用 (Ubuntu 20.04+ / Debian 11+)

set -euo pipefail

APP_DIR=/opt/kidsai-server
ENV_FILE=/etc/kidsai-server/.env
SERVICE_FILE=/etc/systemd/system/kidsai-server.service
RUN_USER=kidsai

echo "==> [1/6] 检查 root 权限"
if [[ $EUID -ne 0 ]]; then
  echo "ERROR: 需要 root, 用 'sudo $0'" >&2
  exit 1
fi

echo "==> [2/6] 创建系统用户 (${RUN_USER})"
if ! id -u "${RUN_USER}" >/dev/null 2>&1; then
  useradd --system --home "${APP_DIR}" --shell /usr/sbin/nologin "${RUN_USER}"
fi

echo "==> [3/6] 准备目录"
mkdir -p "${APP_DIR}/data" /var/log/kidsai-server
chown -R "${RUN_USER}:${RUN_USER}" "${APP_DIR}/data" /var/log/kidsai-server

echo "==> [4/6] 检查 .env"
if [[ ! -f "${ENV_FILE}" ]]; then
  echo "ERROR: ${ENV_FILE} 不存在"
  echo "  1) sudo cp deploy/.env.production.example ${ENV_FILE}"
  echo "  2) sudo chmod 600 ${ENV_FILE}"
  echo "  3) sudo ${EDITOR:-nano} ${ENV_FILE}  # 填 JWT_SECRET / ADMIN_TOKEN / 真实 API keys"
  echo "  4) 重跑本脚本"
  exit 1
fi
chmod 600 "${ENV_FILE}"
chown root:"${RUN_USER}" "${ENV_FILE}" 2>/dev/null || chown root:root "${ENV_FILE}"

echo "==> [5/6] 安装 systemd unit"
cp "$(dirname "$0")/kidsai-server.service" "${SERVICE_FILE}"
chmod 644 "${SERVICE_FILE}"
systemctl daemon-reload

echo "==> [6/6] 启动服务"
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
echo "  1) 配置 nginx (见 deploy/nginx-kids.ibi.ren.conf)"
echo "  2) 验证 HTTPS:  curl https://kids.ibi.ren/healthz"
echo "  3) 用 deploy/kidsai-admin.py 给设备 grant 学币"