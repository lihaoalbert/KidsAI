// studioStore 状态机测试（Task E 对话引擎兜底）
// 覆盖: start / pick choice / pick stuck / pick free → submitFree / reEditSlot / dice /
//       confirmStory 阶段推进 / 所有 assistant slot 写入 directorStore.story
import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';

const runAgentMock: Mock = vi.fn();
const listCharactersMock: Mock = vi.fn();
const listStylesMock: Mock = vi.fn();
const saveCreationMock: Mock = vi.fn();

vi.mock('../api/tauri', async () => {
  const actual = await vi.importActual<typeof import('../api/tauri')>('../api/tauri');
  return {
    ...actual,
    runAgent: (...args: unknown[]) => runAgentMock(...args),
    listCharacters: (...args: unknown[]) => listCharactersMock(...args),
    listStyles: (...args: unknown[]) => listStylesMock(...args),
    saveCreation: (...args: unknown[]) => saveCreationMock(...args),
  };
});

import { useStudioStore } from './studioStore';
import { useDirectorStore } from './directorStore';
import { useTokenStore } from './tokenStore';
import { STAGE_COPY } from '../data/videoScript';
import type { AgentRunResponse, Character, StylePreset } from '../api/tauri';

const xiaoqi: Character = {
  id: 'xiaoqi',
  name: '小启',
  description: '黄发女孩',
  styleTags: ['cartoon'],
  referenceImageUrl: 'https://picsum.photos/seed/xiaoqi-ref/512/512',
};
const cartoon: StylePreset = { id: 'cartoon', name: '卡通', description: '明亮卡通', styleTags: [] };

function makePlanResponse(): AgentRunResponse {
  return {
    sessionId: 's',
    levelId: 'director_plan',
    finalAnswer: JSON.stringify({
      idea: 'x',
      character_id: 'xiaoqi',
      style_id: 'cartoon',
      shots: [
        { description: 'a', motion: '1' },
        { description: 'b', motion: '2' },
        { description: 'c', motion: '3' },
      ],
    }),
    thoughts: [],
    toolCalls: [],
    assets: [],
    durationMs: 100,
  };
}

function getCards(studio: ReturnType<typeof useStudioStore.getState>) {
  const item = [...studio.items].reverse().find((it) => it.kind === 'cards');
  return item?.kind === 'cards' ? item.cards : null;
}

function getStory(studio: ReturnType<typeof useStudioStore.getState>) {
  const item = [...studio.items].reverse().find((it) => it.kind === 'story');
  return item?.kind === 'story';
}

function getItemKind(studio: ReturnType<typeof useStudioStore.getState>, kind: string) {
  return [...studio.items].filter((it) => it.kind === kind);
}

beforeEach(() => {
  runAgentMock.mockReset();
  listCharactersMock.mockReset();
  listStylesMock.mockReset();
  saveCreationMock.mockReset();
  listCharactersMock.mockResolvedValue([xiaoqi]);
  listStylesMock.mockResolvedValue([cartoon]);
  runAgentMock.mockResolvedValue(makePlanResponse());
  useDirectorStore.getState().reset();
  useTokenStore.getState().reset();
  useStudioStore.getState().reset();
});

// ============ start / 初始 beat ============

describe('useStudioStore.start', () => {
  it('重置状态 + 推 Beat 0 的 AI + 5 张卡(3 选+free+stuck)', () => {
    useStudioStore.getState().start();
    const s = useStudioStore.getState();
    expect(s.started).toBe(true);
    expect(s.phase).toBe('stage1');
    expect(s.currentBeatId).toBe('s1_who');
    const cards = getCards(s);
    expect(cards).not.toBeNull();
    expect(cards).toHaveLength(5); // 3 选 + 🎤 + 🤔
    expect(cards!.some((c) => c.label === '会喷火的小恐龙')).toBe(true);
    expect(cards!.some((c) => c.kind === 'free')).toBe(true);
    expect(cards!.some((c) => c.kind === 'stuck')).toBe(true);
    // directorStore story 全部空
    expect(useDirectorStore.getState().story).toEqual({ who: '', wants: '', but: '', ending: '' });
  });
});

// ============ pick(choice) 推进 + slot 写入 ============

