# KidsAI Server 部署 Runbook (W4.5 B3)

> 一份给 lihao 看的部署 + 日常操作手册. 全程手敲命令, 无外部依赖 (除 ssh 到 ECS).

## 0. 前置清单 (一次)

| 项 | 说明 |
|---|---|
| ECS host | ibi.ren 项目里的同一台, Ubuntu 20.04+ / Debian 11+ |
| SSH 接入 | 由 ibi.ren 项目管, 不在这里复述 |
| 域名 | `kids.ibi.ren` 已 DNS 解析到 ECS 公网 IP |
| 证书 | `~/Downloads/26041845_kids.ibi.ren_nginx.zip` → `kids.ibi.ren.pem` + `kids.ibi.ren.key` |
| 密钥轮换 | W4.5 A1 暴露过的 MiniMax + 火山方舟 key **必须先轮换** 才能填进 `.env` |
| Provider 硬 cap | 火山方舟 / MiniMax 控制台给新 key 设 QPS + 每日 $ 上限 (见 §6) |

## 1. 上传代码

```bash
# 在 ibi.ren ECS 上 (假设 git 可用)
sudo mkdir -p /opt && sudo chown $USER /opt
cd /opt
git clone https://github.com/lihaoalbert/KidsAI.git kidsai-studio
cd kidsai-studio
git checkout main
# 只取 kidsai-server 子目录
sudo rsync -av --delete kidsai-server/ /opt/kidsai-server/
sudo chown -R root:root /opt/kidsai-server
```

## 2. 安装 Python 依赖

```bash
cd /opt/kidsai-server
sudo apt-get update && sudo apt-get install -y python3 python3-venv python3-dev build-essential
sudo python3 -m venv .venv
sudo .venv/bin/pip install --upgrade pip
sudo .venv/bin/pip install -e .
```

## 3. 准备 `.env` (root 700)

```bash
sudo mkdir -p /etc/kidsai-server
sudo cp deploy/.env.production.example /etc/kidsai-server/.env
sudo chmod 600 /etc/kidsai-server/.env

# 生成密钥
JWT_SECRET=$(openssl rand -hex 32)
ADMIN_TOKEN=$(openssl rand -hex 16)

# 编辑, 填 JWT_SECRET / ADMIN_TOKEN / LLM_API_KEY / SEEDANCE_API_KEY (4 个必填)
sudo -e /etc/kidsai-server/.env
```

`.env` 关键 4 项:
- `JWT_SECRET`: `openssl rand -hex 32`
- `ADMIN_TOKEN`: `openssl rand -hex 16`, 记下备用 (admin CLI 需要)
- `LLM_API_KEY`: 火山方舟 / MiniMax 控制台轮换后复制
- `SEEDANCE_API_KEY`: 同上

## 4. 安装 systemd unit

```bash
sudo bash deploy/install.sh
```

会自动:
- 创建 `kidsai` 系统用户
- 写 `/etc/systemd/system/kidsai-server.service`
- `systemctl enable --now kidsai-server`
- 启动后 `curl http://127.0.0.1:8080/healthz` 应返 `{"status":"ok"}`

## 5. 配置 nginx + 上证书

```bash
# 上传证书 (本机先解压 zip, scp 上传)
scp kids.ibi.ren.{pem,key} root@<ECS>:/tmp/
ssh root@<ECS> 'sudo mkdir -p /etc/nginx/ssl && \
  sudo mv /tmp/kids.ibi.ren.{pem,key} /etc/nginx/ssl/ && \
  sudo chmod 600 /etc/nginx/ssl/kids.ibi.ren.key && \
  sudo chown root:root /etc/nginx/ssl/kids.ibi.ren.*'

# 上传 vhost
scp deploy/nginx-kids.ibi.ren.conf root@<ECS>:/tmp/
ssh root@<ECS> 'sudo mv /tmp/nginx-kids.ibi.ren.conf /etc/nginx/sites-available/kids.ibi.ren && \
  sudo ln -sf /etc/nginx/sites-available/kids.ibi.ren /etc/nginx/sites-enabled/ && \
  sudo nginx -t && sudo systemctl reload nginx'
```

⚠️ nginx 是**增量**配置: 不动现有其他 site. 只新增 `kids.ibi.ren` vhost.

