# 更新日志

所有重要的项目变更都会记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### Added (W6 — MiniMax 多能力 + 资产库 + Token 池)
- **后端 MiniMax key 池 (A)**: `device_key_assignment` 表 + sticky 选 key + admin rotate endpoint
- **桌面 4 新 tool (C)**: image-01 立绘 / T2A v2 旁白 / voice_clone 训练 / music-01 BGM
- **Hailuo 视频备用 (C4)**: `HailuoVideoAdapter` + 引擎选择器 (`videoEngine` 状态)
- **资产批跑 (B1)**: `kidsai-server/tools/generate_assets.py` + 200 条 yaml spec
- **assets.kids.ibi.ren 静态托管 (B2)**: 新子域 + nginx vhost + manifest API
- **前端 assetStore (B3)**: zustand store + sessionStorage 缓存 + picsum fallback 替换
- **关卡 cover banner (E1)**: L1-L7 manifest 真图占顶
- **声音复刻入口 (E2)**: VoiceClonePicker 弹窗
- **视频引擎选择器 (E3)**: VideoEnginePicker 弹窗
- **消耗看板 (E4)**: `GET /api/v1/me/spend-summary` + byKind 分组
- **真机集成测试 (D2)**: 3 个 `--ignored` 测试 (~¥6 + 23 学币)
- **学币计费扩展 (A1)**: 4 新 kind (image_gen=5 / voice_clone=10 / music_gen=8 / hailuo_video=12)
- **总测试**: 60 backend + 119 frontend + 166 Rust = 345 pass + 4 ignored

### Added
- Week 1 项目骨架：Tauri 2.0 + React 18 + Vite + TypeScript
- 侧边栏导航（课程中心 / 作品工坊 / 作品库 / 我的 Agent）
- 4 个空页面（Home / Workshop / Library / MyAgent）
- 基础组件库 v0.1：Button / Input / Card
- Storybook 8 组件文档
- GitHub Actions CI/CD 流水线
  - 前端 lint + build 验证
  - macOS / Windows / Linux Tauri 多平台构建
  - 内测版自动构建（main 分支）
- 应用图标占位（紫底 K 字）

### Added (Week 2)
- **W2.1 关卡数据模型**
  - `shared/types/level.ts`：Level / LevelStep / ScoringCriteria / LevelProgress / LevelSubmission / AgentOutput 类型
  - `src/data/levels.ts`：5 个 MVP 关卡 (L1-L5) 静态数据 + getLevel / getAvailableLevels
  - `src/pages/LevelDetailPage.tsx`：关卡详情页（封面 / 步骤 / 评分维度 / 前置依赖 / 开始按钮）
  - 路由：HomePage 卡片可点击进入详情页
- **W2.2 Tauri 命令框架 + Zustand stores**
  - `src-tauri/src/types.rs`：Rust 端 Level/LevelStep/ScoringCriteria/LevelProgress
  - `src-tauri/src/content.rs`：5 个内置关卡（与前端数据保持一致）
  - `src-tauri/src/levels.rs`：list_levels / get_level / list_progress / start_level / submit_level / completed_level_ids 命令（in-memory 存储，W2.3 换 SQLite）
  - `src-tauri/src/agent.rs`：run_agent 命令（W2.4 实现真实循环）
  - `src/api/tauri.ts`：前端命令封装
  - `src/stores/levelStore.ts`：关卡状态 + 解锁判断
  - `src/stores/agentStore.ts`：Agent 会话 / 消息流
  - `src/stores/tokenStore.ts`：学币余额
