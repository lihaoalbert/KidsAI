// v2 视频导演流程状态机 (W5 升级)
// - 6 阶状态机:点子 → 主角 → 画风 → 分镜 → 试拍 → 定稿
// - 锁定命题 (locked_props): 每阶 ✓ 拍板后写入 locked_props.subject / story_core / art_style,
//   下游 LLM (尤其分镜) 必须基于已锁定命题生成, 保证故事连贯。
// - 可回退 (cursor + history): 顶部进度胶囊可点击跳回任一已完成的阶,
//   跳回时还原当时的快照, 下游阶标 ⚪ stale, 需重新 ✓ 拍板才会重生。
//
// 设计原则 + 失败兜底沿用 v1. 详见 memory/video_director_flow.md

import { create } from 'zustand';
import {
  CREDITS,
  DIRECTOR_PLAN_SYSTEM_PROMPT,
  VIDEO_MODEL,
  downloadAsset,
  listCharacters,
  listStyles,
  parseDirectorPlan,
  runAgent,
  saveCreation,
  type Character,
  type DirectorPlan,
  type StoryBeat,
  type ShotCamera,
  type ShotMood,
  type ShotTransition,
  type StylePreset,
} from '../api/tauri';
import { useAssetStore } from './assetStore';
import { useTokenStore } from './tokenStore';
import { useProjectStore } from './projectStore';
import { useToastStore } from './toastStore';

export type DirectorStage = 1 | 2 | 3 | 4 | 5 | 6;

/** 阶段1 故事骨架四槽：谁 / 想要 / 但是 / 结局味道 */
export type StorySlot = 'who' | 'wants' | 'but' | 'ending';

/** W9: 故事骨架 7 维 — 数据是 first-class，不再是步骤 */
export type StoryTone = 'playful' | 'epic' | 'healing' | 'comedy' | 'mystery' | 'serious' | 'romantic';
export interface StorySpine {
  core: string;          // 故事内核：「勇气战胜恐惧」「友情可贵」
  conflict: string;      // 冲突：小火龙 vs 黑暗大怪兽
  world: string;         // 世界观：火山世界 / 现代都市 / 太空站
  tone: StoryTone;       // 调性
  audience: string;      // 适龄：6-9 / 10-13 / 14+
  theme_color: string;   // 主色调
  ending_moral: string;  // 结尾寓意
}

/** W9: LLM 充实的详细剧本（3-5 段） */
export interface StoryNarrative {
  paragraphs: string[];  // 3-5 段详细描写
  updatedAt: number;     // 上次刷新时间（毫秒）
}

export interface Story {
  who: string;
  wants: string;
  but: string;
  ending: string;
  /** W9: 7 维故事骨架 */
  spine: StorySpine;
  /** W9: LLM 充实的详细剧本 */
  narrative: StoryNarrative;
}

/** 阶段5 单镜微调（轻量：档位/预设，绝不数值） */
export interface ShotFx {
  speed?: 'slow' | 'normal' | 'fast';
  sound?: string;
  filter?: string;
}

/** 阶段2 主角微调（轻量：色块/档位/预设） */
export interface CharacterTweak {
  color?: string;
  size?: 'S' | 'M' | 'L';
  expression?: string;
}

export interface DirectorShot {
  id: string;
  description: string;
  motion: string;
  previewUrl: string | null;
  seed: number;
  previewing: boolean; // ⑤ 当前分镜是否在试拍
  fx?: ShotFx; // ⑤ 轻量微调
  /// W4.6 #4: 5 拍节奏 — hook / conflict / payoff
  beat: StoryBeat;
  /// W4.6 #4: 情绪颗粒度 — 喂给 build_seedance_prompt 的 [Mood] 行
  mood: ShotMood;
  /// W4.6 #4: 镜头语言 — 喂给 build_seedance_prompt 的 [Camera] 行
  camera: ShotCamera;
  /// W4.6 #4: 这镜涉及哪些角色 id (多角色故事必填, 单角色简化为 [character_id])
  characterRefs: string[];
  /// W4.6 #4: 与下一镜的转场 (最后一镜固定 'none')
  transitionToNext: ShotTransition;
  /// W4.6 #4: 同 session 内跨镜共享 seed (锁定角色一致性)
  seedSession?: number;
  /// W9: 镜头语言 6 维（与 camera 字段并存，camera 是简版，cinematography 是完整版）
  cinematography?: ShotCinematography;
  /// W9: 声音设计 4 维
  soundDesign?: ShotSoundDesign;
  /// W9: 这一镜里每个角色用哪个形态 (characterId -> formId)
  characterForms?: Record<string, string>;
}

/** W9: 镜头语言 6 维 — 与 W4.6 #4 ShotCamera 并存，cinematography 是完整版 */
export type ShotType = 'extreme-wide' | 'wide' | 'medium' | 'close-up' | 'extreme-close-up' | 'over-shoulder' | 'pov' | 'aerial';
export type ShotAngle = 'eye-level' | 'low' | 'high' | 'dutch' | 'birds-eye' | 'worms-eye';
export type ShotMovement = 'static' | 'pan' | 'tilt' | 'dolly' | 'track' | 'crane' | 'handheld' | 'zoom';
export type ShotTransitionStyle = 'cut' | 'fade' | 'dissolve' | 'wipe' | 'match' | 'jump';
export type ShotLighting = 'natural' | 'golden-hour' | 'blue-hour' | 'high-key' | 'low-key' | 'silhouette' | 'neon';
export type ShotColorGrade = 'warm' | 'cool' | 'desaturated' | 'high-saturation' | 'noir' | 'pastel';

export interface ShotCinematography {
  shot_type: ShotType;
  angle: ShotAngle;
  movement: ShotMovement;
  transition_in: ShotTransitionStyle;
  transition_out: ShotTransitionStyle;
  lighting: ShotLighting;
  color_grade: ShotColorGrade;
}

/** W9: 声音设计 4 维 */
export type BgmMood = 'none' | 'playful' | 'tense' | 'epic' | 'sad' | 'triumphant' | 'mysterious';
export interface SfxCue {
  timeSec: number;       // 0..shot_duration
  kind: string;          // 'footstep' | 'roar' | 'wind' | 'magic' | 'fire' | 'splash' | 'magic-chime'
  description: string;
}
export interface ShotSoundDesign {
  bgm_mood: BgmMood;
  bgm_volume: number;        // 0..100
  sfx_cues: SfxCue[];
  voice_direction: string;   // 配音语气指令
  silence_beat: boolean;     // 是否静音节拍
}

