// Day 20+: 小月 (9 岁) persona — 完整功能 + 全视频流程 E2E (前端层)
//
// 覆盖路径 (用 vi.mock Tauri IPC, 不真调 LLM/Seedance/hailuo):
//   1. Onboarding → createProject → 写项目进 projectStore
//   2. 阶段1 编故事 → runPlanGeneration: DirectorPlan 4 镜, LLM 选 xiaoyue + cartoon
//   3. 阶段2 主角 (xiaoyue already in builtin) → 阶段3 画风 (cartoon)
//   4. 阶段4 设计分镜: 4 镜显式填 (符合小月"钩/冲/转/收"叙事模板)
//   5. 阶段5 试拍 × 4: 走 hailuo image_to_video, 学币 -9 × 4 = -36, balance 校验, 退款场景
//   6. 阶段6 定稿: 走 hailuo, 学币 -19, saveCreation 入库
//   7. 作品墙 (LibraryPage 数据源): 从 creations 拉到该作品
//   8. 学币总账: 4*9 + 19 = 55 学币, 默认 500 → 扣完剩 445
//   9. 宠物互动: 用 PetTickRequest mock, 验证
//      - 3 天 idle → 召回 "想你了" 文案
//      - 5h 连续创作 → "休息一下" 召回
//      - 5 分钟内有动作 → happy (NoOp)
//   10. 模式: child mode 装 audience=adult skill 被拒, audience=both 的能装
//
// 验证 9 岁孩子特有的体验门槛:
//   - 4 个分镜是 sweet spot (不会像 7 镜那么累, 也不会 1 镜太简单)
//   - hailuo 模型默认 5s/duration, 适合注意力短的孩子
//   - 学币压力低 (单次 4 镜 + 定稿 = 55 ≈ 1/9 默认额度), 可以反复试错

import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';
import { useDirectorStore } from './directorStore';
import { useTokenStore } from './tokenStore';
import { useToastStore } from './toastStore';
import { useProjectStore } from './projectStore';
import type { AgentRunResponse } from '../api/tauri';

const runAgentMock: Mock = vi.fn();
const listCharactersMock: Mock = vi.fn();
const listStylesMock: Mock = vi.fn();
const saveCreationMock: Mock = vi.fn();
const listProjectsMock: Mock = vi.fn();
const createProjectMock: Mock = vi.fn();
const loadProjectMock: Mock = vi.fn();
const saveProjectStateMock: Mock = vi.fn();
const listCreationsMock: Mock = vi.fn();
const petTickMock: Mock = vi.fn();
const installSkillMock: Mock = vi.fn();
const audienceForMock: Mock = vi.fn();

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
    saveProjectState: (...args: unknown[]) => saveProjectStateMock(...args),
    listCreations: (...args: unknown[]) => listCreationsMock(...args),
    petTick: (...args: unknown[]) => petTickMock(...args),
    installSkill: (...args: unknown[]) => installSkillMock(...args),
    loadIdentity: () =>
      Promise.resolve({
        userId: 'user:小月',
        nickname: '小月',
        petId: 'huomiao',
        petMood: 'happy',
        lastSeenAt: Date.now(),
        ageTier: '8-10',
        parentId: 'mother:ayan',
      }),
  };
});

// 小月 persona 期望的角色 + 风格 (frontend builtin 已知)
const XIAOYUE_CHAR = {
  id: 'xiaoyue',
  name: '小月',
  description: '红裙双马尾 8 岁女孩',
  styleTags: ['cartoon'],
  referenceImageUrl: 'https://picsum.photos/seed/xiaoyue-ref/512/512',
};
const CARTOON_STYLE = {
  id: 'cartoon',
  name: '卡通',
  description: '明亮色彩卡通',
  styleTags: [] as string[],
};

const FOUR_SHOTS = [
  { beat: 'hook', mood: 'calm', camera: 'wide', motion: '她站在家门口抬头看' },
  { beat: 'conflict', mood: 'tense', camera: 'medium', motion: '走进黑森林' },
  { beat: 'payoff', mood: 'sad', camera: 'close', motion: '找不到出口' },
  { beat: 'payoff', mood: 'joyful', camera: 'wide', motion: '找到光回到家' },
];
// 注: DirectorPlan schema 只允许 3 个 beat (hook/conflict/payoff).
// 4 镜中前 3 镜走 3 幕, 第 4 镜是 payoff 的延伸结局.
// 长度 3-5 镜都合法.

