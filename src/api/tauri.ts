// Tauri 命令的 TypeScript 封装
// 所有跨进程调用都集中在这里，方便 mock 和测试

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Level, LevelProgress, ScoringCriteria } from '../../shared/types/level';

// ============ 关卡 ============

export async function listLevels(): Promise<Level[]> {
  return invoke<Level[]>('list_levels');
}

export async function getLevel(id: string): Promise<Level | null> {
  return invoke<Level | null>('get_level', { id });
}

export async function listProgress(): Promise<LevelProgress[]> {
  return invoke<LevelProgress[]>('list_progress');
}

export async function startLevel(id: string): Promise<LevelProgress> {
  return invoke<LevelProgress>('start_level', { id });
}

export async function submitLevel(
  levelId: string,
  score: number,
  rubric: ScoringCriteria,
  feedback: string,
): Promise<LevelProgress> {
  return invoke<LevelProgress>('submit_level', {
    levelId,
    score,
    rubric,
    feedback,
  });
}

export async function completedLevelIds(): Promise<string[]> {
  return invoke<string[]>('completed_level_ids');
}

// ============ 作品 (W2.3) ============

export interface AssetInput {
  type: 'image' | 'video' | 'audio';
  url: string;
  thumbnailUrl?: string;
  prompt: string;
  tool: string;
  tokensCost: number;
}

export interface SaveCreationRequest {
  id: string;
  levelId: string;
  userInput: string;
  agentOutput: Record<string, unknown>;
  score?: number;
  rubric?: ScoringCriteria;
  feedback?: string;
  assets: AssetInput[];
}

export interface CreationWithAssets {
  id: string;
  levelId: string;
  userInput: string;
  agentOutput: string; // JSON string from DB
  score: number | null;
  rubric: string | null;
  feedback: string | null;
  createdAt: number;
  assets: Array<{
    kind: string;
    url: string;
    thumbnailUrl: string | null;
    prompt: string;
    tool: string;
    tokensCost: number;
  }>;
}

export async function saveCreation(
  request: SaveCreationRequest,
): Promise<void> {
  return invoke<void>('save_creation', { request });
}

export async function listCreations(
  levelId?: string,
): Promise<CreationWithAssets[]> {
  return invoke<CreationWithAssets[]>('list_creations', { levelId });
}

// ============ 角色 (W3.4) ============

export interface Character {
  id: string;
  name: string;
  description: string;
  styleTags: string[];
  referenceImageUrl?: string;
}

export async function listCharacters(): Promise<Character[]> {
  return invoke<Character[]>('list_characters');
}

// ============ 风格模板 (W3.6) ============

export interface StylePreset {
  id: string;
  name: string;
  description: string;
  styleTags: string[];
}

export async function listStyles(): Promise<StylePreset[]> {
  return invoke<StylePreset[]>('list_styles');
}

// ============ Agent ============

export interface AgentRunRequest {
  levelId: string;
  userInput: string;
  systemPrompt: string;
  tools?: string[];
  /// W3.4: session 绑定角色（可选）— 选了角色后，同一会话多次生成的图片会保持形象一致
  characterId?: string;
  /// W3.6: session 绑定视觉风格（可选）— 选了风格后，同一会话生成的图片共享同一视觉语言
  styleId?: string;
}

export interface AgentRunResponse {
  sessionId: string;
  levelId: string;
  finalAnswer: string;
  thoughts: string[];
  toolCalls: Array<{
    tool: string;
    args: Record<string, unknown>;
    result: string;
    timestamp: number;
  }>;
  assets: Array<{
    type: 'image' | 'video' | 'audio';
    url: string;
    thumbnailUrl?: string;
    prompt: string;
    tool: string;
    tokensCost: number;
  }>;
  durationMs: number;
  // W3.2: 真实 LLM token 累计 + 取消标记
  tokensUsed?: number;
  cancelled?: boolean;
}

export async function runAgent(
  request: AgentRunRequest,
): Promise<AgentRunResponse> {
  return invoke<AgentRunResponse>('run_agent', { request });
}

// W3.2: 中途取消正在运行的 agent 循环
export async function cancelAgent(sessionId: string): Promise<boolean> {
  return invoke<boolean>('cancel_agent', { sessionId });
}

// ============ 系统命令 ============

export async function getAppVersion(): Promise<string> {
  return invoke<string>('get_app_version');
}

export async function greet(name: string): Promise<string> {
  return invoke<string>('greet', { name });
}

// ============ 安全审核 (W2.7) ============

export type SafetyVerdict =
  | 'pass'
  | { warn: { reason: string } }
  | { block: { reason: string } };

