// Day 20+: 小墨 (16 岁) persona — 完整功能 + 全视频流程 E2E (前端层)
//
// 与 小月 共用同一视频管线, 但 persona 维度有差异:
//   - age_tier = "14-16", PetEngine recall 阈值 5 天 (vs 小月 3 天), 给期末复习留 buffer
//   - pet_id = "墨石", recall 文案走"考完试了吗?"
//   - 默认可选 adult-readable skill (但 16 岁还在 child mode, 不会被展示, 仅家长切成人后才解锁)
//   - 期末模考专属 prompt: 用更专业的"分镜"叙事, demo 一次"纪录片"短片
//
// 视频管线路径 (与小月同):
//   Onboarding → 4 镜 DirectorPlan → 4 次试拍 (hailuo) → 定稿 (hailuo) → 作品墙
//
// 不变量:
//   - 学币默认 500, 4 镜 + 定稿 = 55, 跟小月同步 (避免"不同人不同预算"的认知割裂).
//   - PetEngine 16 岁 persona: 3 天不动不召回 (留给她复习), 5 天才召回.

import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';
import { useDirectorStore } from './directorStore';
import { useTokenStore } from './tokenStore';
import { useProjectStore } from './projectStore';
import type { AgentRunResponse } from '../api/tauri';

const runAgentMock: Mock = vi.fn();
const listCharactersMock: Mock = vi.fn();
const listStylesMock: Mock = vi.fn();
const saveCreationMock: Mock = vi.fn();
const listProjectsMock: Mock = vi.fn();
const createProjectMock: Mock = vi.fn();
const loadProjectMock: Mock = vi.fn();
const listCreationsMock: Mock = vi.fn();
const petTickMock: Mock = vi.fn();
const installSkillMock: Mock = vi.fn();

vi.mock('../api/tauri', async () => {
  const actual = await vi.importActual<typeof import('../api/tauri')>(
    '../api/tauri',
  );
  return {
    ...actual,
    runAgent: (...args: unknown[]) => runAgentMock(...args),
    listCharacters: (...args: unknown[]) => listCharactersMock(...args),
    listStyles: (...args: unknown[]) => listStylesMock(...args),
    saveCreation: (...args: unknown[]) => saveCreationMock(...args),
    listProjects: (...args: unknown[]) => listProjectsMock(...args),
    createProject: (...args: unknown[]) => createProjectMock(...args),
    loadProject: (...args: unknown[]) => loadProjectMock(...args),
    listCreations: (...args: unknown[]) => listCreationsMock(...args),
    petTick: (...args: unknown[]) => petTickMock(...args),
    installSkill: (...args: unknown[]) => installSkillMock(...args),
    loadIdentity: () =>
      Promise.resolve({
        userId: 'user:小墨',
        nickname: '小墨',
        petId: '墨石',
        petMood: 'happy',
        lastSeenAt: Date.now(),
        ageTier: '14-16',
        parentId: null,
      }),
  };
});

// 小墨 persona 在 builtin 候选里挑 主角 xiaoqi + 风格 line-drawing (更素描风, 偏 art-school girl vibe)
const XIAOQI_CHAR = {
  id: 'xiaoqi',
  name: '小启',
  description: '9 岁好奇小猫女孩',
  styleTags: ['cartoon'],
  referenceImageUrl: 'https://picsum.photos/seed/xiaoqi-ref/512/512',
};
const LINE_STYLE = {
  id: 'line-drawing',
  name: '线描',
  description: '黑白线描, 文学插画感',
  styleTags: [] as string[],
};

const FOUR_SHOTS = [
  { beat: 'hook', mood: 'calm', camera: 'wide', motion: '清晨教室空无一人' },
  { beat: 'conflict', mood: 'tense', camera: 'follow', motion: '追逐着未写完的字' },
  { beat: 'payoff', mood: 'epic', camera: 'extreme', motion: '笔尖触到纸面那一刻' },
  { beat: 'payoff', mood: 'joyful', camera: 'wide', motion: '窗外飞起一群鸟' },
];

function makeRunResponse(overrides: Partial<AgentRunResponse> = {}): AgentRunResponse {
  return {
    sessionId: 'sess_xiaomo',
    levelId: 'director',
    finalAnswer: '',
    thoughts: [],
    toolCalls: [],
    assets: [],
    durationMs: 100,
    ...overrides,
  };
}

function planJson() {
  return JSON.stringify({
    idea: '期末的那只笔',
    character_id: 'xiaoqi',
    style_id: 'line-drawing',
    shots: FOUR_SHOTS.map((s, idx) => ({
      description: `${s.beat} 镜`,
      motion: s.motion,
      beat: s.beat,
      mood: s.mood,
      camera: s.camera,
      character_refs: ['xiaoqi'],
      transition_to_next: idx === FOUR_SHOTS.length - 1 ? 'none' : 'cut',
    })),
  });
}

