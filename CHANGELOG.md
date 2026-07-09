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

### TODO
- 真实品牌图标（设计师出图后替换）
- Apple Developer / Windows 代码签名证书
- Tauri signing 公私钥对
