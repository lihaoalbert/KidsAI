// directorStore 状态机测试（v1 视频导演流程）
// 覆盖:
//   - parseDirectorPlan: 严格 JSON / markdown fence / 无效 → 3 种路径
//   - runPlanGeneration: LLM 成功 → 填 ②③④, stage 跳 2; LLM 失败重试 1 次 → 用兜底
//   - runPreviewShot: 余额 0 → 报错; 成功 → 余额 -9, previewUrl 写入; 失败 → 退款
//   - runFinalize: 成功 → 余额 -19, finalVideoUrl 写入, saveCreation 被调
//   - moveShot: 边界(已在最上/最下不动)

import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';
import { useDirectorStore } from './directorStore';
import { useTokenStore } from './tokenStore';
import type { AgentRunResponse, Character, StylePreset } from '../api/tauri';
import { parseDirectorPlan } from '../api/tauri';

// ============ parseDirectorPlan 单测(纯函数, 不需要 mock) ============
// W4.6 #4: shots 必须带 6 字段 (beat/mood/camera/character_refs/transition_to_next).
// 老的只 description/motion 的 plan 形状 → 严格 schema 拒绝 (用 tauri.test.ts case 1 覆盖).
function shot(beat: string, transition: string, overrides: Partial<{mood: string; camera: string}> = {}) {
  return {
    description: 'd',
    motion: 'm',
    beat,
    mood: overrides.mood ?? 'joyful',
    camera: overrides.camera ?? 'wide',
    character_refs: ['xiaoqi'],
    transition_to_next: transition,
  };
}

describe('parseDirectorPlan', () => {
  it('接受严格 JSON (W4.6 #4: 6 字段 + 3 镜)', () => {
    const raw = JSON.stringify({
      idea: '小猫追蝴蝶',
      character_id: 'xiaoqi',
      style_id: 'cartoon',
      shots: [
        shot('hook', 'fade'),
        shot('conflict', 'cut'),
        shot('payoff', 'none'),
      ],
    });
    const r = parseDirectorPlan(raw);
    expect(r.ok).toBe(true);
    expect(r.plan?.shots).toHaveLength(3);
    expect(r.plan?.character_id).toBe('xiaoqi');
  });

  it('从 markdown fence 抽取 (W4.6 #4: 6 字段)', () => {
    const raw = '我帮你想好了：\n```json\n' +
      JSON.stringify({
        idea: 'x',
        character_id: 'xiaoyue',
        style_id: 'anime',
        shots: [
          shot('hook', 'fade'),
          shot('conflict', 'cut'),
          shot('payoff', 'none'),
        ],
      }) +
      '\n```\n好了';
    const r = parseDirectorPlan(raw);
    expect(r.ok).toBe(true);
    expect(r.plan?.character_id).toBe('xiaoyue');
  });

  it('shots 数量不是 3 → 失败', () => {
    const raw = JSON.stringify({
      idea: 'x',
      character_id: 'xiaoqi',
      style_id: 'cartoon',
      shots: [{ description: 'a', motion: '1' }],
    });
    const r = parseDirectorPlan(raw);
    expect(r.ok).toBe(false);
    expect(r.error).toBeDefined();
  });

  it('缺字段 → 失败', () => {
    const r = parseDirectorPlan(JSON.stringify({ idea: 'x' }));
    expect(r.ok).toBe(false);
  });

  it('完全不是 JSON → 失败', () => {
    const r = parseDirectorPlan('hello world');
    expect(r.ok).toBe(false);
  });

  it('空字符串 → 失败', () => {
    const r = parseDirectorPlan('');
    expect(r.ok).toBe(false);
  });
});

const EMPTY_STORY = { who: '', wants: '', but: '', ending: '' };

// ============ directorStore 状态机测试 ============

const runAgentMock: Mock = vi.fn();
const listCharactersMock: Mock = vi.fn();
const listStylesMock: Mock = vi.fn();
const saveCreationMock: Mock = vi.fn();
const downloadAssetMock: Mock = vi.fn();

