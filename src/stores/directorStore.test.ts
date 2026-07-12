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
describe('parseDirectorPlan', () => {
  it('接受严格 JSON', () => {
    const raw = JSON.stringify({
      idea: '小猫追蝴蝶',
      character_id: 'xiaoqi',
      style_id: 'cartoon',
      shots: [
        { description: '开头', motion: 'a' },
        { description: '中间', motion: 'b' },
        { description: '结尾', motion: 'c' },
      ],
    });
    const r = parseDirectorPlan(raw);
    expect(r.ok).toBe(true);
    expect(r.plan?.shots).toHaveLength(3);
    expect(r.plan?.character_id).toBe('xiaoqi');
  });

  it('从 markdown fence 抽取', () => {
    const raw = '我帮你想好了：\n```json\n' +
      JSON.stringify({
        idea: 'x',
        character_id: 'xiaoyue',
        style_id: 'anime',
        shots: [
          { description: 'a', motion: '1' },
          { description: 'b', motion: '2' },
          { description: 'c', motion: '3' },
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

// ============ directorStore 状态机测试 ============

const runAgentMock: Mock = vi.fn();
const listCharactersMock: Mock = vi.fn();
const listStylesMock: Mock = vi.fn();
const saveCreationMock: Mock = vi.fn();

vi.mock('../api/tauri', async () => {
  // 透传真实 parseDirectorPlan 供 test 用
  const actual = await vi.importActual<typeof import('../api/tauri')>('../api/tauri');
  return {
    ...actual,
    runAgent: (...args: unknown[]) => runAgentMock(...args),
    listCharacters: (...args: unknown[]) => listCharactersMock(...args),
    listStyles: (...args: unknown[]) => listStylesMock(...args),
    saveCreation: (...args: unknown[]) => saveCreationMock(...args),
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
          shots: [
            { description: '开头', motion: 'a' },
            { description: '中间', motion: 'b' },
            { description: '结尾', motion: 'c' },
          ],
        }),
      }),
    );
    await useDirectorStore.getState().runPlanGeneration('小猫追蝴蝶');
    const s = useDirectorStore.getState();
    expect(s.error).toBeNull();
    expect(s.stage).toBe(2);
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
        { id: 's1', description: 'a', motion: 'm', previewUrl: null, seed: 1, previewing: false },
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
        { id: 's1', description: 'a', motion: 'm', previewUrl: null, seed: 1, previewing: false },
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
        { id: 's1', description: 'a', motion: 'm', previewUrl: null, seed: 1, previewing: false },
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
        { id: 's1', description: '1', motion: '', previewUrl: null, seed: 1, previewing: false },
        { id: 's2', description: '2', motion: '', previewUrl: null, seed: 1, previewing: false },
        { id: 's3', description: '3', motion: '', previewUrl: null, seed: 1, previewing: false },
      ],
    });
    useDirectorStore.getState().moveShot('s2', 'up');
    const order = useDirectorStore.getState().shots.map((s) => s.id);
    expect(order).toEqual(['s2', 's1', 's3']);
  });

  it('向下移动到末尾', () => {
    useDirectorStore.setState({
      shots: [
        { id: 's1', description: '1', motion: '', previewUrl: null, seed: 1, previewing: false },
        { id: 's2', description: '2', motion: '', previewUrl: null, seed: 1, previewing: false },
        { id: 's3', description: '3', motion: '', previewUrl: null, seed: 1, previewing: false },
      ],
    });
    useDirectorStore.getState().moveShot('s2', 'down');
    const order = useDirectorStore.getState().shots.map((s) => s.id);
    expect(order).toEqual(['s1', 's3', 's2']);
  });

  it('最上向上不动', () => {
    useDirectorStore.setState({
      shots: [
        { id: 's1', description: '1', motion: '', previewUrl: null, seed: 1, previewing: false },
        { id: 's2', description: '2', motion: '', previewUrl: null, seed: 1, previewing: false },
      ],
    });
    useDirectorStore.getState().moveShot('s1', 'up');
    const order = useDirectorStore.getState().shots.map((s) => s.id);
    expect(order).toEqual(['s1', 's2']);
  });

  it('最下向下不动', () => {
    useDirectorStore.setState({
      shots: [
        { id: 's1', description: '1', motion: '', previewUrl: null, seed: 1, previewing: false },
        { id: 's2', description: '2', motion: '', previewUrl: null, seed: 1, previewing: false },
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
        { id: 's1', description: 'a', motion: 'm1', previewUrl: null, seed: 1, previewing: false },
        { id: 's2', description: 'b', motion: 'm2', previewUrl: null, seed: 2, previewing: false },
        { id: 's3', description: 'c', motion: 'm3', previewUrl: null, seed: 3, previewing: false },
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
        { id: 's1', description: 'a', motion: 'm', previewUrl: null, seed: 1, previewing: false },
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