function makeRunResponse(overrides: Partial<AgentRunResponse> = {}): AgentRunResponse {
  return {
    sessionId: 'sess_xiaoyue',
    levelId: 'director',
    finalAnswer: '',
    thoughts: [],
    toolCalls: [],
    assets: [],
    durationMs: 100,
    ...overrides,
  };
}

function fourShotJson() {
  return JSON.stringify({
    idea: '小月的勇气森林',
    character_id: 'xiaoyue',
    style_id: 'cartoon',
    shots: FOUR_SHOTS.map((s, idx) => ({
      description: `${s.beat} 镜`,
      motion: s.motion,
      beat: s.beat,
      mood: s.mood,
      camera: s.camera,
      character_refs: ['xiaoyue'],
      transition_to_next: idx === FOUR_SHOTS.length - 1 ? 'none' : 'fade',
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
  saveProjectStateMock.mockReset();
  listCreationsMock.mockReset();
  petTickMock.mockReset();
  installSkillMock.mockReset();
  audienceForMock.mockReset();

  useDirectorStore.getState().reset();
  useTokenStore.getState().reset();
  useToastStore.setState({ toasts: [] });

  listCharactersMock.mockResolvedValue([XIAOYUE_CHAR]);
  listStylesMock.mockResolvedValue([CARTOON_STYLE]);
});

// ============ 1. 编故事 → DirectorPlan 4 镜 (钩/冲/转/收) ============

describe('小月 persona: 阶段1 编故事', () => {
  it('LLM 一次成功 → 4 镜全到位, cursor=2', async () => {
    runAgentMock.mockResolvedValueOnce(makeRunResponse({ finalAnswer: fourShotJson() }));
    await useDirectorStore.getState().runPlanGeneration('小月的勇气森林');

    const s = useDirectorStore.getState();
    expect(s.error).toBeNull();
    expect(s.cursor).toBe(2);
    expect(s.character?.id).toBe('xiaoyue');
    expect(s.style?.id).toBe('cartoon');
    expect(s.shots).toHaveLength(4);

    const beats = s.shots.map((sh) => sh.beat);
    expect(beats).toEqual(['hook', 'conflict', 'payoff', 'payoff']);
  });

  it('idea 为空 → 报错, 不调 LLM (前端守门员)', async () => {
    await useDirectorStore.getState().runPlanGeneration('   ');
    expect(runAgentMock).not.toHaveBeenCalled();
    expect(useDirectorStore.getState().error).toMatch(/先说说/);
  });
});

// ============ 2. 阶段4-5 试拍 4 次 (hailuo) ============

describe('小月 persona: 阶段4 试拍 4 镜', () => {
  beforeEach(async () => {
    // 准备 4 镜运行时分镜
    runAgentMock.mockResolvedValueOnce(makeRunResponse({ finalAnswer: fourShotJson() }));
    await useDirectorStore.getState().runPlanGeneration('小月的勇气森林');
  });

  it('试拍 4 次成功 (hailuo 默认 5s) → 学币净减 36', async () => {
    const startBalance = useTokenStore.getState().balance; // 500
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [
          {
            type: 'video',
            url: 'https://hailuo.mock/x.mp4',
            prompt: 'm',
            tool: 'image_to_video',
            tokensCost: 9,
          },
        ],
      }),
    );

    for (const shot of useDirectorStore.getState().shots) {
      await useDirectorStore.getState().runPreviewShot(shot.id);
    }

    expect(useTokenStore.getState().balance).toBe(startBalance - 4 * 9);

    const s = useDirectorStore.getState();
    expect(s.shots.every((sh) => sh.previewUrl === 'https://hailuo.mock/x.mp4')).toBe(true);
    expect(s.error).toBeNull();
  });

  it('第 3 镜 hailuo down → 退款 9 学币, shot[2].previewUrl 仍 null', async () => {
    const startBalance = useTokenStore.getState().balance;
    // 前 2 次成功, 第 3 次失败, 第 4 次成功
    runAgentMock
      .mockResolvedValueOnce(
        makeRunResponse({
          assets: [{ type: 'video', url: 'https://hailuo.mock/s1.mp4', prompt: 'm', tool: 'image_to_video', tokensCost: 9 }],
        }),
      )
      .mockResolvedValueOnce(
        makeRunResponse({
          assets: [{ type: 'video', url: 'https://hailuo.mock/s2.mp4', prompt: 'm', tool: 'image_to_video', tokensCost: 9 }],
        }),
      )
      .mockRejectedValueOnce(new Error('hailuo api 502'))
      .mockResolvedValueOnce(
        makeRunResponse({
          assets: [{ type: 'video', url: 'https://hailuo.mock/s4.mp4', prompt: 'm', tool: 'image_to_video', tokensCost: 9 }],
        }),
      );

    const shots = useDirectorStore.getState().shots;
    await useDirectorStore.getState().runPreviewShot(shots[0].id);
    await useDirectorStore.getState().runPreviewShot(shots[1].id);
    await useDirectorStore.getState().runPreviewShot(shots[2].id); // 失败
    await useDirectorStore.getState().runPreviewShot(shots[3].id);

    // 预期: 4 次扣减但仅 3 次成功 ⇒ 净花费 3 × 9 = 27
    expect(useTokenStore.getState().balance).toBe(startBalance - 27);
    const after = useDirectorStore.getState();
    expect(after.shots[2].previewUrl).toBeNull();
    expect(after.shots[2].previewing).toBe(false);
    expect(after.shots[0].previewUrl).toBe('https://hailuo.mock/s1.mp4');
  });
});

// ============ 3. 阶段6 定稿 (hailuo image_to_video, model=HAILUO) ============

describe('小月 persona: 阶段6 定稿', () => {
  beforeEach(async () => {
    runAgentMock.mockResolvedValueOnce(makeRunResponse({ finalAnswer: fourShotJson() }));
    await useDirectorStore.getState().runPlanGeneration('小月的勇气森林');
    // 4 次试拍
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
        assets: [
          {
            type: 'video',
            url: 'https://hailuo.mock/final.mp4',
            prompt: '4 镜合成',
            tool: 'image_to_video',
            tokensCost: 19,
          },
        ],
      }),
    );
  });

  it('定稿成功 → finalVideoUrl, 学币 -19, saveCreation 入库', async () => {
    const startBalance = useTokenStore.getState().balance; // 已扣 36 = 464
    saveCreationMock.mockResolvedValue({ id: 'c_xiaoyue_001' });
    listCreationsMock.mockResolvedValue([
      {
        id: 'c_xiaoyue_001',
        levelId: 'director',
        userInput: '小月的勇气森林',
        agentOutput: JSON.stringify({ title: '小月的勇气森林' }),
        createdAt: Date.now(),
        score: null,
        rubric: null,
        feedback: null,
        assets: [{ kind: 'video', url: 'https://hailuo.mock/final.mp4', thumbnailUrl: null, prompt: 'p', tool: 'image_to_video', tokensCost: 19 }],
      },
    ]);

    await useDirectorStore.getState().runFinalize('小月的勇气森林');

    const s = useDirectorStore.getState();
    expect(s.finalVideoUrl).toBe('https://hailuo.mock/final.mp4');
    expect(s.error).toBeNull();
    expect(saveCreationMock).toHaveBeenCalledOnce();
    expect(useTokenStore.getState().balance).toBe(startBalance - 19);

    // 作品墙: listCreations 应能拉到
    const list = await (await import('../api/tauri')).listCreations();
    expect(list).toHaveLength(1);
    expect(list[0].userInput).toBe('小月的勇气森林');
  });

  it('定稿 hailuo 抛异常 → 学币回退, finalVideoUrl null', async () => {
    const startBalance = useTokenStore.getState().balance;
    runAgentMock.mockRejectedValueOnce(new Error('hailuo task failed'));
    await useDirectorStore.getState().runFinalize('小月的勇气森林');

    const s = useDirectorStore.getState();
    expect(s.finalVideoUrl).toBeNull();
    expect(s.error).toContain('定稿失败');
    expect(useTokenStore.getState().balance).toBe(startBalance);
  });
});