/** W9: 角色形态 + 微表情 — 同一角色可以"幼儿/少年/战斗/受伤/胜利"等形态 */
export interface CharacterForm {
  id: string;            // 'default' | 'battle' | 'injured' | 'victory' | 自定义
  name: string;          // 显示名：「战斗形态」
  prompt: string;        // 该形态在视频生成 prompt 中的修饰
  imageUrl?: string;
}
export interface CharacterExpression {
  id: string;            // 'happy' | 'angry' | 'curious' | 'determined' | 自定义
  name: string;
  imageUrl?: string;
}

/** W9: 角色扩展元数据 — 与后端 Character 解耦，前端可自由扩展 */
export interface CharacterMeta {
  forms: CharacterForm[];
  expressions: CharacterExpression[];
  voiceId?: string;
  voiceSampleUrl?: string;
}

/**
 * 已锁定命题 (W5 修复 ②)
 * 每一阶 ✓ 拍板后写入对应字段. 下游 LLM 调用前置拼接【上下文:已锁定命题】块,
 * 让分镜 step 强制服务于 story_core, 不会再生成跟主角/故事无关的镜头.
 */
export interface LockedProps {
  subject?: string; // 例: "主角是小启(9岁小猫女孩,黄短发+黄T恤+小围巾), 中等大小"
  story_core?: string; // 例: "小恐龙想要找回火焰,被大冰山挡住,最后和朋友一起闯过去"
  art_style?: string; // 例: "明亮卡通,色彩饱和,线条干净"
}

/** 阶段4–6 进入下游时, 下游状态(分镜/视频)的"是否有效"标记 */
export interface DirectorHistoryEntry {
  stage: DirectorStage;
  /// 该阶段的"决策后"快照(idea + story + character + style + shots + locked_props).
  /// 回退到该阶段时还原.
  snapshot: {
    idea: string;
    story: Story;
    character: Character | null;
    characterTweak: CharacterTweak;
    characterMetas: Record<string, CharacterMeta>;
    style: StylePreset | null;
    shots: DirectorShot[];
    locked_props: LockedProps;
  };
  decided_at: number;
  /// 回退后再前进时该阶段是否已重新生成 (false = 仍为旧版, UI 标 ⚪ stale)
  stale: boolean;
}

export interface DirectorState {
  // —— 状态机 cursor + history (W5 修复 ③) ——
  /// 当前所在的阶段 (1-6). 用 cursor 而非单调 stage, 支持 goBackTo
  cursor: DirectorStage;
  /// 已决策的历史快照. 长度 0..5. history[i] 对应第 (i+1) 阶决策后的状态.
  history: DirectorHistoryEntry[];

  // —— 当前显示用的运行时字段 (来自 history[cursor-1] 的 snapshot 或运行中编辑) ——
  idea: string;
  story: Story;
  character: Character | null;
  characterTweak: CharacterTweak;
  style: StylePreset | null;
  shots: DirectorShot[];
  finalVideoUrl: string | null;
  isLLMRunning: boolean;
  isVideoRunning: boolean;
  error: string | null;
  /// 已锁定的命题, 供下游 LLM 调用拼接到 system_prompt
  locked_props: LockedProps;
  /// W6 C4: 视频引擎 — 默认 Seedance; 可切 MiniMax hailuo-02 (备用)
  videoEngine: 'seedance' | 'hailuo';
  /// W6 C2: 主角的声音 id (可选, voice_clone 后存)
  voiceId: string | null;
  /// W9: 角色扩展元数据（形态/微表情/配音）— 按 character.id 索引
  characterMetas: Record<string, CharacterMeta>;
  /// W9: Agent 长期对话历史（用户可随时打字，agent 永远倾听）
  chatHistory: ChatMessage[];
  /// W9: 学币会话累计（每次扣费/退款实时累加，供 ResultPane 显示）
  sessionCredits: number;
  /// W9: 待确认的代价（costModel 弹层等待用户响应）
  pendingConfirmation: PendingConfirmation | null;
  /// W9: 当前激活的故事版本（Master 模式多版本切换）
  activeVersionId: string;
  /// W9: 已保存的版本快照
  versions: Record<string, VersionSnapshot>;

