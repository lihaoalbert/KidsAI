# KidsAI Server (W4.5 B1)

**KidsAI Studio 的控制平面后端 — License 签发 + 学币 (学币) 记账, 不代理 LLM/Seedance 流量.**

桌面客户端直连 MiniMax / 火山方舟, 本服务只做:
- 设备激活 (返回 license JWT + API key 池)
- 学币余额查询 (防超额)
- 调用后异步上报 spend (幂等记账)
- License 续签 (server 可轮换 API key)
- 管理员发放学币 / 吊销 license

## 快速启动

```bash
cd kidsai-server
python3.13 -m venv .venv
.venv/bin/pip install -e ".[dev]"
cp .env.example .env   # 然后编辑 JWT_SECRET / ADMIN_TOKEN
.venv/bin/uvicorn kidsai_server.main:app --reload
```

## 7 个 endpoints

| Method | Path | 鉴权 |
|---|---|---|
| GET | /healthz | 无 |
| POST | /api/v1/devices/activate | 无 (fingerprint) |
| GET | /api/v1/me/balance | Bearer license |
| POST | /api/v1/me/record-spend | Bearer license |
| POST | /api/v1/me/refresh-license | Bearer license |
| POST | /api/v1/admin/devices/{id}/grant | X-Admin-Token |
| POST | /api/v1/admin/devices/{id}/revoke | X-Admin-Token |

## 测试

```bash
.venv/bin/pytest         # 28 tests (含 LLM cost + revoke 立即失效 + admin grant/revoke)
```

## 部署 (B3 — RHEL/AL4)

生产 ECS: Aliyun Linux 4 / Anolis 12 (RHEL 系, `dnf` 而非 `apt-get`).

**3 步上手:**

```bash
# 1) 安装依赖 + systemd 服务 + 健康检查
sudo bash deploy/install.sh

# 2) 装 nginx vhost (kids.ibi.ren 主站 + api.kids.ibi.ren 桌面端 API 域名)
sudo cp deploy/nginx-kids.ibi.ren.conf    /etc/nginx/conf.d/
sudo cp deploy/nginx-api.kids.ibi.ren.conf /etc/nginx/conf.d/
sudo nginx -t && sudo systemctl reload nginx

# 3) 上传证书到 /etc/nginx/ssl/{kids,api}.kids.ibi.ren.{pem,key}
# (证书在本地 Downloads/26041845_kids.ibi.ren_nginx.zip 等)
```

完整步骤 + 故障排查 + 续签见 [`deploy/RUNBOOK.md`](deploy/RUNBOOK.md).

**日常运维 CLI** (`deploy/kidsai-admin.py`):

```bash
export KIDSAI_SERVER_URL=https://api.kids.ibi.ren
export KIDSAI_ADMIN_TOKEN=<同 /etc/kidsai-server/.env>
python3 deploy/kidsai-admin.py list          # 查 device_id
python3 deploy/kidsai-admin.py grant <id> 50 # 发学币
python3 deploy/kidsai-admin.py revoke <id>   # 吊销 (立即生效)
```