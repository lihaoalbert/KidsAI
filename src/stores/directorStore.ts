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
  SEEDANCE_MODEL,
  listCharacters,
  listStyles,
  parseDirectorPlan,
  runAgent,
  saveCreation,
  type Character,
  type DirectorPlan,
  type StylePreset,
} from '../api/tauri';
import { useTokenStore } from './tokenStore';

export type DirectorStage = 1 | 2 | 3 | 4 | 5 | 6;

/** 阶段1 故事骨架四槽：谁 / 想要 / 但是 / 结局味道 */
export type StorySlot = 'who' | 'wants' | 'but' | 'ending';
export interface Story {
  who: string;
  wants: string;
  but: string;
  ending: string;
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
    style: StylePreset | null;
    shots: DirectorShot[];
    locked_props: LockedProps;
  };
  decided_at: number;
  /// 回退后再前进时该阶段是否已重新生成 (false = 仍为旧版, UI 标 ⚪ stale)
  stale: boolean;
}

interface DirectorState {
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

  // actions
  reset(): void;
  /** 进入阶段 s: 先把当前状态快照入 history[cursor-1], 再设 cursor=s */
  goToStage(s: DirectorStage): void;
  /** 回退到阶段 idx (1-based). 还原 history[idx-1] 快照, 后续阶标 stale=true */
  goBackTo(idx: DirectorStage): boolean;
  setIdea(text: string): void;
  setStorySlot(slot: StorySlot, value: string): void;
  /** 把四槽拼成喂给 LLM 的骨架句 */
  assembledIdea(): string;
  setCharacter(c: Character): void;
  setCharacterTweak(patch: Partial<CharacterTweak>): void;
  setStyle(s: StylePreset): void;
  /** 显式写入"已锁定命题"片段. 在每一阶 ✓ 拍板时由 studioStore 调用. */
  lockSubject(): void;
  lockStoryCore(): void;
  lockArtStyle(): void;
  updateShot(id: string, patch: Partial<Pick<DirectorShot, 'description' | 'motion'>>): void;
  setShotFx(id: string, patch: Partial<ShotFx>): void;
  moveShot(id: string, dir: 'up' | 'down'): void;
  /** ① → ②③④: 1 次 LLM 出 DirectorPlan, 失败重试 1 次, 二次失败用兜底 */
  runPlanGeneration(idea: string): Promise<void>;
  /** ⑤: 拍单条分镜(默认走 mini + 学分扣/退) */
  runPreviewShot(shotId: string): Promise<void>;
  /** ⑥: 2.0 出 1 条高清 + 入库 */
  runFinalize(planTitle: string): Promise<void>;
}

const FALLBACK_PLAN: DirectorPlan = {
  idea: '一个简单有趣的小动画',
  character_id: 'xiaoqi',
  style_id: 'cartoon',
  shots: [
    { description: '小启站在花园里,抬头看着天空', motion: '小启站在花园里,抬头看着天空,微风吹动头发' },
    { description: '小启张开手臂,开始慢慢地飘起来', motion: '小启张开手臂, 慢慢地从地面飘到半空中' },
    { description: '小启在云朵之间穿行,露出开心的笑容', motion: '小启在云朵之间穿行, 开心地笑' },
  ],
};

function genId(prefix: string) {
  return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
}

const EMPTY_STORY: Story = { who: '', wants: '', but: '', ending: '' };
const EMPTY_LOCKED: LockedProps = { subject: undefined, story_core: undefined, art_style: undefined };

function freshSeed(): number {
  // Seedance seed 范围(参考其文档示例): 用 32 位正整数
  return Math.floor(Math.random() * 0x7fffffff);
}

function shotsFromPlan(plan: DirectorPlan): DirectorShot[] {
  return plan.shots.map((s) => ({
    id: genId('shot'),
    description: s.description,
    motion: s.motion,
    previewUrl: null,
    seed: freshSeed(),
    previewing: false,
  }));
}

/** 拍当前运行时状态快照, 用于 goToStage 入 history. */
function snapshotOf(state: DirectorState) {
  return {
    idea: state.idea,
    story: { ...state.story },
    character: state.character,
    characterTweak: { ...state.characterTweak },
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
  story: { ...EMPTY_STORY },
  character: null,
  characterTweak: {},
  style: null,
  shots: [],
  finalVideoUrl: null,
  isLLMRunning: false,
  isVideoRunning: false,
  error: null,
  locked_props: { ...EMPTY_LOCKED },

  reset: () =>
    set({
      cursor: 1,
      history: [],
      idea: '',
      story: { ...EMPTY_STORY },
      character: null,
      characterTweak: {},
      style: null,
      shots: [],
      finalVideoUrl: null,
      isLLMRunning: false,
      isVideoRunning: false,
      error: null,
      locked_props: { ...EMPTY_LOCKED },
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
      story: { ...snap.story },
      character: snap.character,
      characterTweak: { ...snap.characterTweak },
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

  setCharacter: (c) => set({ character: c, characterTweak: {} }),

  setCharacterTweak: (patch) =>
    set((state) => ({ characterTweak: { ...state.characterTweak, ...patch } })),

  setStyle: (s) => set({ style: s }),

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
    const { character, style } = state;
    if (!character) {
      set({ error: '没选主角, 无法拍' });
      return;
    }

    // 学分扣: 先扣后调, 失败退款
    const credits = CREDITS.PREVIEW_PER_SHOT;
    const ok = useTokenStore.getState().spendTokens(credits);
    if (!ok) {
      set({ error: `学分不足, 拍这一镜需要 ${credits} 学分` });
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
      const motionWithStyle = style
        ? `${shot.motion}. 视觉风格: ${style.description}.`
        : shot.motion;
      const systemPrompt = `你是 KidsAI 的视频工具。**只调用一次 image_to_video 工具**,参数:
- motion: ${motionWithStyle}
- image_url: ${character.referenceImageUrl ?? 'https://picsum.photos/seed/' + character.id + '-ref/512/512'}
- image_role: reference_image
- model: ${SEEDANCE_MODEL.PREVIEW}
- seed: ${seed}
- duration: 4
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
    const { character, style, shots, idea } = state;
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
      set({ error: `学分不足, 定稿需要 ${credits} 学分` });
      return;
    }

    set({ isVideoRunning: true, error: null });

    try {
      // 定稿: 1 条完整 prompt(把 3 镜连起来), 2.0 端点, 用 first_frame 不用 reference(更稳定)
      const combinedMotion = shots
        .map((s, i) => `第 ${i + 1} 镜: ${s.motion}`)
        .join(' | ');
      const motionWithStyle = style
        ? `${combinedMotion}. 视觉风格: ${style.description}.`
        : combinedMotion;
      const systemPrompt = `你是 KidsAI 的视频工具。**只调用一次 image_to_video 工具**,参数:
- motion: ${motionWithStyle}
- image_url: ${character.referenceImageUrl ?? 'https://picsum.photos/seed/' + character.id + '-ref/512/512'}
- image_role: first_frame
- model: ${SEEDANCE_MODEL.FINALIZE}
- duration: 5
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
    } catch (e) {
      useTokenStore.getState().addTokens(credits);
      set({ isVideoRunning: false, error: `定稿失败: ${e}` });
    }
  },
}));