- **W2.3 本地 SQLite 集成**
  - `Cargo.toml`：新增 `rusqlite = { version = "0.32", features = ["bundled"] }`（无系统依赖）
  - `src-tauri/src/db.rs`：单例 `Db`（Mutex<Connection>），启动时自动 migrate
    - `level_progress`：原子 UPSERT（attempts+1），mark_completed 取 max(best_score)
    - `creations`：用户输入 + Agent 输出 JSON
    - `assets`：每个作品的生成资产（image / video / audio）
    - 时间戳统一存毫秒（INTEGER）
  - `src-tauri/src/levels.rs`：改用 Db 存储，命令签名不变
  - `src-tauri/src/creations.rs`：`save_creation` / `list_creation` s 命令
  - `src-tauri/src/lib.rs`：启动时打开 `~/Library/Application Support/com.kidsai.studio/kidsai.db`（macOS）
  - `src/api/tauri.ts`：新增 `saveCreation` / `listCreations` 类型
  - `src/pages/LibraryPage.tsx`：接入真实作品数据
  - `src-tauri/tests/db_smoke.rs`：4 个集成测试（open / upsert / mark_completed / creations+assets）全部通过
- **W3.1 真实 LLM 集成（MiniMax-M3 + 备选 OpenAI 兼容 provider）**
  - `Cargo.toml`：新增 `reqwest`（rustls-tls，无系统 OpenSSL 依赖）+ `tokio` + `dotenvy`
  - `src-tauri/src/model_openai.rs`：`OpenAiCompatible` 适配器，调用 `/v1/chat/completions` + tool calling
  - `src-tauri/src/model_factory.rs`：按环境变量选 provider
    - 优先级：MINIMAX > DEEPSEEK > OPENAI > DASHSCOPE > mock
    - 默认模型：MiniMax-M3
  - `src-tauri/src/lib.rs`：新增 `current_model_source` 命令，前端可展示
  - `.env.example`：4 个 provider 的配置模板
  - **tool_call_id 闭环**：新增 `ModelMessage.tool_calls` + `ModelMessage.tool_call_id` 字段，agent loop 把上一轮的 tool_call 结构塞回历史，下游 MiniMax 严格校验
  - **`tokens_used` 累加**：`AgentRunResponse.tokens_used` 字段透出每轮 token 消耗，前端可展示
  - **dotenv 加载收敛到入口**：`select_model` 不再内部调 `dotenvy::dotenv()`，改由 `run_agent` / `current_model_source` 命令加载，避免测试 `remove_var` 后被重置
  - `safety.rs`：max_len 从 500 → 5000，真实 LLM 教学回答常超 500 字
  - `reqwest::Client`：timeout 60s → 180s（3 轮 L1 串行 ~30-120s）
  - 公开 `parse_decision_from_response` 供测试，不依赖网络
  - 测试
    - `tests/openai_parse.rs` 7 个集成测试（mock fallback / minimax 选中 / DeepSeek 真实响应 JSON 解析 / final answer 解析 / 无 usage 兜底 / 构造校验 / 端到端 mock L1）
    - `tests/real_llm.rs` 1 个真实 LLM 测试（仅当 `.env` 有 `MINIMAX_API_KEY` 时跑），用 L1 真实 system_prompt + "演示模式"覆盖，验证 tool_call 链路 + 3 步 ReAct
  - **总计 21 个测试全过（5 + 4 + 7 + 1 + 4 单元）**
