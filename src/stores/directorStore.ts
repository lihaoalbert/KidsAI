// v1 视频导演流程状态机（6 阶）
// 设计原则:每一阶孩子拍板才能进下一阶。①-④ 用 1 次 LLM 出 DirectorPlan 填充默认;
// ⑤-⑥ 真实 Seedance 调用,学分扣/退由前端 useTokenStore 兜底。
// 详见 memory/video_director_flow.md

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

interface DirectorState {
  stage: DirectorStage;
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

  // actions
  reset(): void;
  goToStage(s: DirectorStage): void;
  setIdea(text: string): void;
  setStorySlot(slot: StorySlot, value: string): void;
  /** 把四槽拼成喂给 LLM 的骨架句 */
  assembledIdea(): string;
  setCharacter(c: Character): void;
  setCharacterTweak(patch: Partial<CharacterTweak>): void;
  setStyle(s: StylePreset): void;
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

export const useDirectorStore = create<DirectorState>((set, get) => ({
  stage: 1,
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

  reset: () =>
    set({
      stage: 1,
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
    }),

  goToStage: (s) => set({ stage: s }),

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
      listCharacters().catch(() => [] as Character[]),
      listStyles().catch(() => [] as StylePreset[]),
    ]);

    // 尝试 LLM 解析,失败重试 1 次
    let plan: DirectorPlan | null = null;
    for (let attempt = 1; attempt <= 2; attempt++) {
      try {
        const resp = await runAgent({
          levelId: 'director_plan',
          userInput: idea,
          systemPrompt: DIRECTOR_PLAN_SYSTEM_PROMPT,
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

    set({
      character: char,
      style,
      shots,
      stage: 2,
      isLLMRunning: false,
    });
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