vi.mock('../api/tauri', async () => {
  // 透传真实 parseDirectorPlan 供 test 用
  const actual = await vi.importActual<typeof import('../api/tauri')>('../api/tauri');
  return {
    ...actual,
    runAgent: (...args: unknown[]) => runAgentMock(...args),
    listCharacters: (...args: unknown[]) => listCharactersMock(...args),
    listStyles: (...args: unknown[]) => listStylesMock(...args),
    saveCreation: (...args: unknown[]) => saveCreationMock(...args),
    downloadAsset: (...args: unknown[]) => downloadAssetMock(...args),
  };
});

const xiaoqi: Character = {
  id: 'xiaoqi',
  name: '小启',
  description: '黄发女孩',
  styleTags: ['cartoon'],
  referenceImageUrl: 'https://picsum.photos/seed/xiaoqi-ref/512/512',
};
const xiaoyue: Character = {
  id: 'xiaoyue',
  name: '小月',
  description: '红裙女孩',
  styleTags: ['cartoon'],
  referenceImageUrl: 'https://picsum.photos/seed/xiaoyue-ref/512/512',
};
const cartoon: StylePreset = { id: 'cartoon', name: '卡通', description: '明亮卡通', styleTags: [] };
const anime: StylePreset = { id: 'anime', name: '动漫', description: '日系动漫', styleTags: [] };

function makeRunResponse(overrides: Partial<AgentRunResponse> = {}): AgentRunResponse {
  return {
    sessionId: 'sess_test',
    levelId: 'director',
    finalAnswer: '',
    thoughts: [],
    toolCalls: [],
    assets: [],
    durationMs: 100,
    ...overrides,
  };
}

beforeEach(() => {
  runAgentMock.mockReset();
  listCharactersMock.mockReset();
  listStylesMock.mockReset();
  saveCreationMock.mockReset();
  useDirectorStore.getState().reset();
  useTokenStore.getState().reset();
});

describe('directorStore.runPlanGeneration', () => {
  it('空 idea → 报错且不调 LLM', async () => {
    await useDirectorStore.getState().runPlanGeneration('   ');
    expect(runAgentMock).not.toHaveBeenCalled();
    expect(useDirectorStore.getState().error).toMatch(/先说说/);
  });

  it('LLM 一次成功 → 填 ②③④, stage=2', async () => {
    listCharactersMock.mockResolvedValue([xiaoqi, xiaoyue]);
    listStylesMock.mockResolvedValue([cartoon, anime]);
    runAgentMock.mockResolvedValueOnce(
      makeRunResponse({
        finalAnswer: JSON.stringify({
          idea: '小猫追蝴蝶',
          character_id: 'xiaoyue',
          style_id: 'anime',
          // W4.6 #4: 6 字段 + 3 镜 (hook → conflict → payoff)
          shots: [
            { description: '开头', motion: 'a', beat: 'hook', mood: 'joyful', camera: 'wide', character_refs: ['xiaoyue'], transition_to_next: 'fade' },
            { description: '中间', motion: 'b', beat: 'conflict', mood: 'tense', camera: 'medium', character_refs: ['xiaoyue'], transition_to_next: 'cut' },
            { description: '结尾', motion: 'c', beat: 'payoff', mood: 'epic', camera: 'overhead', character_refs: ['xiaoyue'], transition_to_next: 'none' },
          ],
        }),
      }),
    );
    await useDirectorStore.getState().runPlanGeneration('小猫追蝴蝶');
    const s = useDirectorStore.getState();
    expect(s.error).toBeNull();
    expect(s.cursor).toBe(2);
    expect(s.character?.id).toBe('xiaoyue');
    expect(s.style?.id).toBe('anime');
    expect(s.shots).toHaveLength(3);
    expect(s.shots[0].description).toBe('开头');
  });

  it('LLM 失败 1 次 + 重试失败 → 用兜底', async () => {
    listCharactersMock.mockResolvedValue([xiaoqi]);
    listStylesMock.mockResolvedValue([cartoon]);
    // 两次都返非 JSON
    runAgentMock.mockResolvedValue(makeRunResponse({ finalAnswer: '不知道' }));
    await useDirectorStore.getState().runPlanGeneration('x');
    const s = useDirectorStore.getState();
    // 兜底 plan: xiaoqi + cartoon + 3 镜模板
    expect(s.character?.id).toBe('xiaoqi');
    expect(s.style?.id).toBe('cartoon');
    expect(s.shots).toHaveLength(3);
    expect(s.error).toBeTruthy();
  });

  it('LLM 抛异常 → 兜底', async () => {
    listCharactersMock.mockResolvedValue([xiaoqi]);
    listStylesMock.mockResolvedValue([cartoon]);
    runAgentMock.mockRejectedValue(new Error('network'));
    await useDirectorStore.getState().runPlanGeneration('x');
    const s = useDirectorStore.getState();
    expect(s.character?.id).toBe('xiaoqi');
    expect(s.shots).toHaveLength(3);
  });
});