- **W2.4 + W2.5 + W2.6 + W2.7 + W2.8 Agent Loop 核心 + 端到端 demo**
  - 后端 Agent Loop
    - `src-tauri/src/model.rs`：`Model` trait + `ModelRouter`（多模型路由接口，W3 接 LiteLLM）
    - `src-tauri/src/model_mock.rs`：Mock 模型，按关卡+轮次生成 ReAct 轨迹
    - `src-tauri/src/tools.rs`：`Tool` trait + `ToolRegistry`，6 个 mock MCP 工具
      - `generate_image` / `image_to_video` / `synthesize_speech` / `add_subtitle` / `add_bgm` / `text_chat`
    - `src-tauri/src/agent.rs`：ReAct 循环 + 事件流（`agent://event` 通道）
      - `EventSink` trait 抽象：`TauriEventSink`（生产）/ `NoopEventSink`（测试）
      - 入口 + 出口双重 `KeywordFilter` 审核（W2.7）
      - 工具白名单强制（防越权）
      - MAX_STEPS=6 保护
    - `src-tauri/src/safety.rs`：关键词审核（pass / warn / block 三态）
  - 后端测试
    - `src-tauri/src/safety.rs`：4 个单元测试（pass / block / warn / too-long）
    - `src-tauri/tests/agent_smoke.rs`：5 个集成测试（L1 轨迹 / L2 轨迹 / 屏蔽输入 / 事件流顺序 / 警告不阻断）
    - 总计 13 个测试全部通过
  - 前端
    - `src/api/tauri.ts`：新增 `checkSafety` / `onAgentEvent` + `AgentEvent` 类型
    - `src/stores/agentStore.ts`：订阅 `agent://event`，把 Thought / ToolCall / ToolResult 实时落进 messages
    - `src/pages/AgentRunnerPage.tsx`：关卡运行页
      - 左：任务说明 + 评分维度
      - 中：对话流（user / assistant / thought / tool 四种气泡）+ 输入框
      - 资产展示区（image / video / audio）
      - "提交并查看评分"按钮：调 submit_level + save_creation
    - `src/App.tsx`：新增 `runner` 路由

### Added (Week 3.2)
- **流式输出 + 取消（W3.2）**
  - `Cargo.toml`：`async-trait = "0.1"` + `futures-util = "0.3"` + `reqwest` 加 `stream` feature
  - `src-tauri/src/model.rs`：`Model` trait 改 async（`async_trait`），`decide_stream` 返回 `(ModelDecision, Vec<Chunk>)`；新增 `Chunk` 结构
  - `src-tauri/src/model_openai.rs`：删 `block_on` 反模式；`SseParser` 解析 SSE（`data: {json}\n\n` 事件 + `[DONE]` 终止符）；`HashMap<usize, ToolBuf>` 按 index 累积 tool_call 碎片（id / name / args）；`tokio::select!` 跑 `bytes_stream` + 50ms cancel 轮询
  - `src-tauri/src/model_mock.rs`：`MockConfig { chunks, final_answer, tool_call, chunk_delay_ms, cancel_flag }`；`with_config()` 构造；`emit_configured` 模拟流式 + 尊重 cancel
  - `src-tauri/src/agent.rs`：
    - 新增 `SessionRegistry`（`Mutex<HashMap<String, Arc<AtomicBool>>>`），`app.manage()` 注册
    - `run_loop` 改 async；插入 registry 后建 `RegistryGuard`（Drop 时自动清理）
    - 新增 `AgentEvent::Chunk { session_id, step, delta }` + `AgentEvent::Cancelled { session_id }`
    - `AgentRunResponse` 加 `cancelled: bool` 字段
    - 取消检测：step 间 + 工具执行后 + model 内部 chunk 间（三层防御）
    - `EventSink: Send + Sync`（async 跨 await 需要）
  - `src-tauri/src/lib.rs`：`cancel_agent` Tauri 命令（`app.state::<SessionRegistry>()` 取注册表）+ `manage(SessionRegistry::default())` + 注册到 invoke_handler
  - `src-tauri/src/test_helpers.rs`：3 个 helper 改 async + 新增 `run_agent_stream_with_model` 暴露 registry + `CollectingSink::chunk_deltas()` helper
  - 测试
    - `tests/agent_smoke.rs` 5 个测试加 `#[tokio::test]` + `.await`（向后兼容）
    - `tests/agent_stream.rs` 4 个新测试：
      - `mock_emits_five_chunks_then_final_answer` — 5 个 chunk + 1 final_answer，事件顺序断言
      - `cancel_mid_stream_emits_cancelled_event` — chunk_delay_ms=200，50ms 后 cancel，断言收到 Cancelled 事件 + `response.cancelled=true`
      - `cancel_between_steps_emits_cancelled_event` — 工具执行完、下一轮 chunk 期间取消
      - `tool_call_produces_no_chunks` — 工具调用不产生 chunk 事件（buffer 在内部）
    - `tests/real_llm.rs` / `openai_parse.rs` 改 async
  - **总计 25 个测试全过（4 单元 + 5 + 4 + 4 + 7 + 1 真实 LLM）**
  - **未实现**：前端 UI 集成（W3.3）— 把 `Chunk` 事件渲染到聊天框 + 取消按钮

