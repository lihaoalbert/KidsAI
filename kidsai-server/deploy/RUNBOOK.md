# KidsAI Server 部署 Runbook (W4.5 B3)

> 目标: 在 ECS (Aliyun Linux 4 / Anolis 12, RHEL 系) 上跑通 kidsai-server systemd 服务,
> nginx 反代到 `https://kids.ibi.ren` 和 `https://api.kids.ibi.ren`, 桌面端走 `https://api.kids.ibi.ren` 作为 `KIDSAI_SERVER_URL`.

**前提:**
- ECS: 8.133.241.103 (root 权限, ssh key 在 `~/Downloads/intfocus-albert.pem`)
- OS: `dnf` 包管理器 (Aliyun Linux 4 / Anolis 12 / RHEL 8+), **不是** apt/Ubuntu/Debian
- nginx 路径: `/etc/nginx/conf.d/*.conf` (RedHat 风格), **没有** sites-available/sites-enabled
- 域名: `kids.ibi.ren` + `api.kids.ibi.ren` (均已在阿里云解析到 ECS, DV 证书 2026-10 到期)

---

## §1 一键安装 (新 ECS / 重装)

```bash
cd /opt/kidsai-server  # 已 git clone 到这
sudo bash deploy/install.sh
```

`install.sh` 做的事情:
1. 校验 root + dnf
2. 创建 `kidsai` 系统用户 (`useradd --system`)
3. 准备 `/opt/kidsai-server/data`, `/var/log/kidsai-server`, `/etc/kidsai-server`
4. 强制要求 `/etc/kidsai-server/.env` 已存在 (`deploy/.env.production.example` 模板)
5. 装 `kidsai-server.service` + `systemctl enable --now`
6. 校验 `curl http://127.0.0.1:8080/healthz`

**重要: `.env` 不在 git 里**, 由 lihao 在本地编辑后用 `scp` 或如下命令传:

```bash
# 在 Mac 上, scp .env 到 ECS
scp -i ~/Downloads/intfocus-albert.pem /path/to/.env \
    root@8.133.241.103:/etc/kidsai-server/.env

# 在 ECS 上
sudo chmod 600 /etc/kidsai-server/.env
sudo chown root:kidsai /etc/kidsai-server/.env
```

---

## §2 nginx vhost (HTTPS 反代)

将以下两个文件复制到 `/etc/nginx/conf.d/`:

```bash
sudo cp deploy/nginx-kids.ibi.ren.conf    /etc/nginx/conf.d/
sudo cp deploy/nginx-api.kids.ibi.ren.conf /etc/nginx/conf.d/
```

### §2.1 证书上传

```bash
# 在 Mac 上, 两个 .zip 已下载:
# 26041845_kids.ibi.ren_nginx.zip    → kids.ibi.ren
# 26043757_api.kids.ibi.ren_nginx.zip → api.kids.ibi.ren
unzip -p 26041845_kids.ibi.ren_nginx.zip '*' > /tmp/kids.tgz
# 解出 _nginx_bundle.pem / .key 上传

sudo mkdir -p /etc/nginx/ssl
scp -i ~/Downloads/intfocus-albert.pem \
    /path/to/kids.ibi.ren.pem   root@8.133.241.103:/etc/nginx/ssl/
scp -i ~/Downloads/intfocus-albert.pem \
    /path/to/api.kids.ibi.ren.pem root@8.133.241.103:/etc/nginx/ssl/
# .key 同理
sudo chmod 600 /etc/nginx/ssl/*.key
```

### §2.2 校验 + reload

```bash
sudo nginx -t
sudo systemctl reload nginx
curl -fsS https://kids.ibi.ren/healthz   # {"status":"ok",...}
curl -fsS https://api.kids.ibi.ren/healthz
```

---

## §3 .env 密钥准备

```bash
# 1) 生成新 JWT_SECRET (32 字节随机)
openssl rand -hex 32

# 2) 生成 ADMIN_TOKEN (16 字节, 易记也可手填)
openssl rand -hex 16

# 3) 抄到 .env
cp deploy/.env.production.example /etc/kidsai-server/.env
$EDITOR /etc/kidsai-server/.env    # 填 JWT_SECRET / ADMIN_TOKEN / LLM_API_KEY / SEEDANCE_API_KEY
chmod 600 /etc/kidsai-server/.env
```

