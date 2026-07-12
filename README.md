# KidsAI Studio

> 8-16 岁青少年 AI 创作学习平台 · 桌面客户端

[![CI](https://github.com/lihaoalbert/KidsAI/actions/workflows/release.yml/badge.svg)](https://github.com/lihaoalbert/KidsAI/actions)

## 项目简介

KidsAI Studio 是一款基于 **Tauri 2.0 + React 18** 的桌面客户端应用，目标用户为 **8-16 岁青少年**。通过 AI 引导式教学，让孩子在**当前 7 关 (L1-L7)** 递进式课程中学会用 AI 创作真实作品（视频、绘画、Agent），最终对接青少年 AI 素养等级认证。L8-L20 关卡规划中，详见 `docs/03-教学内容/`。

## 技术栈

| 层 | 选型 |
|---|---|
| 桌面框架 | Tauri 2.0（Rust 后端） |
| 前端 | React 18 + Vite + TypeScript |
| 样式 | Tailwind CSS 3 |
| 状态管理 | Zustand |
| 组件库 | Radix UI + 自研组件 |
| 路由 | React Router 6 |
| 组件文档 | Storybook 8 |
| CI/CD | GitHub Actions |

## 快速开始

### 环境要求

- **Node.js** >= 20
- **Rust** >= 1.77（[安装](https://rustup.rs/)）
- **macOS** 11+ / **Windows** 10+ / **Linux** (Ubuntu 22.04+)

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri:dev
```

### 构建发布版

```bash
npm run tauri:build
```

### 组件库开发

```bash
npm run storybook
# 访问 http://localhost:6006
```

## 项目结构

```
KidsAI/
├── src/                  # React 前端
│   ├── components/       # 通用组件（Button/Input/Card/Sidebar）
│   ├── pages/            # 页面（Home/Workshop/Library/Agent）
│   ├── styles/           # 全局样式
│   └── main.tsx          # 入口
├── src-tauri/            # Rust 后端
│   ├── src/              # Rust 源码
│   ├── capabilities/     # Tauri 2.0 权限配置
│   ├── icons/            # 应用图标
│   ├── tauri.conf.json   # Tauri 配置
│   └── Cargo.toml
├── shared/types/         # 前后端共享 TS 类型
├── .github/workflows/    # CI/CD
├── .storybook/           # Storybook 配置
└── docs/                 # 产品文档（gitignored）
```

## 开发规范

- **TypeScript**：所有新代码必须用 TS
- **组件**：原子化、可复用、有 Storybook story
- **样式**：Tailwind 优先，自定义类限定在 `@layer`
- **提交**：Conventional Commits 规范
- **分支**：`main` (稳定) / `feat/*` (功能) / `fix/*` (修复)

## 当前进度

### ✅ Week 1 骨架（已完成）
- [x] Tauri 2.0 项目初始化
- [x] React + Vite + TypeScript 前端
- [x] 侧边栏导航 + 4 个空页面
- [x] 基础组件库 v0.1（Button / Input / Card）
- [x] Storybook 配置
- [x] CI/CD 流水线（GitHub Actions）
- [x] 应用图标（占位紫底）

### 📋 Week 2 计划
- [ ] Agent Loop 核心实现
- [ ] LiteLLM 多模型路由
- [ ] MCP 工具：文生图 + TTS
- [ ] 微信扫码登录

## 相关文档

详细产品文档见 `docs/` 目录（本地维护，不进 git）：
- `docs/01-产品方案/` — 产品定位与方案
- `docs/02-技术架构/` — 17 章技术架构
- `docs/03-教学内容/` — 7 关卡脚本 (L1-L7) + AI 助教 Prompt 库
- `docs/04-设计规范/` — UI/UX 完整设计稿
- `docs/05-家长端/` — 家长端小程序原型
- `docs/06-合规法务/` — 100 项合规自查清单
- `docs/07-商业化/` — Token 计费详细规则
- `docs/08-运营交付/` — 6 周冲刺 + 客户端更新机制

## License

内部使用 · 未经允许禁止外传