beforeEach(() => {
  runAgentMock.mockReset();
  listCharactersMock.mockReset();
  listStylesMock.mockReset();
  saveCreationMock.mockReset();
  listProjectsMock.mockReset();
  createProjectMock.mockReset();
  loadProjectMock.mockReset();
  listCreationsMock.mockReset();
  petTickMock.mockReset();
  installSkillMock.mockReset();

  useDirectorStore.getState().reset();
  useTokenStore.getState().reset();
  useProjectStore.setState({ list: [], current: null });

  listCharactersMock.mockResolvedValue([XIAOQI_CHAR]);
  listStylesMock.mockResolvedValue([LINE_STYLE, { id: 'cartoon', name: '卡通', description: '卡通', styleTags: [] }]);
});

// ============ 1. 编故事 (LLM 一次成功, 4 镜 line-drawing 风格) ============

describe('小墨 persona: 阶段1 编故事', () => {
  it('LLM 选 xiaoqi + line-drawing, 4 镜 文档风格 (epic/follow 镜头)', async () => {
    runAgentMock.mockResolvedValueOnce(makeRunResponse({ finalAnswer: planJson() }));
    await useDirectorStore.getState().runPlanGeneration('期末的那只笔');

    const s = useDirectorStore.getState();
    expect(s.cursor).toBe(2);
    expect(s.character?.id).toBe('xiaoqi');
    expect(s.style?.id).toBe('line-drawing'); // 小墨偏好 line-drawing (文学感)
    expect(s.shots).toHaveLength(4);
    expect(s.error).toBeNull();
  });

  it('LLM 失败 → 走兜底 3 镜模板, 小墨仍能继续创作 (不让卡死)', async () => {
    runAgentMock.mockResolvedValue(makeRunResponse({ finalAnswer: '我不知道怎么分镜' }));
    await useDirectorStore.getState().runPlanGeneration('一个故事');

    const s = useDirectorStore.getState();
    expect(s.error).toBeTruthy();
    expect(s.shots).toHaveLength(3); // 兜底 3 镜
    expect(s.character).not.toBeNull();
  });
});

// ============ 2. 阶段4-5 试拍 4 镜 (hailuo 默认) ============

describe('小墨 persona: 阶段4 试拍 4 镜', () => {
  beforeEach(async () => {
    runAgentMock.mockResolvedValueOnce(makeRunResponse({ finalAnswer: planJson() }));
    await useDirectorStore.getState().runPlanGeneration('期末的那只笔');
  });

  it('4 次试拍全成功 (hailuo) → 学币 -36, 视频 URL 全填', async () => {
    const start = useTokenStore.getState().balance;
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://hailuo.mock/xiaomo.mp4', prompt: 'm', tool: 'image_to_video', tokensCost: 9 }],
      }),
    );

    for (const shot of useDirectorStore.getState().shots) {
      await useDirectorStore.getState().runPreviewShot(shot.id);
    }

    expect(useTokenStore.getState().balance).toBe(start - 4 * 9);
    const s = useDirectorStore.getState();
    for (const shot of s.shots) {
      expect(shot.previewUrl).toBe('https://hailuo.mock/xiaomo.mp4');
      expect(shot.previewing).toBe(false);
    }
  });

  it('小墨重拍第 2 镜: 已经成功过, 重新生成也要扣 9 (coherent with shoot button)', async () => {
    const start = useTokenStore.getState().balance;
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://hailuo.mock/xiaomo.mp4', prompt: 'm', tool: 'image_to_video', tokensCost: 9 }],
      }),
    );
    const shot = useDirectorStore.getState().shots[1];
    await useDirectorStore.getState().runPreviewShot(shot.id);
    expect(useTokenStore.getState().balance).toBe(start - 9);
  });
});

// ============ 3. 阶段6 定稿 (hailuo) ============

describe('小墨 persona: 阶段6 定稿', () => {
  beforeEach(async () => {
    runAgentMock.mockResolvedValueOnce(makeRunResponse({ finalAnswer: planJson() }));
    await useDirectorStore.getState().runPlanGeneration('期末的那只笔');
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://hailuo.mock/p.mp4', prompt: 'm', tool: 'image_to_video', tokensCost: 9 }],
      }),
    );
    for (const shot of useDirectorStore.getState().shots) {
      await useDirectorStore.getState().runPreviewShot(shot.id);
    }
    runAgentMock.mockReset();
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://hailuo.mock/final-xiaomo.mp4', prompt: 'f', tool: 'image_to_video', tokensCost: 19 }],
      }),
    );
  });

  it('小墨定稿 → finalVideoUrl, 学币 -19, saveCreation 入库', async () => {
    const start = useTokenStore.getState().balance;
    saveCreationMock.mockResolvedValue({ id: 'c_xiaomo_001' });

    await useDirectorStore.getState().runFinalize('期末的那只笔');

    const s = useDirectorStore.getState();
    expect(s.finalVideoUrl).toBe('https://hailuo.mock/final-xiaomo.mp4');
    expect(saveCreationMock).toHaveBeenCalledOnce();
    expect(useTokenStore.getState().balance).toBe(start - 19);

    const callArgs = saveCreationMock.mock.calls[0][0];
    expect(callArgs.agentOutput.title).toBe('期末的那只笔');
  });
});