### Added (Week 3.3)
- **前端流式集成 + 取消按钮（W3.3）**
  - `src/api/tauri.ts`：
    - 新增 `cancelAgent(sessionId)` — 调 `cancel_agent` Tauri 命令
    - `AgentEvent` 加 `chunk { sessionId, step, delta }` + `cancelled { sessionId }` 变体
    - `AgentRunResponse` 加 `tokensUsed?` + `cancelled?` 字段
  - `src/stores/agentStore.ts`：
    - 新增 `streaming: { messageId, step } | null` 流式槽位
    - `chunk` 事件累积：同一 step 后续 chunk append 到现有消息；新 step 创建新 assistant 消息
    - `final_answer` 事件：用干净 final_answer 替换 streaming 槽位的内容
    - `cancelled` 事件：设置 `error="已取消"` + `isRunning=false` + 清空 streaming
    - `started` 事件回填 server-side `sessionId`（前端预设的 `sess_xxx` 不可靠）
    - 新增 `cancel()` action — 调 `cancelAgent` API
    - `send()` 兜底：run_agent 异常路径用 `lastResponse.finalAnswer` 写入 streaming 槽位
  - `src/pages/AgentRunnerPage.tsx`：
    - isRunning 时显示"取消"按钮（替代"开始生成"）
    - 取消后显示"已取消"红框 + 按钮回到"开始生成"
- **后端 <think> 标签剥离（W3.3 — `[[llm-integration-quirks]]` #3）**
  - `src-tauri/src/model_openai.rs`：
    - 新增 `strip_think_tags(input: &str) -> String`：剥除推理模型 `<think>...</think>` 思考片段
    - 完整配对（greedy + 连续多段）全剥
    - 未闭合保守原样保留（不破坏正常内容）
    - 在 `parse_decision`（流式）和 `parse_decision_from_response`（非流式）两处都应用
    - 同步剥 `decision.thought`（tool_call 路径的 thought 来自 raw text）
  - 测试
    - `tests/openai_parse.rs` 加 4 个 strip 测试：simple / multi-segment / no-tag-passthrough / unclosed-passthrough
    - **总计 29 个测试全过（4 单元 + 5 + 4 + 4 + 11 + 1 真实 LLM）**

### Added (Week 4.5 — 种子用户启动)
- **A 紧急安全 + IPC 修复**
  - `docs/00-账号信息/code.md` 含明文密钥 → `git rm` + `.gitignore` 加固 `docs/00-*/` 模式
  - `src-tauri/tauri.conf.json` `updater.active: true` + 空 pubkey → `active: false`
  - `src-tauri/src/creations.rs` + `db.rs` 加 `#[serde(rename_all = "camelCase")]` 修 IPC 边界
  - `index.html` CSP 加 `media-src https://*.volces.com https://*.cdn.volces.com data: blob:` (允许 Seedance 视频 CDN)
- **B1 License + Quota 控制平面后端** (`kidsai-server/`)
  - FastAPI + SQLite + python-jose JWT HS256, 7 个 endpoint: activate / balance / record-spend / refresh-license / admin grant+revoke / healthz
  - 学币计费: 默认 daily_quota=30/天, 起始 balance=100, LLM cost=0.001/token, 视频试拍 9/定稿 19, 单笔 cap 20
  - 幂等: `transactions.call_id UNIQUE`, 二次上报同 call_id 返原记录
  - 修 2 bug: revoke 立即失效 (加 blacklist check) + LLM cost floor (min 1 token)