export async function checkSafety(text: string): Promise<SafetyVerdict> {
  // Rust 返回的是枚举 tagged union（snake_case 自动），前端会收到
  // { "pass": null } | { "warn": { "reason": "..." } } | { "block": { "reason": "..." } }
  return invoke<SafetyVerdict>('check_safety', { text });
}

// ============ Agent 事件流 (W2.4 + W3.2 流式 + 取消) ============

export type AgentEvent =
  | { kind: 'started'; sessionId: string }
  | { kind: 'thought'; sessionId: string; step: number; thought: string }
  | { kind: 'tool_call'; sessionId: string; step: number; tool: string; args: unknown }
  | { kind: 'tool_result'; sessionId: string; step: number; tool: string; result: string; assets: AgentAsset[] }
  // W3.2: 流式 — 每个 delta 一条事件，前端累积到一条 assistant 消息
  | { kind: 'chunk'; sessionId: string; step: number; delta: string }
  // W3.2: 取消 — 收到这条后立即停止生成
  | { kind: 'cancelled'; sessionId: string }
  | { kind: 'final_answer'; sessionId: string; answer: string }
  | { kind: 'done'; sessionId: string; steps: number; durationMs: number }
  | { kind: 'error'; sessionId: string; message: string };

export interface AgentAsset {
  type: 'image' | 'video' | 'audio';
  url: string;
  thumbnailUrl?: string;
  prompt: string;
  tool: string;
  tokensCost: number;
  /// W3.5: 如果这个资产是「点哪改哪」的产物，记录源图 URL（仅前端 metadata，不发后端）
  sourceAssetUrl?: string;
}

export async function onAgentEvent(
  handler: (event: AgentEvent) => void,
): Promise<UnlistenFn> {
  return listen<AgentEvent>('agent://event', (e) => handler(e.payload));
}

// ============ 视频导演流程 (W1 v1) ============
// 1 学分 = 0.1 元。Seedance 各档位计费参考 memory/reference_seedance_video_api.md。
export const CREDITS = {
  PREVIEW_PER_SHOT: 9,  // mini 试拍单条分镜 (480P/4s)
  FINALIZE: 19,         // 2.0 定稿 1 条 (480P/默认时长)
} as const;

// 端点 ID 写死后端:见 .env SEEDANCE_MODEL = mini 端点, 导演流程在 tool args 里用 model 字段覆盖
export const SEEDANCE_MODEL = {
  PREVIEW: 'doubao-seedance-2-0-mini-260615',   // mini 端点实际解析的 model
  FINALIZE: 'doubao-seedance-2-0-260128',        // 2.0 端点实际解析的 model
} as const;

/// DirectorPlan = ① 输入后 LLM 返回的全案 JSON, 用于填充 ②③④
export interface DirectorPlanShot {
  description: string;   // 1 句话:这一镜发生什么(孩子能看懂的)
  motion: string;        // Seedance prompt(带动作、镜头、风格)
}

export interface DirectorPlan {
  idea: string;                            // 孩子的点子(原样或润色)
  character_id: 'xiaoqi' | 'xiaoyue' | 'xiaoxing';
  style_id: 'anime' | 'cartoon' | 'clay' | 'ink' | 'line-drawing' | 'photo' | 'pixel';
  shots: DirectorPlanShot[];               // 长度 3
}

export interface DirectorPlanParseResult {
  ok: boolean;
  plan: DirectorPlan | null;
  error?: string;
  raw?: string;                            // LLM 原文(排障用)
}

/// 把 LLM 输出的纯文本解析成 DirectorPlan。容错策略:
/// 1) 严格 JSON 解析成功 → 返回
/// 2) 失败 → 尝试从 markdown ```json ... ``` 代码块里抽
/// 3) 仍失败 → ok=false, 调用方用兜底
export function parseDirectorPlan(raw: string): DirectorPlanParseResult {
  const trimmed = raw.trim();
  if (!trimmed) return { ok: false, plan: null, error: 'empty', raw };

  // 策略 1: 整体就是 JSON
  try {
    const v = JSON.parse(trimmed) as DirectorPlan;
    if (validateDirectorPlan(v)) return { ok: true, plan: v, raw };
  } catch {
    // 继续策略 2
  }

  // 策略 2: 找 ```json ... ``` 块
  const fence = /```(?:json)?\s*([\s\S]+?)\s*```/.exec(trimmed);
  if (fence) {
    try {
      const v = JSON.parse(fence[1]) as DirectorPlan;
      if (validateDirectorPlan(v)) return { ok: true, plan: v, raw };
    } catch {
      // 继续
    }
  }

  return { ok: false, plan: null, error: 'invalid JSON or schema', raw };
}