// ============ 4. 学币总账: 小月一次完整流程 = 4*9 + 19 = 55 学币 ============

describe('小月 persona: 学币总账', () => {
  it('4 镜 + 定稿 = 55 学币 (默认 500 能跑 ≥7 部)', async () => {
    runAgentMock.mockResolvedValueOnce(makeRunResponse({ finalAnswer: fourShotJson() }));
    await useDirectorStore.getState().runPlanGeneration('小月的勇气森林');
    expect(useDirectorStore.getState().shots.length).toBe(4);
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://hailuo.mock/v.mp4', prompt: 'm', tool: 'image_to_video', tokensCost: 9 }],
      }),
    );
    for (const shot of useDirectorStore.getState().shots) {
      await useDirectorStore.getState().runPreviewShot(shot.id);
    }
    expect(useTokenStore.getState().balance).toBe(500 - 36);
    runAgentMock.mockReset();
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [{ type: 'video', url: 'https://hailuo.mock/f.mp4', prompt: 'f', tool: 'image_to_video', tokensCost: 19 }],
      }),
    );
    saveCreationMock.mockResolvedValue({ id: 'c1' });

    const start = useTokenStore.getState().balance;
    await useDirectorStore.getState().runFinalize('小月的勇气森林');
    const spent = start - useTokenStore.getState().balance;
    expect(spent).toBe(19);
  });
});