  // actions
  reset(): void;
  /** 进入阶段 s: 先把当前状态快照入 history[cursor-1], 再设 cursor=s */
  goToStage(s: DirectorStage): void;
  /** 回退到阶段 idx (1-based). 还原 history[idx-1] 快照, 后续阶标 stale=true */
  goBackTo(idx: DirectorStage): boolean;
  setIdea(text: string): void;
  setStorySlot(slot: StorySlot, value: string): void;
  /** W9: 设置故事骨架 7 维任一字段 */
  setSpineField<K extends keyof StorySpine>(field: K, value: StorySpine[K]): void;
  /** W9: 替换整个 narrative（LLM 充实产物） */
  setNarrative(paragraphs: string[]): void;
  /** 把四槽拼成喂给 LLM 的骨架句 */
  assembledIdea(): string;
  setCharacter(c: Character): void;
  setCharacterTweak(patch: Partial<CharacterTweak>): void;
  setStyle(s: StylePreset): void;
  /** W6 C4: 切换视频引擎 (Seedance 默认 / MiniMax hailuo-02 备用) */
  setVideoEngine(engine: 'seedance' | 'hailuo'): void;
  /** W6 C2: 存 voice_clone 返回的 voice_id */
  setVoiceId(id: string | null): void;
  /** 显式写入"已锁定命题"片段. 在每一阶 ✓ 拍板时由 studioStore 调用. */
  lockSubject(): void;
  lockStoryCore(): void;
  lockArtStyle(): void;
  updateShot(
    id: string,
    patch: Partial<
      Pick<
        DirectorShot,
        | 'description'
        | 'motion'
        | 'beat'
        | 'mood'
        | 'camera'
        | 'characterRefs'
        | 'transitionToNext'
      >
    >,
  ): void;
  /** W9: 改分镜提示词（不动 status），用户确认 costModel 后调用 */
  editShotPrompt(id: string, newDescription: string, newMotion: string): void;
  /** W9: 重拍一镜（清 previewUrl，重置 previewing） */
  reRenderShot(id: string): Promise<void>;
  /** W9: 设置一镜的镜头语言 6 维 */
  setShotCinematography(id: string, patch: Partial<ShotCinematography>): void;
  /** W9: 设置一镜的声音设计 4 维 */
  setShotSoundDesign(id: string, patch: Partial<ShotSoundDesign>): void;
  /** W9: 选择该镜里某角色用的形态 */
  setShotCharacterForm(shotId: string, characterId: string, formId: string): void;
  /** W9: 给某角色加形态 */
  addCharacterForm(characterId: string, form: CharacterForm): void;
  /** W9: 给某角色加微表情 */
  addCharacterExpression(characterId: string, expression: CharacterExpression): void;
  setShotFx(id: string, patch: Partial<ShotFx>): void;
  moveShot(id: string, dir: 'up' | 'down'): void;
  /** W9: 插入新镜到指定位置 */
  insertShot(at: number, shot: Partial<DirectorShot>): void;
  /** W9: 删除一镜 */
  deleteShot(id: string): void;
  /** W9: Agent 长期对话 — 用户随便打的文字落 chatHistory */
  chat(message: string): void;
  /** W9: 确认 / 取消待执行的代价 */
  confirmPending(): void;
  cancelPending(): void;
  /** W9: 多版本 — 保存当前快照为版本 */
  saveVersion(name: string): void;
  /** W9: 多版本 — 切换到某版本 */
  switchVersion(id: string): void;
  /** ① → ②③④: 1 次 LLM 出 DirectorPlan, 失败重试 1 次, 二次失败用兜底 */
  runPlanGeneration(idea: string): Promise<void>;
  /** ⑤: 拍单条分镜(默认走 mini + 学分扣/退) */
  runPreviewShot(shotId: string): Promise<void>;
  /** ⑥: 2.0 出 1 条高清 + 入库 */
  runFinalize(planTitle: string): Promise<void>;
  /** W9: 重合成视频（用最新分镜，覆盖旧 finalVideoUrl） */
  reFinalize(): Promise<void>;
}

/** W9: Agent 长期对话消息 */
export interface ChatMessage {
  id: string;
  role: 'kid' | 'ai' | 'system';
  text: string;
  timestamp: number;
  /** agent 主动建议时附带的 cost 评估摘要（UI 用来显示代价） */
  costHint?: string;
}

/** W9: 待确认的代价（costModel 弹层） */
export interface PendingConfirmation {
  id: string;
  change: string;         // 'who' | 'spine.world' | 'shot.cinematography' | ...
  description: string;    // '改主角'
  invalidates: string[];  // ['character.threeView', 'shots[*].prompt', 'shots[*].preview', 'final']
  credits: number;
  seconds: number;
  rationale: string;
  /** 实际执行的回调描述（前端用来调对应 action） */
  executeHint: string;
}

/** W9: 多版本快照 — Master 模式保存/对比 */
export interface VersionSnapshot {
  id: string;
  name: string;
  createdAt: number;
  idea: string;
  story: Story;
  character: Character | null;
  characterTweak: CharacterTweak;
  characterMetas: Record<string, CharacterMeta>;
  style: StylePreset | null;
  shots: DirectorShot[];
  locked_props: LockedProps;
  finalVideoUrl: string | null;
}

/// W4.6 #4: FALLBACK_PLAN 也要带齐 6 字段 — 兜底也要满足严格 schema,
/// 否则前端校验兜底数据时会再次失败,陷入死循环.
const FALLBACK_PLAN: DirectorPlan = {
  idea: '一个简单有趣的小动画',
  character_id: 'xiaoqi',
  style_id: 'cartoon',
  shots: [
    {
      description: '小启站在花园里,抬头看着天空',
      motion: '小启站在花园里,抬头看着天空,微风吹动头发',
      beat: 'hook',
      mood: 'joyful',
      camera: 'wide',
      character_refs: ['xiaoqi'],
      transition_to_next: 'fade',
    },
    {
      description: '小启张开手臂,开始慢慢地飘起来',
      motion: '小启张开手臂, 慢慢地从地面飘到半空中',
      beat: 'conflict',
      mood: 'tense',
      camera: 'medium',
      character_refs: ['xiaoqi'],
      transition_to_next: 'cut',
    },
    {
      description: '小启在云朵之间穿行,露出开心的笑容',
      motion: '小启在云朵之间穿行, 开心地笑',
      beat: 'payoff',
      mood: 'epic',
      camera: 'overhead',
      character_refs: ['xiaoqi'],
      transition_to_next: 'none',
    },
  ],
};

function genId(prefix: string) {
  return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
}

const EMPTY_SPINE: StorySpine = {
  core: '',
  conflict: '',
  world: '',
  tone: 'playful',
  audience: '',
  theme_color: '',
  ending_moral: '',
};

const EMPTY_NARRATIVE: StoryNarrative = {
  paragraphs: [],
  updatedAt: 0,
};

const EMPTY_CINEMATOGRAPHY: ShotCinematography = {
  shot_type: 'medium',
  angle: 'eye-level',
  movement: 'static',
  transition_in: 'cut',
  transition_out: 'cut',
  lighting: 'natural',
  color_grade: 'warm',
};

const EMPTY_SOUND_DESIGN: ShotSoundDesign = {
  bgm_mood: 'none',
  bgm_volume: 30,
  sfx_cues: [],
  voice_direction: '',
  silence_beat: false,
};

const EMPTY_CHARACTER_META: CharacterMeta = {
  forms: [],
  expressions: [],
  voiceId: undefined,
  voiceSampleUrl: undefined,
};

const EMPTY_STORY: Story = {
  who: '',
  wants: '',
  but: '',
  ending: '',
  spine: { ...EMPTY_SPINE },
  narrative: { ...EMPTY_NARRATIVE, paragraphs: [] },
};
const EMPTY_LOCKED: LockedProps = { subject: undefined, story_core: undefined, art_style: undefined };

function freshSeed(): number {
  // Seedance seed 范围(参考其文档示例): 用 32 位正整数
  return Math.floor(Math.random() * 0x7fffffff);
}