describe('directorStore.runPreviewShot', () => {
  it('余额 0 → 报错, 不调 LLM', async () => {
    useTokenStore.getState().reset(); // 余额 500, 正常; 改成 0
    useTokenStore.setState({ balance: 0 });
    // 先填好 character + shots
    useDirectorStore.setState({
      character: xiaoqi,
      style: cartoon,
      shots: [
        { id: 's1', description: 'a', motion: 'm', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
      ],
    });
    await useDirectorStore.getState().runPreviewShot('s1');
    expect(runAgentMock).not.toHaveBeenCalled();
    expect(useDirectorStore.getState().error).toMatch(/学分不足/);
  });

  it('成功 → 余额 -9, previewUrl 写入', async () => {
    const before = useTokenStore.getState().balance;
    useDirectorStore.setState({
      character: xiaoqi,
      style: cartoon,
      shots: [
        { id: 's1', description: 'a', motion: 'm', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
      ],
    });
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://e/v.mp4', prompt: 'm', tool: 'image_to_video', tokensCost: 9 }],
      }),
    );
    await useDirectorStore.getState().runPreviewShot('s1');
    const s = useDirectorStore.getState();
    expect(s.shots[0].previewUrl).toBe('https://e/v.mp4');
    expect(s.shots[0].previewing).toBe(false);
    expect(s.error).toBeNull();
    expect(useTokenStore.getState().balance).toBe(before - 9);
  });

  it('LLM 抛异常 → 余额回退 + shot.previewing=false', async () => {
    const before = useTokenStore.getState().balance;
    useDirectorStore.setState({
      character: xiaoqi,
      style: cartoon,
      shots: [
        { id: 's1', description: 'a', motion: 'm', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
      ],
    });
    runAgentMock.mockRejectedValue(new Error('seedance down'));
    await useDirectorStore.getState().runPreviewShot('s1');
    const s = useDirectorStore.getState();
    expect(s.shots[0].previewUrl).toBeNull();
    expect(s.shots[0].previewing).toBe(false);
    expect(s.error).toMatch(/试拍失败/);
    expect(useTokenStore.getState().balance).toBe(before); // 退款
  });
});