**API key 安全提醒 (W4.5 A1):**
- LLM_API_KEY = MiniMax 控制台, **不复用** `docs/00-账号信息/code.md` 旧 key (已 git rm)
- SEEDANCE_API_KEY = 火山方舟控制台, 同上轮换
- **不要把 key 写到 git/对话/截图**, 用 HTTPS 一次性传输

---

## §4 Provider 端硬 cap (防破产兜底)

⚠️ 这是**最关键**的安全层 — 用户机被破解不会让我们烧穿钱包:

### MiniMax 控制台
- API Key 详情页 → "使用限制" → 设 QPS ≤ 10, 每日消费 ≤ ¥5

### 火山方舟控制台
- Seedance API Key 详情 → QPS ≤ 5, 每日消费 ≤ ¥10

> 单台 ≤ ¥5/天 × 10 万台 = ¥50 万/天**上限可控**, 不会爆雷.

---

## §5 日常操作

### 重启服务
```bash
sudo systemctl restart kidsai-server
sudo journalctl -u kidsai-server -f -n 100
```

### 发学币 (种子用户启动)
```bash
# 在 Mac 上 (kidsai-admin.py 通过 HTTPS 调 admin API)
export KIDSAI_SERVER_URL=https://api.kids.ibi.ren
export KIDSAI_ADMIN_TOKEN=<填 .env 里的 ADMIN_TOKEN>
python3 deploy/kidsai-admin.py grant <device_id> 50 --reason "种子启动"
python3 deploy/kidsai-admin.py list --limit 10   # 查 device_id
```

### 吊销设备 (退款 / 异常)
```bash
python3 deploy/kidsai-admin.py revoke <device_id> --reason "退款"
# revoke 后已签发的 license_token 立刻 401 (assert_device_active 查 devices.revoked_at)
```

### 查看学币流水
```bash
ssh root@8.133.241.103 'sqlite3 /opt/kidsai-server/data/kidsai.db \
    "SELECT kind, amount, call_id, reason, datetime(created_at/1000,\"unixepoch\") \
     FROM transactions WHERE device_id=\"<dev>\" ORDER BY created_at DESC LIMIT 20"'
```

---

## §6 证书续签 (到期: 2026-10-09)

两个 DigiCert DV 证书 (kids.ibi.ren + api.kids.ibi.ren) 均 2026-10-09 到期.

**续签前置:** 在阿里云 SSL 证书控制台 **提前 30 天** 申请续签 → 下载新的 nginx 包 (`*_nginx.zip`) → 按 §2.1 替换文件.

```bash
# 续签当天 (替换 + reload)
scp -i ~/Downloads/intfocus-albert.pem new_kids.pem \
    root@8.133.241.103:/etc/nginx/ssl/kids.ibi.ren.pem
scp -i ~/Downloads/intfocus-albert.pem new_kids.key \
    root@8.133.241.103:/etc/nginx/ssl/kids.ibi.ren.key
ssh root@8.133.241.103 'sudo nginx -t && sudo systemctl reload nginx && \
    echo | openssl s_client -connect kids.ibi.ren:443 -servername kids.ibi.ren 2>/dev/null | \
    openssl x509 -noout -dates'
```

> 阿里云 DV 证书免费, 一年一续, **不接 certbot 自动续签** (DV 已经够用, ACME 流程反而引入风险).

---

## §7 故障排查

| 症状 | 查 |
|---|---|
| `https://kids.ibi.ren/healthz` 502 | `systemctl status kidsai-server` / `journalctl -u kidsai-server -n 50` |
| 401 invalid license | desktop `license.json` 时间戳过期 → `OnboardingPage` 重激活; 或查 server `.env` JWT_SECRET 是否变更 |
| 学币扣错 | `sqlite3 /opt/kidsai-server/data/kidsai.db "SELECT * FROM transactions WHERE ..."` |
| ECS 端 LLM/Seedance 调用慢 | `journalctl -u kidsai-server` (我们不代理, 只看 license 自身) |
| nginx reload 失败 | `nginx -t` 看语法错; 大概率是 `.pem` 文件名错或权限 |

---

## §8 不在范围 (后续 plan E)

- E1 家长端小程序 (原型见 `docs/05-家长端/01-家长端小程序原型.md`)
- E2 Apple/Win 代码签名 (种子用户接受 Gatekeeper 警告)
- E3 Sentry/OpenTelemetry
- E4 Postgres 替换 SQLite (设备 > 1 万时)
- E5 per-device API key 拆池 (目前是单一共享 key)
