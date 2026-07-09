# 更新日志

所有重要的项目变更都会记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

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

### TODO
- 真实品牌图标（设计师出图后替换）
- Apple Developer / Windows 代码签名证书
- Tauri signing 公私钥对