function shotsFromPlan(plan: DirectorPlan): DirectorShot[] {
  return plan.shots.map((s, idx) => {
    // W9: 从 LLM 的简版 camera/mood 推导完整版 cinematography / soundDesign
    const cinematography: ShotCinematography = {
      ...EMPTY_CINEMATOGRAPHY,
      shot_type: shotTypeFromCamera(s.camera),
      transition_out: s.transition_to_next === 'none' ? 'cut' : s.transition_to_next,
    };
    const soundDesign: ShotSoundDesign = {
      ...EMPTY_SOUND_DESIGN,
      bgm_mood: bgmMoodFromMood(s.mood),
      voice_direction: voiceFromBeat(s.beat),
    };
    return {
      id: genId('shot'),
      description: s.description,
      motion: s.motion,
      previewUrl: null,
      seed: freshSeed(),
      previewing: false,
      // W4.6 #4: LLM 已校验过 6 字段, 兜底 (validateDirectorPlan 通过) 才能到这
      beat: s.beat,
      mood: s.mood,
      camera: s.camera,
      characterRefs: [...s.character_refs],
      transitionToNext: s.transition_to_next,
      // W4.6 #4: 用 index+session_seed_from_id 算同 plan 内统一 seed_session,
      // 跨镜锁定角色一致性. 后端 run_loop 会用 session_seed_from_id(hash(session_id))
      // 同算法, 但前端只是 hint, 真正锁定由后端 ToolContext.seed_session 负责.
      seedSession: idx === 0 ? Math.floor(Math.random() * 0x7fffffff) : undefined,
      // W9: 完整版
      cinematography,
      soundDesign,
      characterForms: {},
    };
  });
}

/** W4.6 #4 ShotCamera 字符串 → W9 ShotType 映射 (LLM 写广义的 camera, 前端展示成 6 维之一) */
function shotTypeFromCamera(camera: string): ShotType {
  switch (camera) {
    case 'wide':
      return 'wide';
    case 'medium':
      return 'medium';
    case 'close':
      return 'close-up';
    case 'overhead':
      return 'aerial';
    default:
      return 'medium';
  }
}

/** W4.6 #4 ShotMood → W9 BgmMood — 情绪决定 BGM 情绪 */
function bgmMoodFromMood(mood: string): BgmMood {
  switch (mood) {
    case 'joyful':
    case 'playful':
      return 'playful';
    case 'tense':
    case 'mysterious':
      return 'tense';
    case 'epic':
    case 'triumphant':
      return 'epic';
    case 'sad':
    case 'reflective':
      return 'sad';
    default:
      return 'none';
  }
}

/** W4.6 #4 StoryBeat → 默认配音语气 */
function voiceFromBeat(beat: string): string {
  switch (beat) {
    case 'hook':
      return '明亮、好奇、有吸引力';
    case 'conflict':
      return '紧张、坚定、推动感';
    case 'payoff':
      return '温暖、满足、有回响';
    default:
      return '自然';
  }
}

/** 拍当前运行时状态快照, 用于 goToStage 入 history. */
function snapshotOf(state: DirectorState) {
  return {
    idea: state.idea,
    story: {
      ...state.story,
      spine: { ...state.story.spine },
      narrative: { ...state.story.narrative, paragraphs: [...state.story.narrative.paragraphs] },
    },
    character: state.character,
    characterTweak: { ...state.characterTweak },
    characterMetas: Object.fromEntries(
      Object.entries(state.characterMetas).map(([k, v]) => [k, { ...v, forms: [...v.forms], expressions: [...v.expressions] }]),
    ),
    style: state.style,
    shots: state.shots.map((s) => ({ ...s })),
    locked_props: { ...state.locked_props },
  };
}

/** 把 stage 之前(含)尚未决策的所有下游阶标 stale=true, 提示 UI 显示 ⚪ 灰. */
function markDownstreamStale(history: DirectorHistoryEntry[], fromStage: DirectorStage): DirectorHistoryEntry[] {
  return history.map((e) => (e.stage > fromStage ? { ...e, stale: true } : e));
}

