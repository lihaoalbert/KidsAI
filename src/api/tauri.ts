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

// ============ 项目 + 本地资产 ============

export interface ProjectMeta {
  id: string;
  title: string;
  levelId: string | null;
  cursor: number;
  thumbPath: string | null;
  totalCredits: number;
  createdAt: number;
  updatedAt: number;
}

export type ProjectSummary = ProjectMeta;

export interface ProjectFull {
  meta: ProjectMeta;
  plan: Record<string, unknown>;
  transcript: Record<string, unknown> | unknown[];
}

export interface ProjectStatePatch {
  cursor?: number;
  thumbPath?: string;
  totalCredits?: number;
}

export async function listProjects(): Promise<ProjectSummary[]> {
  return invoke<ProjectSummary[]>('list_projects');
}

export async function loadProject(id: string): Promise<ProjectFull> {
  return invoke<ProjectFull>('load_project', { id });
}

export async function createProject(
  title: string,
  levelId?: string,
): Promise<ProjectMeta> {
  return invoke<ProjectMeta>('create_project', { title, levelId });
}

export async function renameProject(id: string, title: string): Promise<void> {
  return invoke<void>('rename_project', { id, title });
}

export async function deleteProject(id: string): Promise<void> {
  return invoke<void>('delete_project', { id });
}

export async function saveProjectState(
  id: string,
  plan: Record<string, unknown>,
  transcript: Record<string, unknown>,
  meta: ProjectStatePatch,
): Promise<ProjectMeta> {
  return invoke<ProjectMeta>('save_project_state', {
    id,
    plan,
    transcript,
    meta,
  });
}

export async function downloadAsset(
  projectId: string,
  url: string,
  kind: 'image' | 'video' | 'audio',
  subPath: string,
): Promise<number> {
  return invoke<number>('download_asset', { projectId, url, kind, subPath });
}

export async function resolveAsset(
  projectId: string,
  url: string,
): Promise<string | null> {
  return invoke<string | null>('resolve_asset', { projectId, url });
}

export interface AssetLocalEvent {
  projectId: string;
  url: string;
  localPath: string;
  status: 'downloaded' | 'failed';
}

export async function onAssetLocal(
  handler: (event: AssetLocalEvent) => void,
): Promise<UnlistenFn> {
  return listen<AssetLocalEvent>('asset://local', (event) => handler(event.payload));
}

// ============ 角色 (W3.4) ============

