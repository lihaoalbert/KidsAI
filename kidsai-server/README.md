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
.venv/bin/pytest         # 23 tests
```

## 部署 (B3)

生产环境由 systemd EnvironmentFile 注入密钥到 `/etc/kidsai-server/.env` (root 700),
nginx `api.kids.ibi.ren` 反代 `http://127.0.0.1:8080`.

详见仓库根 plan: `W4.5 种子用户启动 (License + 直连架构)`.