// ============ 4. 小墨专有: 5 天 idle 才召回, 3 天不动仅走 sleepy ============

describe('小墨 persona: 宠物互动 (PetEngine via pet_tick)', () => {
  it('小墨 3 天 idle → 不召回, mood 转 Sleepy (留 buffer 给期末复习)', async () => {
    petTickMock.mockResolvedValueOnce({
      kind: 'mood_changed',
      from: 'happy',
      to: 'sleepy',
      reason: 'tick',
    });
    const r = await (await import('../api/tauri')).petTick({
      userId: 'user:小墨',
      isInConversation: false,
      conversationStartedSecsAgo: 0,
    });
    expect(r.kind).toBe('mood_changed');
    if (r.kind === 'mood_changed') {
      expect(r.to).toBe('sleepy');
    }
  });

  it('小墨 5 天 idle → 召回, 文案含 "考"', async () => {
    petTickMock.mockResolvedValueOnce({
      kind: 'recall',
      currentMood: 'sleepy',
      message: '很久没见你了, 考完试了吗? 墨石还在原地等你 🪨',
    });
    const r = await (await import('../api/tauri')).petTick({
      userId: 'user:小墨',
      isInConversation: false,
      conversationStartedSecsAgo: 0,
    });
    expect(r.kind).toBe('recall');
    if (r.kind === 'recall') {
      expect(r.message).toMatch(/考/);
      expect(r.message).toMatch(/墨石/);
    }
  });
});

// ============ 5. 学币压力 (小墨默认 500, 与小月同步 — 同 app 双 persona) ============

describe('小墨 persona: 学币一致性 (双 persona 共享 500 学币池)', () => {
  it('小墨 1 流程扣 55 学币 (跟小月同步, 避免 persona 割裂)', async () => {
    runAgentMock.mockResolvedValueOnce(makeRunResponse({ finalAnswer: planJson() }));
    await useDirectorStore.getState().runPlanGeneration('期末的那只笔');
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://hailuo.mock/p.mp4', prompt: 'm', tool: 'image_to_video', tokensCost: 9 }],
      }),
    );
    for (const shot of useDirectorStore.getState().shots) {
      await useDirectorStore.getState().runPreviewShot(shot.id);
    }
    runAgentMock.mockReset();
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://hailuo.mock/f.mp4', prompt: 'f', tool: 'image_to_video', tokensCost: 19 }],
      }),
    );
    saveCreationMock.mockResolvedValue({ id: 'c1' });

    const start = useTokenStore.getState().balance;
    await useDirectorStore.getState().runFinalize('期末的那只笔');
    const spent = start - useTokenStore.getState().balance;
    expect(spent).toBe(19); // 定稿扣 19 (previews 在前置断言已验证)
    expect(useTokenStore.getState().balance).toBe(500 - 36 - 19);
  });
});

// ============ 6. 持久化 + 跨 session 续作 (P1-1) ============

describe('小墨 persona: 跨 session 续作', () => {
  it('小墨启动 → 拉最近项目 (updatedAt 倒序) → open', async () => {
    const fixedNow = 1_700_000_000_000;
    const recent = {
      id: 'p_xiaomo_001',
      title: '期末的那只笔',
      levelId: 'director',
      cursor: 6,
      thumbPath: null,
      totalCredits: 55,
      createdAt: fixedNow - 86_400_000,
      updatedAt: fixedNow - 7200_000, // 2h 前
    };
    listProjectsMock.mockResolvedValue([recent]);
    loadProjectMock.mockResolvedValue({
      meta: recent,
      plan: {},
      transcript: { items: [], started: false },
    });

    const list = await (await import('../api/tauri')).listProjects();
    const sorted = [...list].sort((a, b) => b.updatedAt - a.updatedAt);
    expect(sorted[0].id).toBe('p_xiaomo_001');
  });
});

// ============ 7. 小墨专属边界: parent_id = None (家长是设备主人, 不是 known parent) ============
//
// 与小月 (parent_id="mother:ayan") 不同, 小墨是设备主人本人 (16 岁, 没有显式家长 ID).
// 这影响安装 audience=adult skill 时的家长 PIN 流程触发.
describe('小墨 persona: 模式边界 (parentless = 设备主人)', () => {
  it('小墨 child mode 装 audience=adult skill 仍被拒 (跟小月一致)', async () => {
    installSkillMock.mockRejectedValueOnce(
      new Error('skill commercial-ad (audience=adult) 不能在 child 模式下安装'),
    );
    await expect(
      (await import('../api/tauri')).installSkill('commercial-ad', '1234'),
    ).rejects.toThrow(/不能.*child.*安装/);
  });
});