describe('pick(choice)', () => {
  it('选 🐉 → story.who 写入 + AI echo + 推进到 s1_wants', () => {
    useStudioStore.getState().start();
    const cards = getCards(useStudioStore.getState())!;
    const dragon = cards.find((c) => c.label === '会喷火的小恐龙')!;
    useStudioStore.getState().pick(dragon);

    const s = useStudioStore.getState();
    expect(useDirectorStore.getState().story.who).toBe('一只会喷火的小恐龙');
    expect(s.currentBeatId).toBe('s1_wants');
    // AI 复述确认
    const echoes = getItemKind(s, 'ai').map((it) => (it as { text: string }).text);
    expect(echoes.some((t) => t.includes('会喷火的小恐龙'))).toBe(true);
    // 旧卡组锁定
    const locked = s.items.find((it) => it.kind === 'cards' && (it as { answered?: string }).answered);
    expect(locked).toBeTruthy();
  });

  it('从 wants → but → ending → 推进 STORY_CARD', () => {
    useStudioStore.getState().start();
    const advance = (label: string, slot: 'who' | 'wants' | 'but' | 'ending') => {
      const cards = getCards(useStudioStore.getState())!;
      const card = cards.find((c) => c.label === label)!;
      useStudioStore.getState().pick(card);
      expect(useDirectorStore.getState().story[slot]).toBeTruthy();
    };
    advance('会喷火的小恐龙', 'who');
    advance('找回丢失的东西', 'wants');
    expect(useStudioStore.getState().currentBeatId).toBe('s1_but');
    advance('被大冰山挡住', 'but');
    expect(useStudioStore.getState().currentBeatId).toBe('s1_ending');
    advance('暖心的', 'ending');
    // 推进到 story card
    expect(useStudioStore.getState().currentBeatId).toBeNull();
    expect(getStory(useStudioStore.getState())).toBe(true);
  });
});

// ============ pick(stuck) → 卡壳兜底 ============

describe('pick(stuck)', () => {
  it('卡壳后 AI 抛 stuck[]（不含 🎤 / 🤔）', () => {
    useStudioStore.getState().start();
    const cards = getCards(useStudioStore.getState())!;
    useStudioStore.getState().pick(cards.find((c) => c.kind === 'stuck')!);
    const after = useStudioStore.getState();
    const newCards = getCards(after)!;
    expect(newCards).toHaveLength(3);
    expect(newCards.every((c) => !c.kind || c.kind === 'choice')).toBe(true);
    // currentBeatId 没变（仍在 s1_who）
    expect(after.currentBeatId).toBe('s1_who');
    // slot 还没写
    expect(useDirectorStore.getState().story.who).toBe('');
  });

  it('从 stuck 选一个 → 当成普通 choice 写入 slot', () => {
    useStudioStore.getState().start();
    useStudioStore.getState().pick(
      getCards(useStudioStore.getState())!.find((c) => c.kind === 'stuck')!,
    );
    // 现在卡片是 stuck 替代组：选一个 unicorn
    const stuckCards = getCards(useStudioStore.getState())!;
    useStudioStore.getState().pick(stuckCards.find((c) => c.label === '独角兽')!);
    expect(useDirectorStore.getState().story.who).toBe('一只闪闪发光的独角兽');
    expect(useStudioStore.getState().currentBeatId).toBe('s1_wants');
  });
});

// ============ pick(free) → submitFree ============

describe('pick(free) + submitFree', () => {
  it('点 🎤 → awaitingFree=true + AI 提示可以打字', () => {
    useStudioStore.getState().start();
    const cards = getCards(useStudioStore.getState())!;
    useStudioStore.getState().pick(cards.find((c) => c.kind === 'free')!);
    expect(useStudioStore.getState().awaitingFree).toBe(true);
    const lastAi = [...useStudioStore.getState().items].reverse().find((it) => it.kind === 'ai');
    expect((lastAi as { text: string }).text).toMatch(/你来说/);
  });

  it('submitFree 写入 slot + AI echo + 推进', () => {
    useStudioStore.getState().start();
    useStudioStore.getState().pick(
      getCards(useStudioStore.getState())!.find((c) => c.kind === 'free')!,
    );
    useStudioStore.getState().submitFree('  一只彩虹色的小狐狸  ');
    expect(useStudioStore.getState().awaitingFree).toBe(false);
    expect(useDirectorStore.getState().story.who).toBe('一只彩虹色的小狐狸');
    expect(useStudioStore.getState().currentBeatId).toBe('s1_wants');
  });

  it('submitFree 空字符串 → 不写入, 不推进', () => {
    useStudioStore.getState().start();
    useStudioStore.getState().pick(
      getCards(useStudioStore.getState())!.find((c) => c.kind === 'free')!,
    );
    const beatBefore = useStudioStore.getState().currentBeatId;
    useStudioStore.getState().submitFree('   ');
    expect(useDirectorStore.getState().story.who).toBe('');
    expect(useStudioStore.getState().currentBeatId).toBe(beatBefore);
  });

  it('不在 awaitingFree 状态 submitFree → noop', () => {
    useStudioStore.getState().start();
    const beatBefore = useStudioStore.getState().currentBeatId;
    useStudioStore.getState().submitFree('一只狗');
    expect(useDirectorStore.getState().story.who).toBe('');
    expect(useStudioStore.getState().currentBeatId).toBe(beatBefore);
  });
});