export interface Character {
  id: string;
  name: string;
  description: string;
  styleTags: string[];
  referenceImageUrl?: string;
  /// W4.6 #2: 三视图合图, 用于 Seedance 跨镜硬锚.
  standardImageUrl?: string;
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
// 1 学分 = 0.1 元。各档位计费参考 memory/reference_seedance_video_api.md (Seedance)
// + memory/reference_minimax_video_api.md (Hailuo).
export const CREDITS = {
  PREVIEW_PER_SHOT: 9,  // Seedance mini 试拍单条分镜 (480P/4s)
  FINALIZE: 19,         // Seedance 2.0 定稿 1 条 (480P/默认时长)
  HAILUO_VIDEO: 12,     // W6 C4: MiniMax hailuo-02 备用 (套餐额度, 不默认给孩)
  IMAGE_GEN: 5,         // W6 C1: image-01 单张立绘
  VOICE_CLONE: 10,      // W6 C2: 声音复刻一次 (录 10s 训练)
  MUSIC_GEN: 8,         // W6 C3: music-01 一段 BGM (默认 30s)
} as const;

// W6 C4: 视频引擎选项. 端点 ID 写死桌面端 (provider 端硬 cap 防破产).
// 导演流程在 tool args 里用 model 字段覆盖, 学币按 model 分发.
export const VIDEO_MODEL = {
  SEEDANCE_PREVIEW: 'doubao-seedance-2-0-mini-260615',
  SEEDANCE_FINALIZE: 'doubao-seedance-2-0-260128',
  HAILUO: 'MiniMax-hailuo-02',
} as const;

/// W6 B3: 拉取资产 manifest (server 静态托管的 200 张预生成图)
export interface AssetManifest {
  version: number;
  generatedCount: number;
  images: Record<string, string>; // key → full URL
}

export async function getAssetManifest(): Promise<AssetManifest> {
  // 直连 server, 跟 license 流程走 KIDSAI_SERVER_URL — 暂时走 fetch 而非 invoke
  // (asset endpoint 在 server, 不在 tauri command). 由调用方注入 base URL.
  // 这里返 type, 实现放 utils/getManifest.ts 里 (避免循环依赖).
  throw new Error('use fetchAssetManifest() instead — see utils/assetUrl.ts');
}

/// W6 B3: 拉取资产 manifest (HTTP, 直连 server 的 /api/v1/asset-manifest).
export async function fetchAssetManifest(
  serverUrl: string,
): Promise<AssetManifest> {
  const r = await fetch(`${serverUrl.replace(/\/$/, '')}/api/v1/asset-manifest`, {
    method: 'GET',
  });
  if (!r.ok) {
    throw new Error(`asset-manifest http ${r.status}`);
  }
  const raw = (await r.json()) as {
    version?: unknown;
    generated_count?: unknown;
    generatedCount?: unknown;
    images?: unknown;
  };
  if (!raw.images || typeof raw.images !== 'object' || Array.isArray(raw.images)) {
    throw new Error('asset-manifest images invalid');
  }
  const images = Object.fromEntries(
    Object.entries(raw.images).filter((entry): entry is [string, string] => typeof entry[1] === 'string'),
  );
  const generatedCount = raw.generatedCount ?? raw.generated_count;
  return {
    version: typeof raw.version === 'number' ? raw.version : 0,
    generatedCount: typeof generatedCount === 'number' ? generatedCount : Object.keys(images).length,
    images,
  };
}

/// W4.6 #4: 节奏 5 拍 + 镜头语言, 每镜独立标注.
/// "5 拍"对应 v2 导演流程的 Hook → Conflict → Payoff 三幕, 每幕 1-2 镜, 共 3-5 镜.
/// beat 决定这一镜在故事弧线里的位置, mood/camera 决定镜头语言 + 情绪颗粒度,
/// characterRefs 锁定"谁在镜里" (多角色时声明具体哪些).
/// **enum 名严格对齐 Rust prompt_builder.rs 的 ShotMood / ShotCamera** —
/// LLM 输出 → 前端 deserialize → 透传到后端 ToolContext.shot.mood/camera → build_seedance_prompt,
/// 整条链路必须同字面量, 否则 serde 反序列化会默认走 Calm/Medium, 静默丢失 LLM 决策.
export type StoryBeat = 'hook' | 'conflict' | 'payoff';
export type ShotMood = 'calm' | 'tense' | 'joyful' | 'sad' | 'epic';
export type ShotCamera = 'wide' | 'medium' | 'close' | 'extreme' | 'follow' | 'overhead';
export type ShotTransition = 'cut' | 'fade' | 'dissolve' | 'wipe' | 'none';

/// DirectorPlan = ① 输入后 LLM 返回的全案 JSON, 用于填充 ②③④
export interface DirectorPlanShot {
  description: string;                 // 1 句话:这一镜发生什么(孩子能看懂的)
  motion: string;                      // Seedance prompt(带动作、镜头、风格)
  /// W4.6 #4: 5 拍节奏里这一镜所属位置 (hook/conflict/payoff)
  beat: StoryBeat;
  /// W4.6 #4: 情绪颗粒度 → build_seedance_prompt 的 [Mood] 行
  mood: ShotMood;
  /// W4.6 #4: 镜头语言 → build_seedance_prompt 的 [Camera] 行
  camera: ShotCamera;
  /// W4.6 #4: 这镜涉及哪些角色 (id 列表). 多角色故事必填, 单角色简化为 [character_id].
  character_refs: string[];
  /// W4.6 #4: 与下一镜的转场 (cut / fade / dissolve / wipe / none). 最后一镜固定 'none'.
  transition_to_next: ShotTransition;
}

export interface DirectorPlan {
  idea: string;                            // 孩子的点子(原样或润色)
  character_id: 'xiaoqi' | 'xiaoyue' | 'xiaoxing';
  style_id: 'anime' | 'cartoon' | 'clay' | 'ink' | 'line-drawing' | 'photo' | 'pixel';
  /// W4.6 #4: 长度 3-5 镜. 3 = 三幕单镜, 4 = 三幕 + 1 个过场, 5 = 三幕各 1-2 镜.
  shots: DirectorPlanShot[];
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
/// W4.6 #4: error 字段填具体失败原因 (JSON parse / schema), 便于排障 + 测试断言.
export function parseDirectorPlan(raw: string): DirectorPlanParseResult {
  const trimmed = raw.trim();
  if (!trimmed) return { ok: false, plan: null, error: 'empty', raw };

  let parseError = '';
  let schemaError = '';

  // 策略 1: 整体就是 JSON
  try {
    const v = JSON.parse(trimmed) as DirectorPlan;
    if (validateDirectorPlan(v)) return { ok: true, plan: v, raw };
    schemaError = 'schema failed (整体 JSON)';
  } catch (e) {
    parseError = `JSON parse failed (整体): ${(e as Error).message}`;
  }

  // 策略 2: 找 ```json ... ``` 块
  const fence = /```(?:json)?\s*([\s\S]+?)\s*```/.exec(trimmed);
  if (fence) {
    try {
      const v = JSON.parse(fence[1]) as DirectorPlan;
      if (validateDirectorPlan(v)) return { ok: true, plan: v, raw };
      schemaError = 'schema failed (代码块)';
    } catch (e) {
      parseError = `JSON parse failed (代码块): ${(e as Error).message}`;
    }
  }

  const error = schemaError || parseError || 'invalid JSON or schema';
  return { ok: false, plan: null, error, raw };
}

/// W4.6 #4: 严格 enum + 5 字段必填 + shots 长度 3-5.
/// 失败原因细分到 error 字段, 便于排障 + 测试断言.
const VALID_BEATS: ReadonlyArray<StoryBeat> = [
  'hook',
  'conflict',
  'payoff',
];
const VALID_MOODS: ReadonlyArray<ShotMood> = [
  'calm',
  'tense',
  'joyful',
  'sad',
  'epic',
];
const VALID_CAMERAS: ReadonlyArray<ShotCamera> = [
  'wide',
  'medium',
  'close',
  'extreme',
  'follow',
  'overhead',
];
const VALID_TRANSITIONS: ReadonlyArray<ShotTransition> = [
  'cut',
  'fade',
  'dissolve',
  'wipe',
  'none',
];
const VALID_CHARACTERS = ['xiaoqi', 'xiaoyue', 'xiaoxing'] as const;
const VALID_STYLES = [
  'anime',
  'cartoon',
  'clay',
  'ink',
  'line-drawing',
  'photo',
  'pixel',
] as const;

function validateDirectorPlan(v: unknown): v is DirectorPlan {
  if (!v || typeof v !== 'object') return false;
  const o = v as Record<string, unknown>;
  if (typeof o.idea !== 'string' || o.idea.length === 0) return false;
  if (typeof o.character_id !== 'string') return false;
  if (!(VALID_CHARACTERS as readonly string[]).includes(o.character_id)) return false;
  if (typeof o.style_id !== 'string') return false;
  if (!(VALID_STYLES as readonly string[]).includes(o.style_id)) return false;
  if (!Array.isArray(o.shots)) return false;
  if (o.shots.length < 3 || o.shots.length > 5) return false;
  const shots = o.shots as Array<unknown>;
  return shots.every((s, idx) => {
    if (!s || typeof s !== 'object') return false;
    const so = s as Record<string, unknown>;
    if (typeof so.description !== 'string' || so.description.length === 0) return false;
    if (typeof so.motion !== 'string' || so.motion.length === 0) return false;
    if (typeof so.beat !== 'string' || !(VALID_BEATS as readonly string[]).includes(so.beat)) return false;
    if (typeof so.mood !== 'string' || !(VALID_MOODS as readonly string[]).includes(so.mood)) return false;
    if (typeof so.camera !== 'string' || !(VALID_CAMERAS as readonly string[]).includes(so.camera)) return false;
    if (!Array.isArray(so.character_refs)) return false;
    if (so.character_refs.length === 0 || so.character_refs.length > 3) return false;
    if (!so.character_refs.every((r) => typeof r === 'string' && r.length > 0)) return false;
    if (typeof so.transition_to_next !== 'string') return false;
    // 最后一镜 transition_to_next 必须是 'none', 前面必须是 4 选 1
    if (idx === shots.length - 1) {
      return so.transition_to_next === 'none';
    }
    return (VALID_TRANSITIONS as readonly string[]).includes(so.transition_to_next);
  });
}

/// ① 系统 prompt: 让 LLM 返回 DirectorPlan JSON
/// 紧: 给 schema 模板 + 明确"只返 JSON,不要解释"
/// W4.6 #4: 加 # 节奏 节 (Hook → Conflict → Payoff) + 每镜 5 字段强约束
export const DIRECTOR_PLAN_SYSTEM_PROMPT = `你是 KidsAI 的"小启",一个会陪孩子拍小动画的 AI 导演朋友。

孩子会告诉你他想拍一个什么样的视频,你的任务是把它拆成一个简单的拍摄方案,以**严格 JSON 形式**返回,**不要任何额外文字**。

# JSON Schema
{
  "idea": "string — 孩子的点子(可润色,但保持原意)",
  "character_id": "xiaoqi" | "xiaoyue" | "xiaoxing"  // 从下面 3 个角色选最匹配的
  "style_id": "anime" | "cartoon" | "clay" | "ink" | "line-drawing" | "photo" | "pixel"  // 从下面 7 个风格选最匹配的
  "shots": [
    {
      "description": "string — 1 句话讲这一镜发生什么(孩子能懂)",
      "motion": "string — 给 Seedance 的 motion prompt",
      "beat": "hook" | "conflict" | "payoff",
      "mood": "calm" | "happy" | "sad" | "excited" | "tense" | "dreamy",
      "camera": "wide" | "medium" | "close-up" | "over-shoulder" | "drone" | "aerial",
      "character_refs": ["xiaoqi" | "xiaoyue" | "xiaoxing"],
      "transition_to_next": "cut" | "fade" | "dissolve" | "wipe" | "none"
    },
    // 长度 3-5 镜. 最后一镜 transition_to_next 必须是 "none".
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

# 节奏 (W4.6 #4: 5 拍三幕, 严格区分每镜位置)
你的方案按"三幕五拍"展开,每镜必须标 beat:
- **hook (钩子)**: 开头 1-2 镜, 抓住眼球. 主角登场 + 抛出问题或冲突种子. 视觉冲击 + 情绪点燃.
- **conflict (冲突)**: 中间 1-2 镜, 把问题升级. 主角尝试但失败 / 遇到障碍 / 紧张对峙. 节奏加快, 镜头更紧.
- **payoff (收尾)**: 最后 1-2 镜, 解决问题 / 给出惊喜 / 情感落点. 镜头拉开, 情绪满足.

3 镜 = hook + conflict + payoff (各 1 镜).
4 镜 = hook(2) + conflict + payoff.
5 镜 = hook(2) + conflict(2) + payoff.

# 镜头语言 (camera, 与后端 prompt_builder.rs enum 严格对齐)
- wide: 远景, 全景建立镜头, 静态 (开场常用, 展示世界)
- medium: 中景, 腰/膝以上, 平衡构图 (对话 / 动作常用)
- close: 近景, 胸/肩以上, 浅景深 (情绪高点)
- extreme: 特写, 面部/道具细节, 浅景深 (微观 / 道具)
- follow: 跟拍, 平滑跟随主体移动 (追逐 / 行走)
- overhead: 俯视, 鸟瞰 (转场常用, 大场面)

# 情绪颗粒度 (mood, 与后端 prompt_builder.rs enum 严格对齐)
- calm: 平静, 安静, 沉思
- tense: 紧张, 害怕, 对峙
- joyful: 开心, 兴奋, 阳光
- sad: 难过, 委屈, 失落
- epic: 史诗, 宏大, 壮阔

# 转场 (transition_to_next)
- cut: 硬切 (节奏快, 冲突常用)
- fade: 渐隐渐显 (温柔, 时间跳转)
- dissolve: 叠化 (回忆 / 想象)
- wipe: 划像 (卡通感强, 适合儿童风格)
- none: 仅最后一镜, 故事结束不需要转场

# motion 写法指导
- 用儿童能想象到的语言描述动作
- 包含"谁+做什么+在哪儿+看起来怎么样"
- 长度 1-2 句, 不要超过 80 字
- 避免成人化或复杂的摄影术语
- 例: "小启在花园里追着一只蝴蝶跑, 边跑边笑"
- 例: "小启张开手臂, 慢慢地从地面飘到半空中"

# 严格校验 (W4.6 #4)
- 任何字段缺失或枚举值不在白名单内, 都会被前端拒绝并回退到默认方案.
- 仔细检查每个字段都填全了, 尤其 character_refs 不能为空数组.

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

// ============ Skills (W10 Day 3) ============

export type Audience = 'child' | 'adult' | 'both';
export type UserMode = 'child' | 'adult';

export interface SkillSummary {
  id: string;
  name: string;
  version: string;
  enabled: boolean;
  installedAt: number;
  audience: Audience;
}

export interface MarketplaceSkill {
  id: string;
  name: string;
  version: string;
  audience: Audience;
  ageTier: number[];
  category: string;
  sizeBytes: number;
  description?: string;
  installed: boolean;
  enabled: boolean;
  creditsPerUse: number;
  dailyQuota: number;
  fromCache: boolean;
}

export interface InstallReceipt {
  skillId: string;
  version: string;
  sizeBytes: number;
  installedAt: number;
  auditId: string;
}

export async function listInstalledSkills(): Promise<SkillSummary[]> {
  return invoke<SkillSummary[]>('list_installed_skills');
}

export async function listAvailableSkills(): Promise<MarketplaceSkill[]> {
  return invoke<MarketplaceSkill[]>('list_available_skills');
}

export async function installSkill(
  skillId: string,
  parentPin: string,
): Promise<InstallReceipt> {
  return invoke<InstallReceipt>('install_skill', { skillId, parentPin });
}

export async function uninstallSkill(skillId: string): Promise<void> {
  return invoke<void>('uninstall_skill', { skillId });
}

export async function toggleSkill(skillId: string, enabled: boolean): Promise<void> {
  return invoke<void>('toggle_skill', { skillId, enabled });
}

// ============ Parent PIN (W10 Day 4) ============

export async function isParentPinSet(): Promise<boolean> {
  return invoke<boolean>('is_parent_pin_set');
}

export async function setParentPin(pin: string): Promise<void> {
  return invoke<void>('set_parent_pin', { pin });
}

export async function verifyParentPin(pin: string): Promise<boolean> {
  return invoke<boolean>('verify_parent_pin', { pin });
}

export async function resetParentPin(): Promise<void> {
  return invoke<void>('reset_parent_pin');
}

// ============ User Mode (W10 Day 4 - Part C) ============

export interface SetModeResponse {
  deviceId: string;
  mode: UserMode;
  switchedAt: number;
}

export async function getUserMode(): Promise<UserMode> {
  return invoke<UserMode>('get_user_mode');
}

export async function setUserMode(
  mode: UserMode,
  parentPin: string,
): Promise<SetModeResponse> {
  return invoke<SetModeResponse>('set_user_mode', { mode, parentPin });
}

// ============ Secrets (W11 Day 7) ============

export interface UpdateInfo {
  profile: string;
  remoteVersion: string;
  currentVersion: string | null;
}

export async function getCurrentSecretVersion(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>('get_current_secret_version');
}

export async function checkSecretsUpdate(): Promise<UpdateInfo[]> {
  return invoke<UpdateInfo[]>('check_secrets_update');
}

export async function applySecretsUpdate(
  profile: string,
  parentPin: string,
): Promise<string> {
  return invoke<string>('apply_secrets_update', { profile, parentPin });
}

export async function rollbackSecrets(
  profile: string,
  toVersion: string,
): Promise<void> {
  return invoke<void>('rollback_secrets', { profile, toVersion });
}