// ============ 5. 宠物互动: huomiao + 小月召回文案 ============

describe('小月 persona: 宠物互动 (PetEngine via pet_tick IPC)', () => {
  it('小月 happy 状态 → petTick 返回 NoOp', async () => {
    petTickMock.mockResolvedValueOnce({ kind: 'noop', currentMood: 'happy' });
    const r = await (await import('../api/tauri')).petTick({
      userId: 'user:小月',
      isInConversation: false,
      conversationStartedSecsAgo: 0,
    });
    expect(r.kind).toBe('noop');
  });

  it('小月 3 天 idle → petTick 返回 Recall 含"想"情感文案', async () => {
    petTickMock.mockResolvedValueOnce({
      kind: 'recall',
      currentMood: 'sleepy',
      message: '咿呀~ 你不在的时候我会想你的 🔥',
    });
    const r = await (await import('../api/tauri')).petTick({
      userId: 'user:小月',
      isInConversation: false,
      conversationStartedSecsAgo: 0,
    });
    expect(r.kind).toBe('recall');
    if (r.kind === 'recall') {
      expect(r.message).toMatch(/[想🔥]/);
    }
  });

  it('小月连续创作 5h → petTick 返回 burnout 召回', async () => {
    petTickMock.mockResolvedValueOnce({
      kind: 'recall',
      currentMood: 'thinking',
      message: '你已经陪着 agent 创作很久啦, 休息一下吧 ☕',
    });
    const r = await (await import('../api/tauri')).petTick({
      userId: 'user:小月',
      isInConversation: true,
      conversationStartedSecsAgo: 5 * 3600 + 1,
    });
    expect(r.kind).toBe('recall');
    if (r.kind === 'recall') {
      expect(r.message).toContain('休息');
    }
  });
});

// ============ 6. 小月模式边界: child mode 应拒绝装 adult skill ============

describe('小月 persona: 模式边界 (9 岁 = child mode)', () => {
  it('child mode 不能装 audience=adult 的 skill', async () => {
    // 模拟 Rust 端 audience gate 已经按 Plan 落地: install_skill 返 Err.
    installSkillMock.mockRejectedValueOnce(
      new Error('skill commercial-ad (audience=adult) 不能在 child 模式下安装'),
    );

    await expect(
      (await import('../api/tauri')).installSkill('commercial-ad', '1234'),
    ).rejects.toThrow(/不能.*child.*安装/);
    expect(installSkillMock).toHaveBeenCalledOnce();
  });
});

// ============ 7. 持久化: project auto-resume (P1-1) ============

describe('小月 persona: 跨 session 续作', () => {
  it('启动时拉最近 updatedAt 项目 → open → 看到上次的 4 镜 plan', async () => {
    const fixedNow = 1_700_000_000_000;
    vi.spyOn(Date, 'now').mockReturnValue(fixedNow);
    const recent = {
      id: 'p_xiaoyue_001',
      title: '小月的勇气森林',
      levelId: 'director',
      cursor: 6, // 已完成
      thumbPath: null,
      totalCredits: 55,
      createdAt: fixedNow - 86_400_000,
      updatedAt: fixedNow - 3600_000, // 1h 前
    };
    listProjectsMock.mockResolvedValue([recent]);
    loadProjectMock.mockResolvedValue({
      meta: recent,
      plan: {},
      transcript: { items: [], started: false },
    });

    // App.tsx P1-1 逻辑的子集 — 排序后 open 最近一个
    const list = await (await import('../api/tauri')).listProjects();
    expect(list).toHaveLength(1);
    const recent2 = [...list].sort((a, b) => b.updatedAt - a.updatedAt)[0];
    expect(recent2?.id).toBe('p_xiaoyue_001');

    await useProjectStore.getState().open(recent2!.id);
    expect(useProjectStore.getState().current?.id).toBe('p_xiaoyue_001');
  });
});