export const useDirectorStore = create<DirectorState>((set, get) => ({
  cursor: 1,
  history: [],
  idea: '',
  story: {
    ...EMPTY_STORY,
    spine: { ...EMPTY_SPINE },
    narrative: { ...EMPTY_NARRATIVE, paragraphs: [] },
  },
  character: null,
  characterTweak: {},
  characterMetas: {},
  style: null,
  shots: [],
  finalVideoUrl: null,
  isLLMRunning: false,
  isVideoRunning: false,
  error: null,
  locked_props: { ...EMPTY_LOCKED },
  videoEngine: 'seedance',
  voiceId: null,
  // W9: 长程 chat + cost + 版本 — 初始值
  chatHistory: [],
  sessionCredits: 0,
  pendingConfirmation: null,
  activeVersionId: 'main',
  versions: {
    main: {
      id: 'main',
      name: '主版本',
      createdAt: Date.now(),
      idea: '',
      story: { ...EMPTY_STORY, spine: { ...EMPTY_SPINE }, narrative: { ...EMPTY_NARRATIVE, paragraphs: [] } },
      character: null,
      characterTweak: {},
      characterMetas: {},
      style: null,
      shots: [],
      locked_props: { ...EMPTY_LOCKED },
      finalVideoUrl: null,
    },
  },

  reset: () =>
    set({
      cursor: 1,
      history: [],
      idea: '',
      story: {
        ...EMPTY_STORY,
        spine: { ...EMPTY_SPINE },
        narrative: { ...EMPTY_NARRATIVE, paragraphs: [] },
      },
      character: null,
      characterTweak: {},
      characterMetas: {},
      style: null,
      shots: [],
      finalVideoUrl: null,
      isLLMRunning: false,
      isVideoRunning: false,
      error: null,
      locked_props: { ...EMPTY_LOCKED },
      videoEngine: 'seedance',
      voiceId: null,
      // W9: 长程 chat + cost + 版本 — reset 时一并清
      chatHistory: [],
      sessionCredits: 0,
      pendingConfirmation: null,
      activeVersionId: 'main',
      versions: { main: { id: 'main', name: '主版本', createdAt: 0, idea: '', story: { ...EMPTY_STORY, spine: { ...EMPTY_SPINE }, narrative: { ...EMPTY_NARRATIVE, paragraphs: [] } }, character: null, characterTweak: {}, characterMetas: {}, style: null, shots: [], locked_props: { ...EMPTY_LOCKED }, finalVideoUrl: null } },
    }),

  goToStage: (s) => {
    const state = get();
    // 不允许跳到比 cursor 还早的阶段 (回退必须走 goBackTo, 会标 stale)
    if (s <= state.cursor) {
      set({ cursor: s });
      return;
    }
    // 把 cursor 处决策的状态写入 history[cursor-1]
    const newHistory = [...state.history];
    const existingIdx = newHistory.findIndex((e) => e.stage === state.cursor);
    const entry: DirectorHistoryEntry = {
      stage: state.cursor,
      snapshot: snapshotOf(state),
      decided_at: Date.now(),
      stale: false,
    };
    if (existingIdx >= 0) newHistory[existingIdx] = entry;
    else newHistory.push(entry);
    // 跳到下游: 下游未决策的阶仍是 stale (默认), 无需特殊处理
    set({ cursor: s, history: newHistory });
  },

  goBackTo: (idx) => {
    const state = get();
    // 只允许回退到已决策的阶
    const target = state.history.find((e) => e.stage === idx);
    if (!target) return false;
    const snap = target.snapshot;
    set({
      cursor: idx,
      idea: snap.idea,
      story: {
        ...snap.story,
        spine: { ...snap.story.spine },
        narrative: { ...snap.story.narrative, paragraphs: [...snap.story.narrative.paragraphs] },
      },
      character: snap.character,
      characterTweak: { ...snap.characterTweak },
      characterMetas: Object.fromEntries(
        Object.entries(snap.characterMetas ?? {}).map(([k, v]) => [k, { ...v, forms: [...v.forms], expressions: [...v.expressions] }]),
      ),
      style: snap.style,
      shots: snap.shots.map((s) => ({ ...s })),
      locked_props: { ...snap.locked_props },
      // 下游全部标 stale
      history: markDownstreamStale(state.history, idx),
      error: null,
      finalVideoUrl: null,
    });
    return true;
  },

  setIdea: (text) => set({ idea: text }),

  setStorySlot: (slot, value) =>
    set((state) => ({ story: { ...state.story, [slot]: value } })),

  assembledIdea: () => {
    const { who, wants, but, ending } = get().story;
    const parts: string[] = [];
    if (who) parts.push(who);
    if (wants) parts.push(`想要${wants}`);
    if (but) parts.push(`但是${but}`);
    if (ending) parts.push(`最后${ending}`);
    return parts.join('，');
  },

  setCharacter: (c) => {
    set({ character: c, characterTweak: {} });
    // W8 M1-D: 标准照落本地 projects/<id>/character/<id>-stand.png
    const ref = c.standardImageUrl ?? c.referenceImageUrl;
    if (ref) void enqueueLocalDownload(ref, 'image', `character/${c.id}-stand.png`);
  },

  setCharacterTweak: (patch) =>
    set((state) => ({ characterTweak: { ...state.characterTweak, ...patch } })),

  setStyle: (s) => {
    set({ style: s });
    // W8 M1-D: 风格预设暂不附带图片 URL, 留待 image-01 出图后单独入队
  },

  setVideoEngine: (engine) => set({ videoEngine: engine }),

  setVoiceId: (id) => set({ voiceId: id }),

  lockSubject: () => {
    const { character, characterTweak } = get();
    if (!character) return;
    const size = characterTweak.size ? `, 体型${characterTweak.size}` : '';
    const color = characterTweak.color ? `, 主色${characterTweak.color}` : '';
    const expr = characterTweak.expression ? `, 表情${characterTweak.expression}` : '';
    const subject = `主角是${character.name}(${character.description})${size}${color}${expr}`;
    set((s) => ({ locked_props: { ...s.locked_props, subject } }));
  },

  lockStoryCore: () => {
    const core = get().assembledIdea();
    if (!core) return;
    set((s) => ({ locked_props: { ...s.locked_props, story_core: core } }));
  },

  lockArtStyle: () => {
    const { style } = get();
    if (!style) return;
    const art_style = `${style.name}, ${style.description}`;
    set((s) => ({ locked_props: { ...s.locked_props, art_style } }));
  },

  updateShot: (id, patch) =>
    set((state) => ({
      shots: state.shots.map((sh) => (sh.id === id ? { ...sh, ...patch } : sh)),
    })),

  setShotFx: (id, patch) =>
    set((state) => ({
      shots: state.shots.map((sh) =>
        sh.id === id ? { ...sh, fx: { ...sh.fx, ...patch } } : sh,
      ),
    })),

  moveShot: (id, dir) =>
    set((state) => {
      const idx = state.shots.findIndex((s) => s.id === id);
      if (idx < 0) return state;
      const target = dir === 'up' ? idx - 1 : idx + 1;
      if (target < 0 || target >= state.shots.length) return state;
      const next = [...state.shots];
      [next[idx], next[target]] = [next[target], next[idx]];
      return { shots: next };
    }),

  // ============================ W9 actions ============================

  setSpineField: (field, value) =>
    set((state) => ({
      story: { ...state.story, spine: { ...state.story.spine, [field]: value } },
    })),

  setNarrative: (paragraphs) =>
    set((state) => ({
      story: {
        ...state.story,
        narrative: { paragraphs, updatedAt: Date.now() },
      },
    })),

  editShotPrompt: (id, newDescription, newMotion) =>
    set((state) => ({
      shots: state.shots.map((sh) =>
        sh.id === id ? { ...sh, description: newDescription, motion: newMotion } : sh,
      ),
    })),

  reRenderShot: async (id) => {
    // 重拍一镜：清 previewUrl + 重置 previewing → 复用 runPreviewShot 的链路
    set((state) => ({
      shots: state.shots.map((sh) =>
        sh.id === id ? { ...sh, previewUrl: null, previewing: false } : sh,
      ),
    }));
    await get().runPreviewShot(id);
  },

  setShotCinematography: (id, patch) =>
    set((state) => ({
      shots: state.shots.map((sh) =>
        sh.id === id
          ? { ...sh, cinematography: { ...(sh.cinematography ?? EMPTY_CINEMATOGRAPHY), ...patch } }
          : sh,
      ),
    })),

  setShotSoundDesign: (id, patch) =>
    set((state) => ({
      shots: state.shots.map((sh) =>
        sh.id === id
          ? { ...sh, soundDesign: { ...(sh.soundDesign ?? EMPTY_SOUND_DESIGN), ...patch } }
          : sh,
      ),
    })),

  setShotCharacterForm: (shotId, characterId, formId) =>
    set((state) => ({
      shots: state.shots.map((sh) =>
        sh.id === shotId
          ? { ...sh, characterForms: { ...(sh.characterForms ?? {}), [characterId]: formId } }
          : sh,
      ),
    })),

  addCharacterForm: (characterId, form) =>
    set((state) => {
      const prev = state.characterMetas[characterId] ?? { ...EMPTY_CHARACTER_META, forms: [], expressions: [] };
      // 覆盖同 id 形态
      const forms = prev.forms.filter((f) => f.id !== form.id).concat(form);
      return {
        characterMetas: {
          ...state.characterMetas,
          [characterId]: { ...prev, forms },
        },
      };
    }),

  addCharacterExpression: (characterId, expression) =>
    set((state) => {
      const prev = state.characterMetas[characterId] ?? { ...EMPTY_CHARACTER_META, forms: [], expressions: [] };
      const expressions = prev.expressions.filter((e) => e.id !== expression.id).concat(expression);
      return {
        characterMetas: {
          ...state.characterMetas,
          [characterId]: { ...prev, expressions },
        },
      };
    }),

  insertShot: (at, partial) =>
    set((state) => {
      const newShot: DirectorShot = {
        id: genId('shot'),
        description: partial.description ?? '',
        motion: partial.motion ?? '',
        previewUrl: null,
        seed: freshSeed(),
        previewing: false,
        beat: partial.beat ?? 'hook',
        mood: partial.mood ?? 'joyful',
        camera: partial.camera ?? 'medium',
        characterRefs: partial.characterRefs ?? (state.character ? [state.character.id] : []),
        transitionToNext: partial.transitionToNext ?? 'cut',
        seedSession: state.shots[0]?.seedSession,
        cinematography: partial.cinematography ?? { ...EMPTY_CINEMATOGRAPHY },
        soundDesign: partial.soundDesign ?? { ...EMPTY_SOUND_DESIGN },
        characterForms: {},
      };
      const clampedAt = Math.max(0, Math.min(at, state.shots.length));
      const next = [...state.shots];
      next.splice(clampedAt, 0, newShot);
      return { shots: next };
    }),

  deleteShot: (id) =>
    set((state) => ({
      shots: state.shots.filter((sh) => sh.id !== id),
    })),

  chat: (message) => {
    const text = message.trim();
    if (!text) return;
    const userMsg: ChatMessage = {
      id: genId('chat'),
      role: 'kid',
      text,
      timestamp: Date.now(),
    };
    set((state) => ({ chatHistory: [...state.chatHistory, userMsg] }));
  },

  confirmPending: () => {
    const state = get();
    const pending = state.pendingConfirmation;
    if (!pending) return;
    // 执行由调用方监听 pendingConfirmation 变化 + 读 executeHint 决定调哪个 action
    // 这里只清 pending + 累加 sessionCredits
    set((s) => ({
      sessionCredits: s.sessionCredits + pending.credits,
      pendingConfirmation: null,
    }));
  },

  cancelPending: () => set({ pendingConfirmation: null }),

  saveVersion: (name) => {
    const state = get();
    const id = genId('ver');
    const snap: VersionSnapshot = {
      id,
      name: name.trim() || `版本 ${new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`,
      createdAt: Date.now(),
      idea: state.idea,
      story: {
        ...state.story,
        spine: { ...state.story.spine },
        narrative: { ...state.story.narrative, paragraphs: [...state.story.narrative.paragraphs] },
      },
      character: state.character,
      characterTweak: { ...state.characterTweak },
      characterMetas: Object.fromEntries(
        Object.entries(state.characterMetas).map(([k, v]) => [k, { ...v, forms: [...v.forms], expressions: [...v.expressions] }]),
      ),
      style: state.style,
      shots: state.shots.map((s) => ({ ...s })),
      locked_props: { ...state.locked_props },
      finalVideoUrl: state.finalVideoUrl,
    };
    set((s) => ({ versions: { ...s.versions, [id]: snap }, activeVersionId: id }));
  },

  switchVersion: (id) => {
    const state = get();
    const snap = state.versions[id];
    if (!snap) return;
    set({
      activeVersionId: id,
      idea: snap.idea,
      story: {
        ...snap.story,
        spine: { ...snap.story.spine },
        narrative: { ...snap.story.narrative, paragraphs: [...snap.story.narrative.paragraphs] },
      },
      character: snap.character,
      characterTweak: { ...snap.characterTweak },
      characterMetas: Object.fromEntries(
        Object.entries(snap.characterMetas ?? {}).map(([k, v]) => [k, { ...v, forms: [...v.forms], expressions: [...v.expressions] }]),
      ),
      style: snap.style,
      shots: snap.shots.map((s) => ({ ...s })),
      locked_props: { ...snap.locked_props },
      finalVideoUrl: snap.finalVideoUrl,
      error: null,
    });
  },

  runPlanGeneration: async (idea) => {
    if (!idea.trim()) {
      set({ error: '先说说你想拍什么' });
      return;
    }
    set({ isLLMRunning: true, error: null, idea });

    // 拉角色 + 风格清单(兜底需要)
    const [chars, styles] = await Promise.all([
      listCharacters().then((r) => r ?? []).catch(() => [] as Character[]),
      listStyles().then((r) => r ?? []).catch(() => [] as StylePreset[]),
    ]);

    // 把已锁定的命题拼接到 system_prompt 前 (W5 修复 ②)
    const locked = get().locked_props;
    const ctxLines: string[] = [];
    if (locked.subject) ctxLines.push(`已锁定的主角: ${locked.subject}`);
    if (locked.story_core) ctxLines.push(`已锁定的故事核心: ${locked.story_core}`);
    if (locked.art_style) ctxLines.push(`已锁定的画风: ${locked.art_style}`);
    const contextChunk = ctxLines.length
      ? `\n\n【上下文: 已锁定的命题(必须遵循)】\n${ctxLines.join('\n')}\n\n硬约束: 每个分镜(description + motion)都必须服务于已锁定的故事核心, 主角必须是已锁定的主角, 风格必须与已锁定的画风一致。不得凭空生成跟主角/故事无关的镜头。`
      : '';
    const planSystemPrompt = DIRECTOR_PLAN_SYSTEM_PROMPT + contextChunk;

    // 尝试 LLM 解析,失败重试 1 次
    let plan: DirectorPlan | null = null;
    for (let attempt = 1; attempt <= 2; attempt++) {
      try {
        const resp = await runAgent({
          levelId: 'director_plan',
          userInput: idea,
          systemPrompt: planSystemPrompt,
          tools: [],
          characterId: undefined,
          styleId: undefined,
        });
        // runAgent 返 finalAnswer, 通常 LLM 把 DirectorPlan JSON 写在 FinalAnswer 里
        const parsed = parseDirectorPlan(resp.finalAnswer);
        if (parsed.ok && parsed.plan) {
          plan = parsed.plan;
          break;
        }
        if (attempt === 2) {
          // 最后一次失败 → 用兜底
          plan = FALLBACK_PLAN;
          set({ error: '小启没想到特别合适的方案, 用了一个默认的, 你可以改' });
        }
      } catch (e) {
        if (attempt === 2) {
          plan = FALLBACK_PLAN;
          set({ error: `小启走神了(${e}), 用了一个默认方案, 你可以改` });
        }
      }
    }

    if (!plan) plan = FALLBACK_PLAN;

    const char = chars.find((c) => c.id === plan!.character_id) ?? chars[0] ?? null;
    const style = styles.find((s) => s.id === plan!.style_id) ?? styles[0] ?? null;
    const shots = shotsFromPlan(plan);

    // LLM 生成的分镜是否与 story_core 强相关? (W5 修复 ② 故事连贯自检)
    let coherenceOk = true;
    let coherenceReason = '';
    if (locked.story_core && shots.length > 0) {
      try {
        const checkPrompt = `你是一个严格的"故事连贯性"审核员。

【已锁定的故事核心】
${locked.story_core}

【LLM 刚生成的分镜(3 镜)】
${shots
  .map((s, i) => `第 ${i + 1} 镜: ${s.description} — ${s.motion}`)
  .join('\n')}

请判断: 这 3 个分镜是否服务于已锁定的故事核心?
- 如果**每镜都跟故事核心直接相关**(同主角 + 同目标 + 同冲突/解决方向)→ 答 "YES"
- 如果**有任何一镜游离于故事核心之外**(换了主角/跑了无关场景/丢了目标/冲突出戏)→ 答 "NO" + 1 句具体原因

只回 "YES" 或 "NO: <原因>", 不要其他解释。`;
        const resp = await runAgent({
          levelId: 'director_coherence',
          userInput: '做连贯性检查',
          systemPrompt: checkPrompt,
          tools: [],
        });
        const verdict = (resp.finalAnswer ?? '').trim().toUpperCase();
        if (verdict.startsWith('NO')) {
          coherenceOk = false;
          coherenceReason = resp.finalAnswer.trim();
        }
      } catch {
        // 自检失败不挡, 仅 warn
      }
    }

    set({
      character: char,
      style,
      shots,
      isLLMRunning: false,
      // 若已有 error(LLM 失败兜底)且连贯性 ok → 不覆盖; 仅当连贯性失败才追加
      error: coherenceOk
        ? get().error
        : `分镜跟故事线有点偏: ${coherenceReason || 'LLM 自检不通过'}。你可以在分镜页直接改。`,
    });

    // 走到阶段2: 顺手把阶段1 决策(idea + story)入 history, 供回退还原
    get().goToStage(2);
  },

  runPreviewShot: async (shotId) => {
    const state = get();
    const shot = state.shots.find((s) => s.id === shotId);
    if (!shot) return;
    const { character, style, videoEngine } = state;
    if (!character) {
      set({ error: '没选主角, 无法拍' });
      return;
    }

    // 学分扣: 先扣后调, 失败退款
    const credits = CREDITS.PREVIEW_PER_SHOT;
    const ok = useTokenStore.getState().spendTokens(credits);
    if (!ok) {
      const msg = `学分不足, 拍这一镜需要 ${credits} 学分`;
      set({ error: msg });
      // P1-2: 弹 toast, 不让用户在聊天流里错过
      useToastStore.getState().push(msg, 'warn');
      return;
    }

    const seed = shot.previewUrl ? freshSeed() : shot.seed;

    set((s) => ({
      isVideoRunning: true,
      error: null,
      shots: s.shots.map((sh) => (sh.id === shotId ? { ...sh, seed, previewing: true } : sh)),
    }));

    try {
      // 把 Seedance 调用的所有信息塞进 system_prompt + userInput,
      // 强制 LLM 调 image_to_video tool 一次(因为 image_to_video tool 内部会
      // 透传 image_url/image_role/seed/model 到 adapter)
      // W4.6 #4: 拼入 mood/camera 提示, 后端 ToolContext.shot 会拿到
      const motionWithStyle = style
        ? `${shot.motion}. 视觉风格: ${style.description}.`
        : shot.motion;
      const systemPrompt = `你是 KidsAI 的视频工具。**只调用一次 image_to_video 工具**,参数:
- motion: ${motionWithStyle}
- image_url: ${character.referenceImageUrl ?? useAssetStore.getState().getUrl(`${character.id}.stand`)}
- image_role: reference_image
- model: ${videoEngine === 'hailuo' ? VIDEO_MODEL.HAILUO : VIDEO_MODEL.SEEDANCE_PREVIEW}
- seed: ${seed}
- duration: 4
- mood: ${shot.mood}        // W4.6 #4: 情绪颗粒度 (build_seedance_prompt 用)
- camera: ${shot.camera}    // W4.6 #4: 镜头语言 (build_seedance_prompt 用)
- beat: ${shot.beat}        // W4.6 #4: 节奏标记 (排障用)
然后立即返回工具的输出,不要做其他任何事。`;

      const resp = await runAgent({
        levelId: 'director_preview',
        userInput: '拍这条分镜',
        systemPrompt,
        tools: ['image_to_video'],
        characterId: character.id,
        styleId: style?.id,
      });

      // 解析 resp.assets[0].url(视频 url)
      const videoUrl = resp.assets?.[0]?.url ?? null;
      if (!videoUrl) {
        throw new Error('出片失败, 没拿到视频');
      }

      set((s) => ({
        isVideoRunning: false,
        shots: s.shots.map((sh) =>
          sh.id === shotId ? { ...sh, previewUrl: videoUrl, previewing: false } : sh,
        ),
      }));

      // W8 M1-D: 后台把视频落到本机 projects/<id>/shots/<shot.id>/preview.mp4
      // 失败不阻塞主流程, UI 仍用远端 URL; 下载完 emit asset://local 事件 → localMap 更新
      void enqueueLocalDownload(videoUrl, 'video', `shots/${shotId}/preview.mp4`);
    } catch (e) {
      // 退款
      useTokenStore.getState().addTokens(credits);
      set((s) => ({
        isVideoRunning: false,
        error: `试拍失败: ${e}`,
        shots: s.shots.map((sh) =>
          sh.id === shotId ? { ...sh, previewing: false } : sh,
        ),
      }));
    }
  },

  runFinalize: async (planTitle) => {
    const state = get();
    const { character, style, shots, idea, videoEngine } = state;
    if (!character) {
      set({ error: '没选主角, 无法定稿' });
      return;
    }
    if (shots.length === 0) {
      set({ error: '还没有分镜, 先回去排好再定稿' });
      return;
    }

    const credits = CREDITS.FINALIZE;
    const ok = useTokenStore.getState().spendTokens(credits);
    if (!ok) {
      const msg = `学分不足, 定稿需要 ${credits} 学分`;
      set({ error: msg });
      // P1-2: 弹 toast, 不让用户在聊天流里错过
      useToastStore.getState().push(msg, 'warn');
      return;
    }

    set({ isVideoRunning: true, error: null });

    try {
      // 定稿: 1 条完整 prompt(把 N 镜连起来), 2.0 端点, 用 first_frame 不用 reference(更稳定)
      // W4.6 #4: 每镜 beat/mood/camera 都拼出来, 后端按 shotIndex 拆解 (current=0)
      const firstShot = shots[0];
      const combinedMotion = shots
        .map((s, i) => `第 ${i + 1} 镜[${s.beat}/${s.mood}/${s.camera}]: ${s.motion}`)
        .join(' | ');
      const motionWithStyle = style
        ? `${combinedMotion}. 视觉风格: ${style.description}.`
        : combinedMotion;
      const systemPrompt = `你是 KidsAI 的视频工具。**只调用一次 image_to_video 工具**,参数:
- motion: ${motionWithStyle}
- image_url: ${character.referenceImageUrl ?? useAssetStore.getState().getUrl(`${character.id}.stand`)}
- image_role: first_frame
- model: ${videoEngine === 'hailuo' ? VIDEO_MODEL.HAILUO : VIDEO_MODEL.SEEDANCE_FINALIZE}
- duration: 5
- mood: ${firstShot.mood}        // W4.6 #4: 首镜情绪 (build_seedance_prompt 用)
- camera: ${firstShot.camera}    // W4.6 #4: 首镜镜头 (build_seedance_prompt 用)
- beat: ${firstShot.beat}        // W4.6 #4: 首镜节奏 (排障用)
然后立即返回工具的输出,不要做其他任何事。`;

      const resp = await runAgent({
        levelId: 'director_finalize',
        userInput: '出定稿',
        systemPrompt,
        tools: ['image_to_video'],
        characterId: character.id,
        styleId: style?.id,
      });

      const videoUrl = resp.assets?.[0]?.url ?? null;
      if (!videoUrl) {
        throw new Error('定稿失败, 没拿到视频');
      }

      // 入库
      const title = planTitle || `${character.name} - ${idea.slice(0, 20)}`;
      try {
        await saveCreation({
          id: `director_${Date.now()}`,
          levelId: 'director',
          userInput: idea,
          agentOutput: { title, character: character.id, style: style?.id, shots: shots.length },
          assets: [
            {
              type: 'video',
              url: videoUrl,
              prompt: motionWithStyle,
              tool: 'image_to_video',
              tokensCost: credits,
            },
          ],
        });
      } catch {
        // 入库失败不挡用户, 仅 log
        // eslint-disable-next-line no-console
        console.warn('saveCreation failed, but video generated ok');
      }

      set({ isVideoRunning: false, finalVideoUrl: videoUrl });

      // W8 M1-D: 落本地 exports/<date>.mp4
      const today = new Date().toISOString().slice(0, 10);
      void enqueueLocalDownload(videoUrl, 'video', `exports/final-${today}.mp4`);
    } catch (e) {
      useTokenStore.getState().addTokens(credits);
      set({ isVideoRunning: false, error: `定稿失败: ${e}` });
    }
  },

  reFinalize: async () => {
    // W9: 用当前分镜列表 + locked_props 重出视频, 覆盖旧 finalVideoUrl
    const state = get();
    if (state.shots.length === 0) {
      set({ error: '还没有分镜, 先去排分镜' });
      return;
    }
    if (!state.character) {
      set({ error: '没选主角, 无法定稿' });
      return;
    }
    const title = `${state.character.name} - ${state.idea.slice(0, 20)}`;
    await get().runFinalize(title);
  },
}));

// W8 M1-D: 把远程资产 URL 排到本机 projects/<id>/<subPath> 下载队列.
// 失败/无 project_id 时静默忽略 — UI 仍用远端 URL, 等下次重启.
async function enqueueLocalDownload(
  url: string,
  kind: 'image' | 'video' | 'audio',
  subPath: string,
): Promise<void> {
  try {
    const projectId = useProjectStore.getState().current?.id;
    if (!projectId) return;
    if (!/^https?:\/\//.test(url)) return; // data: / 本地路径跳过
    await downloadAsset(projectId, url, kind, subPath);
  } catch (e) {
    // eslint-disable-next-line no-console
    console.warn('[W8] enqueue local download failed', e);
  }
}