- **B2 桌面 License 适配层** (直连 provider 模式, 不做代理)
  - `src-tauri/src/license_client.rs` reqwest 4 method (activate/balance/record-spend/refresh-license)
  - `src-tauri/src/license_store.rs` 持久化 `app_data_dir/license.json` chmod 600
  - `agent.rs` / `video_adapter.rs` 加 check-then-act: 调用前查余额, 调用后异步 fire-and-forget 上报
  - `src/pages/OnboardingPage.tsx` 首次激活流 (昵称 + 年级)
  - `src/stores/tokenStore.ts` 退化为只读视图, 实际扣账由 server 权威
- **B3 ECS 部署** (8.133.241.103, Aliyun Linux 4)
  - systemd unit `kidsai-server.service` + nginx `api.kids.ibi.ren` vhost + `kids.ibi.ren` 静态前端
  - 双 DigiCert DV 证书 (2026-10-09 到期需续)
  - install.sh + admin CLI (`kidsai-admin grant DEVICE_ID 50`) + runbook
  - 文档: `kidsai-server/DEPLOY.md` (systemd + nginx + certbot + .env 注入)
- **C1 真 Seedance 端到端集成测试** (`src-tauri/tests/real_seedance_via_license.rs`, `--ignored`, ¥1 段视频)
  - 真后端激活 → grant 学币 → 桌面直连 Seedance → 上报 spend → 拿到 video_url
- **C2 + D3 vite 截图 + 删 dev 标记 + 修侧栏 hardcode**
  - HomePage 删 `Week 2 进行中...` 内部 dev 行
  - Sidebar "💎 500" → 读 `useTokenStore.balance`
- **C3 macOS 真机 .dmg 打包** (`KidsAI Studio_0.1.0_aarch64.dmg`, 3.0 MB)
  - Windows .msi 暂缓 (需 lihao 在 Windows VM 上 cargo-xwin)
- **C4 种子用户指南** (`docs/seed-user-guide.md`)
  - 3 屏图文: 安装 (mac .dmg + Win .msi deferred) / 首次启动激活 / Studio 试玩

### Added (Week 5 — Studio 3 UX issues 修复)
- **① ProjectsPane 去重**
  - 砍掉重复的 视频/游戏/Agent tab + ➕开始新创作 + 学币栏 (全局 Sidebar 已管)
  - 左栏 w-56 → w-44, 中屏对话流更宽
  - ✕ 关闭按钮回首页 (`onBackHome` 通过 App.tsx `handleBackToHome` 注入)
- **② directorStore 锁定命题 + 分镜故事连贯**
  - 新增 `locked_props: { subject?, story_core?, art_style? }`
  - studioStore 在每阶 ✓ 拍板时调 `lockSubject` / `lockStoryCore` / `lockArtStyle` 写入
  - `runPlanGeneration` 注入【上下文:已锁定命题】chunk + 硬约束 "每个分镜必须服务于 story_core"
  - 加 LLM 连贯性自检 (`director_coherence` level), 不达标时回原因给 UI 让用户在分镜页直接改
- **③ 状态机可回退 (cursor + history)**
  - `stage` (单调) → `cursor` + `history[]` (每阶决策时快照)
  - `goBackTo(idx)` 还原快照 + 下游标 `stale: true` (UI 显示 ⚪)
  - ProgressMap 的 ✓ 胶囊变可点击 `<button>`, 点击调 `goBackTo`
  - 回退到 stage 1 还调 `studioStore.goBackToStep1()` 重放 4 个 beat
  - 新增 7 个单测覆盖 cursor/history/locked_props/goBackTo/system_prompt 注入
- **Bug fix**: `listCharacters`/`listStyles` 返回 null 时 `.find` 崩 (mock 返 null), 加 `.then((r) => r ?? [])` 防御

### TODO
- 真实品牌图标（设计师出图后替换）
- Apple Developer / Windows 代码签名证书
- Tauri signing 公私钥对