describe('directorStore.moveShot', () => {
  it('向上移动中间项', () => {
    useDirectorStore.setState({
      shots: [
        { id: 's1', description: '1', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
        { id: 's2', description: '2', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
        { id: 's3', description: '3', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
      ],
    });
    useDirectorStore.getState().moveShot('s2', 'up');
    const order = useDirectorStore.getState().shots.map((s) => s.id);
    expect(order).toEqual(['s2', 's1', 's3']);
  });

  it('向下移动到末尾', () => {
    useDirectorStore.setState({
      shots: [
        { id: 's1', description: '1', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
        { id: 's2', description: '2', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
        { id: 's3', description: '3', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
      ],
    });
    useDirectorStore.getState().moveShot('s2', 'down');
    const order = useDirectorStore.getState().shots.map((s) => s.id);
    expect(order).toEqual(['s1', 's3', 's2']);
  });

  it('最上向上不动', () => {
    useDirectorStore.setState({
      shots: [
        { id: 's1', description: '1', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
        { id: 's2', description: '2', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
      ],
    });
    useDirectorStore.getState().moveShot('s1', 'up');
    const order = useDirectorStore.getState().shots.map((s) => s.id);
    expect(order).toEqual(['s1', 's2']);
  });

  it('最下向下不动', () => {
    useDirectorStore.setState({
      shots: [
        { id: 's1', description: '1', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
        { id: 's2', description: '2', motion: '', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
      ],
    });
    useDirectorStore.getState().moveShot('s2', 'down');
    const order = useDirectorStore.getState().shots.map((s) => s.id);
    expect(order).toEqual(['s1', 's2']);
  });
});

describe('directorStore.runFinalize', () => {
  it('成功 → 余额 -19, finalVideoUrl 写入, saveCreation 被调', async () => {
    const before = useTokenStore.getState().balance;
    useDirectorStore.setState({
      character: xiaoqi,
      style: cartoon,
      idea: '小猫追蝴蝶',
      shots: [
        { id: 's1', description: 'a', motion: 'm1', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
        { id: 's2', description: 'b', motion: 'm2', previewUrl: null, seed: 2, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
        { id: 's3', description: 'c', motion: 'm3', previewUrl: null, seed: 3, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
      ],
    });
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://e/final.mp4', prompt: '...', tool: 'image_to_video', tokensCost: 19 }],
      }),
    );
    saveCreationMock.mockResolvedValue({ id: 'c1' });
    await useDirectorStore.getState().runFinalize('我的小猫');
    const s = useDirectorStore.getState();
    expect(s.finalVideoUrl).toBe('https://e/final.mp4');
    expect(s.error).toBeNull();
    expect(saveCreationMock).toHaveBeenCalledOnce();
    expect(useTokenStore.getState().balance).toBe(before - 19);
  });

  it('LLM 抛异常 → 余额回退, finalVideoUrl 仍 null', async () => {
    const before = useTokenStore.getState().balance;
    useDirectorStore.setState({
      character: xiaoqi,
      style: cartoon,
      shots: [
        { id: 's1', description: 'a', motion: 'm', previewUrl: null, seed: 1, previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none" },
      ],
    });
    runAgentMock.mockRejectedValue(new Error('seedance down'));
    await useDirectorStore.getState().runFinalize('');
    const s = useDirectorStore.getState();
    expect(s.finalVideoUrl).toBeNull();
    expect(s.error).toMatch(/定稿失败/);
    expect(useTokenStore.getState().balance).toBe(before);
  });
});

// ============ W5 修复: cursor + history + locked_props + goBackTo ============

describe('directorStore.cursor + history', () => {
  it('runPlanGeneration 把阶段1 决策入 history, cursor 跳 2', async () => {
    listCharactersMock.mockResolvedValue([xiaoqi]);
    listStylesMock.mockResolvedValue([cartoon]);
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        finalAnswer: JSON.stringify({
          idea: '小猫追蝴蝶',
          character_id: 'xiaoqi',
          style_id: 'cartoon',
          shots: [
            { description: 'a', motion: 'm1' },
            { description: 'b', motion: 'm2' },
            { description: 'c', motion: 'm3' },
          ],
        }),
      }),
    );
    useDirectorStore.setState({
      idea: '小猫追蝴蝶',
      story: { who: '小猫', wants: '追蝴蝶', but: '迷路了', ending: '找到了' },
    });
    await useDirectorStore.getState().runPlanGeneration('小猫追蝴蝶');
    const s = useDirectorStore.getState();
    expect(s.cursor).toBe(2);
    expect(s.history).toHaveLength(1);
    expect(s.history[0].stage).toBe(1);
    expect(s.history[0].snapshot.idea).toBe('小猫追蝴蝶');
    expect(s.history[0].snapshot.story.who).toBe('小猫');
    expect(s.history[0].stale).toBe(false);
  });

  it('goToStage 推进 → history 累积, 后续阶不立刻标 stale', () => {
    useDirectorStore.setState({
      cursor: 2,
      history: [
        {
          stage: 1,
          snapshot: {
            idea: 'x', story: { who: 'w', wants: '', but: '', ending: '' },
            character: null, characterTweak: {}, style: null, shots: [],
            locked_props: {},
          },
          decided_at: 1, stale: false,
        },
      ],
    });
    useDirectorStore.getState().goToStage(3);
    const s = useDirectorStore.getState();
    expect(s.cursor).toBe(3);
    expect(s.history).toHaveLength(2);
    expect(s.history[1].stage).toBe(2);
  });

  it('goBackTo 还原快照 + 标下游 stale', () => {
    useDirectorStore.setState({
      cursor: 4,
      history: [
        {
          stage: 1,
          snapshot: {
            idea: 'idea_1', story: { who: 'w1', wants: '', but: '', ending: '' },
            character: null, characterTweak: {}, style: null, shots: [],
            locked_props: { story_core: '老故事' },
          },
          decided_at: 1, stale: false,
        },
        {
          stage: 2,
          snapshot: {
            idea: 'idea_1', story: { who: 'w1', wants: '', but: '', ending: '' },
            character: xiaoqi, characterTweak: {}, style: null, shots: [],
            locked_props: { story_core: '老故事', subject: '老主角' },
          },
          decided_at: 2, stale: false,
        },
        {
          stage: 3,
          snapshot: {
            idea: 'idea_1', story: { who: 'w1', wants: '', but: '', ending: '' },
            character: xiaoqi, characterTweak: {}, style: cartoon, shots: [],
            locked_props: { story_core: '老故事', subject: '老主角', art_style: '老画风' },
          },
          decided_at: 3, stale: false,
        },
      ],
    });
    const ok = useDirectorStore.getState().goBackTo(2);
    expect(ok).toBe(true);
    const s = useDirectorStore.getState();
    expect(s.cursor).toBe(2);
    expect(s.character?.id).toBe('xiaoqi');
    expect(s.style).toBeNull();
    expect(s.locked_props.story_core).toBe('老故事');
    expect(s.locked_props.subject).toBe('老主角');
    expect(s.locked_props.art_style).toBeUndefined();
    // 下游 (stage 3) 标 stale
    expect(s.history[2].stale).toBe(true);
  });

  it('goBackTo 未决策的阶 → 返回 false', () => {
    useDirectorStore.setState({ cursor: 2, history: [] });
    const ok = useDirectorStore.getState().goBackTo(1);
    expect(ok).toBe(false);
  });

  it('reset 清空 cursor + history + locked_props', () => {
    useDirectorStore.setState({
      cursor: 5,
      history: [
        { stage: 1, snapshot: { idea: 'x', story: EMPTY_STORY, character: null, characterTweak: {}, style: null, shots: [], locked_props: {} }, decided_at: 1, stale: false },
      ],
      locked_props: { subject: 'x' },
    });
    useDirectorStore.getState().reset();
    const s = useDirectorStore.getState();
    expect(s.cursor).toBe(1);
    expect(s.history).toHaveLength(0);
    expect(s.locked_props).toEqual({ subject: undefined, story_core: undefined, art_style: undefined });
  });
});

describe('directorStore.locked_props', () => {
  it('lockSubject 写入主角描述 + 微调', () => {
    useDirectorStore.setState({
      character: xiaoqi,
      characterTweak: { color: 'yellow', size: 'M', expression: 'smile' },
    });
    useDirectorStore.getState().lockSubject();
    expect(useDirectorStore.getState().locked_props.subject).toContain('小启');
    expect(useDirectorStore.getState().locked_props.subject).toContain('主色yellow');
    expect(useDirectorStore.getState().locked_props.subject).toContain('体型M');
    expect(useDirectorStore.getState().locked_props.subject).toContain('表情smile');
  });

  it('lockStoryCore 从 assembledIdea() 拼', () => {
    useDirectorStore.setState({
      story: { who: '小猫', wants: '追蝴蝶', but: '迷路了', ending: '找到了' },
    });
    useDirectorStore.getState().lockStoryCore();
    expect(useDirectorStore.getState().locked_props.story_core).toBe(
      '小猫，想要追蝴蝶，但是迷路了，最后找到了',
    );
  });

  it('lockArtStyle 写入 style.name + description', () => {
    useDirectorStore.setState({ style: cartoon });
    useDirectorStore.getState().lockArtStyle();
    expect(useDirectorStore.getState().locked_props.art_style).toBe('卡通, 明亮卡通');
  });

  it('runPlanGeneration 注入 locked_props 到 system_prompt (含硬约束)', async () => {
    listCharactersMock.mockResolvedValue([xiaoqi]);
    listStylesMock.mockResolvedValue([cartoon]);
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        finalAnswer: JSON.stringify({
          idea: 'x', character_id: 'xiaoqi', style_id: 'cartoon',
          shots: [
            { description: 'a', motion: 'm1' },
            { description: 'b', motion: 'm2' },
            { description: 'c', motion: 'm3' },
          ],
        }),
      }),
    );
    useDirectorStore.setState({
      locked_props: {
        subject: '主角是小启(黄发女孩)',
        story_core: '小启想要追蝴蝶但是迷路了最后找到了',
        art_style: '明亮卡通',
      },
    });
    await useDirectorStore.getState().runPlanGeneration('x');
    const call = runAgentMock.mock.calls[0][0] as { systemPrompt: string };
    expect(call.systemPrompt).toContain('已锁定的主角');
    expect(call.systemPrompt).toContain('主角是小启');
    expect(call.systemPrompt).toContain('已锁定的故事核心');
    expect(call.systemPrompt).toContain('已锁定的画风');
    expect(call.systemPrompt).toContain('硬约束');
    expect(call.systemPrompt).toContain('必须服务于已锁定的故事核心');
  });
});

// ============ W8 M1-D: 资产本地下载入队 ============
describe('W8 M1-D: downloadAsset 触发点', () => {
  it('setCharacter 触发 standardImageUrl 入队 (有 project_id)', async () => {
    const { useProjectStore } = await import('./projectStore');
    useProjectStore.setState({
      current: {
        id: 'p-active',
        title: 't',
        levelId: null,
        cursor: 0,
        thumbPath: null,
        totalCredits: 0,
        createdAt: 0,
        updatedAt: 0,
      },
    });
    downloadAssetMock.mockClear();
    useDirectorStore.getState().setCharacter({
      id: 'x',
      name: 'x',
      description: 'd',
      styleTags: [],
      standardImageUrl: 'https://cdn/x.png',
    });
    // 让 microtask 跑完
    await new Promise((r) => setTimeout(r, 0));
    expect(downloadAssetMock).toHaveBeenCalledWith(
      'p-active',
      'https://cdn/x.png',
      'image',
      'character/x-stand.png',
    );
  });

  it('setCharacter 无 project_id 时静默跳过 (不抛错)', async () => {
    const { useProjectStore } = await import('./projectStore');
    useProjectStore.setState({ current: null });
    downloadAssetMock.mockClear();
    useDirectorStore.getState().setCharacter({
      id: 'y',
      name: 'y',
      description: 'd',
      styleTags: [],
      standardImageUrl: 'https://cdn/y.png',
    });
    await new Promise((r) => setTimeout(r, 0));
    expect(downloadAssetMock).not.toHaveBeenCalled();
  });
});