// ============ dice 一键随机 ============

describe('dice', () => {
  it('写入所有 4 槽 + 推系统消息 + 推故事卡（不需走 Beat）', () => {
    useStudioStore.getState().start(); // 进入 Beat 0
    useStudioStore.getState().dice();
    const story = useDirectorStore.getState().story;
    expect(story.who).toBeTruthy();
    expect(story.wants).toBeTruthy();
    expect(story.but).toBeTruthy();
    expect(story.ending).toBeTruthy();
    const s = useStudioStore.getState();
    expect(s.currentBeatId).toBeNull();
    expect(s.items.some((it) => it.kind === 'system')).toBe(true);
    expect(getStory(s)).toBe(true);
  });
});

// ============ reEditSlot 逐块回改 ============

describe('reEditSlot', () => {
  it('先选 4 槽进入故事卡 → reEditSlot("wants") 回到 wants beat', () => {
    useStudioStore.getState().start();
    const advance = (label: string) => {
      const cards = getCards(useStudioStore.getState())!;
      useStudioStore.getState().pick(cards.find((c) => c.label === label)!);
    };
    advance('会喷火的小恐龙');
    advance('找回丢失的东西');
    advance('被大冰山挡住');
    advance('暖心的');
    expect(getStory(useStudioStore.getState())).toBe(true);

    useStudioStore.getState().reEditSlot('wants');
    expect(useStudioStore.getState().currentBeatId).toBe('s1_wants');
    expect(useStudioStore.getState().returnToStoryAfter).toBe(true);

    // 选个新的 wants → 回到故事卡
    const newCards = getCards(useStudioStore.getState())!;
    useStudioStore.getState().pick(newCards.find((c) => c.label === '交到新朋友')!);
    expect(useDirectorStore.getState().story.wants).toBe('交到新朋友');
    expect(useStudioStore.getState().returnToStoryAfter).toBe(false);
    expect(getStory(useStudioStore.getState())).toBe(true);
  });
});

// ============ confirmStory 阶段推进 ============

describe('confirmStory', () => {
  it('阶段 → plan → 调 runPlanGeneration → 入阶段 character + 出「就这样定它」卡', async () => {
    useStudioStore.getState().start();
    const advance = (label: string) => {
      const cards = getCards(useStudioStore.getState())!;
      useStudioStore.getState().pick(cards.find((c) => c.label === label)!);
    };
    advance('会喷火的小恐龙');
    advance('找回丢失的东西');
    advance('被大冰山挡住');
    advance('暖心的');

    await useStudioStore.getState().confirmStory();

    const s = useStudioStore.getState();
    const d = useDirectorStore.getState();
    // 阶段跳转
    expect(d.stage).toBe(2);
    expect(s.phase).toBe('character');
    expect(d.character?.id).toBe('xiaoqi');
    expect(d.style?.id).toBe('cartoon');
    // 卡组：✅ / 🔄
    const lastCards = getCards(s)!;
    expect(lastCards.some((c) => c.value === '__confirm__')).toBe(true);
    expect(lastCards.some((c) => c.value === '__change__')).toBe(true);
  });

  it('assembledIdea 拼成骨架句（directorStore 已在 directorStore.test.ts 覆盖）', () => {
    useStudioStore.getState().start();
    const advance = (label: string) => {
      const cards = getCards(useStudioStore.getState())!;
      useStudioStore.getState().pick(cards.find((c) => c.label === label)!);
    };
    advance('会喷火的小恐龙');
    advance('找回丢失的东西');
    advance('被大冰山挡住');
    advance('暖心的');
    expect(useDirectorStore.getState().assembledIdea()).toBe(
      '一只会喷火的小恐龙，想要找回丢失的宝贝，但是被一座大冰山挡住了去路，最后交到朋友，一起开心地过关',
    );
  });
});

// ============ reset ============

describe('reset', () => {
  it('清空 items / started=false / phase=stage1 / previewIndex=0', () => {
    useStudioStore.getState().start();
    useStudioStore.getState().reset();
    const s = useStudioStore.getState();
    expect(s.started).toBe(false);
    expect(s.items).toEqual([]);
    expect(s.currentBeatId).toBeNull();
    expect(s.awaitingFree).toBe(false);
    expect(s.returnToStoryAfter).toBe(false);
    expect(s.phase).toBe('stage1');
    expect(s.previewIndex).toBe(0);
  });
});

// ============ STAGE_COPY 文案锚定（保证 LLM 关键文案不被静默改坏） ============

describe('STAGE_COPY anchors', () => {
  it('planReady / character / finalizeDone 关键短语存在', () => {
    expect(STAGE_COPY.planReady).toContain('方案好啦');
    expect(STAGE_COPY.character).toContain('标准照');
    expect(STAGE_COPY.finalizeDone).toContain('完成啦');
  });
});