验证:
```bash
curl -fsS https://kids.ibi.ren/healthz
# 期望: {"status":"ok","version":"0.1.0"}
```

## 6. Provider 端硬 cap (lihao 控制台动作)

即使后端 + 桌面都做了 license check, **provider 端硬 cap 仍是最后一道防线** (用户机被破解后能绕过 license 直连 provider).

### 火山方舟 (Seedance) 控制台

1. 进入 API Key 管理 → 选中刚轮换的 Seedance key
2. 限流策略:
   - **QPS ≤ 5** (单台 1 次视频 1-2s 排队, 5 足够; 用户机 1 台难触顶)
   - **每日消费 ≤ ¥10** (单台 ≤ ¥5/天 = 留 100% 余量)
3. IP 白名单: 暂不设 (种子机 IP 散落, 设了反而挡正常用户)

### MiniMax 控制台 (LLM)

1. API Key 管理 → 选中轮换的 MiniMax key
2. 限流策略:
   - **QPS ≤ 10** (Agent 单 session 峰值约 3-5 QPS)
   - **每日消费 ≤ ¥5** (单台 ≤ ¥3/天 = 留 67% 余量)
3. 模型: `claude-haiku-4-5` (成本低, 适合种子阶段)

## 7. 日常操作

### 查看状态
```bash
sudo systemctl status kidsai-server
sudo journalctl -u kidsai-server -n 100 --no-pager
```

### 重启
```bash
sudo systemctl restart kidsai-server
```

### Admin CLI (在 ECS 本机或装了 Python 的本机)

```bash
# 装 admin CLI
pip install --user kidsai-admin  # 暂无, 直接 python3 deploy/kidsai-admin.py

# 配置 token
export KIDSAI_SERVER_URL=https://kids.ibi.ren
export KIDSAI_ADMIN_TOKEN=<第 3 步填的那个>

# 查设备
python3 deploy/kidsai-admin.py list --limit 20

# 给设备发 50 学币
python3 deploy/kidsai-admin.py grant <device_id> 50 --reason "种子用户启动"

# 吊销
python3 deploy/kidsai-admin.py revoke <device_id> --reason "退款"
```

### 备份 SQLite

```bash
sudo cp /opt/kidsai-server/data/kidsai.db /backup/kidsai-$(date +%Y%m%d).db
```

### 升级

```bash
cd /opt/kidsai-studio && git pull
sudo rsync -av --delete kidsai-server/ /opt/kidsai-server/
cd /opt/kidsai-server && sudo .venv/bin/pip install -e . --quiet
sudo systemctl restart kidsai-server
```

## 8. 故障排查

| 现象 | 排查 |
|---|---|
| `curl /healthz` 返 502 | `journalctl -u kidsai-server -n 50` 看启动错 (常见: `.env` 没读, JWT_SECRET 缺失) |
| `curl /healthz` 返 502 + nginx 502 | nginx upstream 配错 — `cat /etc/nginx/sites-enabled/kids.ibi.ren` 确认 `proxy_pass http://127.0.0.1:8080;` |
| `activate_device` 返 422 | `ADMIN_TOKEN` 没匹配; `journalctl -u kidsai-server` 看 startup 时 JWT_SECRET 长度 assert |
| 学币余额没刷新 | 前端 BalanceWidget 拉 /me/balance 失败 — 检查 KIDSAI_SERVER_URL 是否能在桌面进程读到 (Tauri 桌面进程的 env 由 launchd / systemd 继承自用户 shell) |
| Provider 返 429 / 配额耗尽 | 控制台检查硬 cap 是否触顶; 真触了说明有异常, 看 audit_log 找异常 device_id revoke |

## 9. 安全 checklist

- [ ] `.env` 是 `chmod 600`, owner `root:root`
- [ ] JWT_SECRET **不是** `.env.example` 里的占位符
- [ ] ADMIN_TOKEN **不是** `.env.example` 里的占位符
- [ ] 火山方舟 / MiniMax 控制台硬 cap 已设
- [ ] nginx 证书私钥 `chmod 600`
- [ ] 日志不包含 API key (FastAPI 默认不带 query string, OK)
- [ ] ECS SSH 只允许 key auth (无密码)