function validateDirectorPlan(v: unknown): v is DirectorPlan {
  if (!v || typeof v !== 'object') return false;
  const o = v as Record<string, unknown>;
  if (typeof o.idea !== 'string') return false;
  if (typeof o.character_id !== 'string') return false;
  if (typeof o.style_id !== 'string') return false;
  if (!Array.isArray(o.shots)) return false;
  if (o.shots.length !== 3) return false;
  return o.shots.every(
    (s) =>
      s &&
      typeof (s as Record<string, unknown>).description === 'string' &&
      typeof (s as Record<string, unknown>).motion === 'string',
  );
}

/// ① 系统 prompt: 让 LLM 返回 DirectorPlan JSON
/// 紧: 给 schema 模板 + 明确"只返 JSON,不要解释"
export const DIRECTOR_PLAN_SYSTEM_PROMPT = `你是 KidsAI 的"小启",一个会陪孩子拍小动画的 AI 导演朋友。

孩子会告诉你他想拍一个什么样的视频,你的任务是把它拆成一个简单的拍摄方案,以**严格 JSON 形式**返回,**不要任何额外文字**。

# JSON Schema
{
  "idea": "string — 孩子的点子(可润色,但保持原意)",
  "character_id": "xiaoqi" | "xiaoyue" | "xiaoxing"  // 从下面 3 个角色选最匹配的
  "style_id": "anime" | "cartoon" | "clay" | "ink" | "line-drawing" | "photo" | "pixel"  // 从下面 7 个风格选最匹配的
  "shots": [
    { "description": "string — 1 句话讲这一镜发生什么(孩子能懂)", "motion": "string — 给 Seedance 的 motion prompt" },
    // 长度固定 3, 顺序:开头 → 中间 → 结尾
  ]
}

# 角色
- xiaoqi(小启):9岁好奇小猫女孩,黄色短发、穿黄色T恤、戴小围巾、眼睛又大又亮。适合:活泼、可爱的故事
- xiaoyue(小月):8岁女孩,双马尾、红色连衣裙、手里捧着书。适合:安静、温馨、阅读相关故事
- xiaoxing(小星):10岁男孩,黑框眼镜、蓝色卫衣、爱思考。适合:探索、思考、冒险故事

# 风格
- anime: 日系动漫
- cartoon: 明亮卡通
- clay: 黏土质感
- ink: 水墨画
- line-drawing: 线描
- photo: 写实照片
- pixel: 像素风

# motion 写法指导
- 用儿童能想象到的语言描述动作
- 包含"谁+做什么+在哪儿+看起来怎么样"
- 长度 1-2 句, 不要超过 80 字
- 避免成人化或复杂的摄影术语
- 例: "小启在花园里追着一只蝴蝶跑, 边跑边笑"
- 例: "小启张开手臂, 慢慢地从地面飘到半空中"

只返回 JSON, 不要 \`\`\`json 标记, 不要任何解释。`;

// ============ License (W4.5 B2) ============

export interface ActivateResponse {
  deviceId: string;
  licenseToken: string;
  apiKeys: { llm: string; video: string };
  balance: number;
  dailyQuota: number;
}

export interface BalanceResponse {
  deviceId: string;
  balance: number;
  dailyConsumed: number;
  dailyQuota: number;
  dailyRemaining: number;
}

export interface RefreshResponse {
  deviceId: string;
  licenseToken: string;
  apiKeys: { llm: string; video: string };
}

export interface LicenseInfo {
  deviceId: string;
  nickname: string;
  ageTier: number;
  lastBalance: number;
  isDemo: boolean;
  activatedAt: number;
}

export async function activateDevice(
  fingerprintHash: string,
  nickname: string,
  ageTier: number,
): Promise<ActivateResponse> {
  return invoke<ActivateResponse>('activate_device', {
    fingerprintHash,
    nickname,
    ageTier,
  });
}

export async function getBalance(): Promise<BalanceResponse> {
  return invoke<BalanceResponse>('get_balance');
}

export async function refreshLicense(): Promise<RefreshResponse> {
  return invoke<RefreshResponse>('refresh_license');
}

export async function getLicenseInfo(): Promise<LicenseInfo | null> {
  return invoke<LicenseInfo | null>('get_license_info');
}

export async function resetLicense(): Promise<void> {
  return invoke<void>('reset_license');
}

/// 用浏览器特征生成稳定 fingerprint hash (用户机器唯一标识, server 用 sha256 截断 32 字节).
/// 不收集跨域 cookie, 只是一个 client-side 标识, 供 server 识别"同一台设备重复激活".
export async function getFingerprintHash(): Promise<string> {
  const parts = [
    navigator.userAgent,
    navigator.language,
    `${screen.width}x${screen.height}`,
    new Date().getTimezoneOffset().toString(),
    navigator.hardwareConcurrency?.toString() ?? '',
  ];
  const raw = parts.join('|');
  const buf = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(raw),
  );
  return Array.from(new Uint8Array(buf))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}
