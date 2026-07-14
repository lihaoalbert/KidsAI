// 视频流对话引擎：中屏 transcript + 阶段1 beat 光标 + 阶段推进。
// 对话是脚本驱动、确定性的（不走 LLM 流式）；directorStore 仍是规范数据源，
// 本 store 只负责"把对话演出来"并在关键节点调用 directorStore 的动作。
import { create } from 'zustand';
import {
  DICE_STORIES,
  STAGE1_BEATS,
  STAGE_COPY,
  STORY_CARD_BEAT,
  getStage1Beat,
  type OptionCard,
} from '../data/videoScript';
import {
  CREDITS,
  listCharacters,
  listStyles,
  type Character,
  type StylePreset,
} from '../api/tauri';
import { useDirectorStore, type StorySlot } from './directorStore';
import { generatedAssetUrl } from './assetStore';
import { useTokenStore } from './tokenStore';

export type StudioPhase =
  | 'stage1'
  | 'plan'
  | 'character'
  | 'style'
  | 'storyboard'
  | 'preview'
  | 'finalize'
  | 'done';

export type ChatItem =
  | { id: string; kind: 'ai'; text: string }
  | { id: string; kind: 'kid'; text: string }
  | { id: string; kind: 'system'; text: string; loading?: boolean }
  | {
      id: string;
      kind: 'cards';
      cards: OptionCard[];
      answered?: string; // 已选的 label（选后禁用整组）
    }
  | { id: string; kind: 'story' };

export interface StudioState {
  items: ChatItem[];
  currentBeatId: string | null; // 阶段1 beat 光标
  awaitingFree: boolean; // 🎤「我自己说」— 下一次输入写入当前 beat 的 slot
  returnToStoryAfter: boolean; // 逐块回改：答完这个 beat 回故事卡而非线性下一步
  phase: StudioPhase;
  previewIndex: number; // 阶段5 当前在拍第几镜（0-based）
  started: boolean;

  start: () => void;
  reset: () => void;
  pick: (card: OptionCard) => void;
  submitFree: (text: string) => void;
  confirmStory: () => Promise<void>;
  reEditSlot: (slot: StorySlot) => void;
  /// W5 修复 ③: 回退到阶段 1, 清空对话流, 重放 4 个 beat (保留 directorStore 已选 slot)
  goBackToStep1: () => void;
  dice: () => void;
  previewCurrent: () => Promise<void>;
  finalize: () => Promise<void>;
}

let seq = 0;
function nid(prefix: string) {
  seq += 1;
  return `si_${Date.now()}_${seq}_${prefix}`;
}

const SLOT_TO_BEAT: Record<StorySlot, string> = {
  who: 's1_who',
  wants: 's1_wants',
  but: 's1_but',
  ending: 's1_ending',
};

export const useStudioStore = create<StudioState>((set, get) => {
  // —— transcript 小工具 ——
  const push = (item: ChatItem) => set((s) => ({ items: [...s.items, item] }));
  const pushAi = (text: string) => push({ id: nid('ai'), kind: 'ai', text });
  const pushKid = (text: string) => push({ id: nid('kid'), kind: 'kid', text });
  const pushSystem = (text: string, loading = false) =>
    push({ id: nid('sys'), kind: 'system', text, loading });

  const clearLoading = () =>
    set((s) => ({ items: s.items.filter((it) => !(it.kind === 'system' && it.loading)) }));

  // 禁用最后一组 cards（选完不能再点）
  const lockLastCards = (answeredLabel: string) =>
    set((s) => {
      const items = [...s.items];
      for (let i = items.length - 1; i >= 0; i--) {
        const it = items[i];
        if (it.kind === 'cards' && !it.answered) {
          items[i] = { ...it, answered: answeredLabel };
          break;
        }
      }
      return { items };
    });

  // 渲染阶段1一个问答 beat
  const runBeat = (beatId: string) => {
    const beat = getStage1Beat(beatId);
    if (!beat) return;
    set({ currentBeatId: beatId, awaitingFree: false });
    pushAi(beat.ai);
    const cards: OptionCard[] = [...beat.cards];
    if (beat.allowFree) cards.push({ id: 'free', label: '我自己说', value: '__free__', emoji: '🎤', kind: 'free' });
    cards.push({ id: 'stuck', label: '我也不知道', value: '__stuck__', emoji: '🤔', kind: 'stuck' });
    push({ id: nid('cards'), kind: 'cards', cards });
  };

  const pushStoryCard = () => {
    set({ phase: 'stage1', currentBeatId: null, awaitingFree: false });
    push({ id: nid('story'), kind: 'story' });
  };

  const actionCards = (cards: OptionCard[]) =>
    push({ id: nid('cards'), kind: 'cards', cards });

  // 阶段3 画风：拉候选 → 出卡
  const enterStyle = async () => {
    const d = useDirectorStore.getState();
    // ✓ 拍板: 锁定主角 + 锁定故事核心
    d.lockSubject();
    d.lockStoryCore();
    d.goToStage(3);
    set({ phase: 'style' });
    pushAi(STAGE_COPY.style);
    const styles = await loadStyleCandidates();
    if (styles.length === 0) {
      pushAi('（没连上画风库，先用默认画风～）');
      actionCards([{ id: 'style_ok', label: '就用这个画风', value: '__style_ok__', emoji: '✅', kind: 'action' }]);
      return;
    }
    actionCards(
      styles.slice(0, 4).map((st) => ({
        id: `style_${st.id}`,
        label: st.name,
        value: `style::${st.id}`,
        imageUrl: generatedAssetUrl('style', `${st.id}.full`),
        imageAlt: `${st.name}画风预览`,
        kind: 'action' as const,
      })),
    );
  };

  // 阶段4 分镜
  const enterStoryboard = () => {
    const d = useDirectorStore.getState();
    // ✓ 拍板: 锁定画风
    d.lockArtStyle();
    d.goToStage(4);
    set({ phase: 'storyboard' });
    pushAi(STAGE_COPY.storyboard);
    actionCards([
      { id: 'sb_ok', label: '顺序很好，开拍！', value: '__sb_ok__', emoji: '✅', kind: 'action' },
    ]);
  };

  const offerFinalize = () => {
    const d = useDirectorStore.getState();
    d.goToStage(6);
    set({ phase: 'finalize' });
    const balance = useTokenStore.getState().balance;
    pushAi(STAGE_COPY.finalize(CREDITS.FINALIZE, balance));
    actionCards([
      { id: 'finalize', label: '合成完整电影', value: '__finalize__', emoji: '✨', kind: 'action' },
    ]);
  };

  // 阶段5 试拍：为当前镜出学分确认卡
  const offerPreview = () => {
    const d = useDirectorStore.getState();
    d.goToStage(5);
    set({ phase: 'preview' });
    const idx = get().previewIndex;
    if (idx >= d.shots.length) {
      offerFinalize();
      return;
    }
    const balance = useTokenStore.getState().balance;
    pushAi(STAGE_COPY.preview(idx + 1, CREDITS.PREVIEW_PER_SHOT, balance));
    actionCards([
      { id: 'shoot', label: '开始拍', value: '__shoot__', emoji: '🚀', kind: 'action' },
    ]);
  };

  // 试拍完一镜后：下一镜 or 去定稿
  const offerNextOrFinalize = () => {
    const d = useDirectorStore.getState();
    const idx = get().previewIndex;
    const hasMore = idx + 1 < d.shots.length;
    const cards: OptionCard[] = [];
    if (hasMore) cards.push({ id: 'next_shot', label: '拍下一段', value: '__next_shot__', emoji: '⏭️', kind: 'action' });
    cards.push({ id: 'to_final', label: '就用这些去定稿', value: '__to_final__', emoji: '🎬', kind: 'action' });
    actionCards(cards);
  };

  // 写入 slot + 复述 + 推进（或回故事卡）
  const commitAnswer = (beatId: string, label: string, value: string) => {
    const beat = getStage1Beat(beatId);
    if (!beat) return;
    if (beat.slot) useDirectorStore.getState().setStorySlot(beat.slot, value);
    if (beat.echo) pushAi(beat.echo(label));
    if (get().returnToStoryAfter) {
      set({ returnToStoryAfter: false });
      pushStoryCard();
      return;
    }
    if (beat.next === STORY_CARD_BEAT) pushStoryCard();
    else runBeat(beat.next);
  };

  // 阶段2–6 动作卡分发
  const handleAction = async (card: OptionCard) => {
    const v = card.value;
    const d = useDirectorStore.getState();
    switch (true) {
      case v === '__confirm__':
        pushAi(STAGE_COPY.characterConfirmed(d.character?.name ?? '主角'));
        await enterStyle();
        break;
      case v === '__change__': {
        const chars = await loadCharacterCandidates();
        if (chars.length === 0) {
          pushAi('（暂时没连上角色库，先用当前这个吧～）');
          actionCards([{ id: 'confirm_char2', label: '好，就用它', value: '__confirm__', emoji: '✅', kind: 'action' }]);
          break;
        }
        pushAi('这些主角你喜欢哪个？');
        actionCards(
          chars.slice(0, 6).map((c) => ({
            id: `char_${c.id}`,
            label: c.name,
            value: `char::${c.id}`,
            imageUrl: generatedAssetUrl('character', `${c.id}.stand`),
            imageAlt: `${c.name}标准照`,
            kind: 'action' as const,
          })),
        );
        break;
      }
      case v.startsWith('char::'): {
        const id = v.slice(6);
        const chars = await loadCharacterCandidates();
        const found = chars.find((c) => c.id === id);
        if (found) d.setCharacter(found);
        pushAi(STAGE_COPY.characterConfirmed(found?.name ?? '主角'));
        await enterStyle();
        break;
      }
      case v === '__style_ok__':
        enterStoryboard();
        break;
      case v.startsWith('style::'): {
        const id = v.slice(7);
        const styles = await loadStyleCandidates();
        const found = styles.find((s) => s.id === id);
        if (found) d.setStyle(found);
        pushAi(STAGE_COPY.styleConfirmed(found?.name ?? '默认'));
        enterStoryboard();
        break;
      }
      case v === '__sb_ok__':
        pushAi(STAGE_COPY.storyboardConfirmed);
        offerPreview();
        break;
      case v === '__shoot__':
        await get().previewCurrent();
        offerNextOrFinalize();
        break;
      case v === '__next_shot__':
        set((s) => ({ previewIndex: s.previewIndex + 1 }));
        offerPreview();
        break;
      case v === '__to_final__':
        offerFinalize();
        break;
      case v === '__finalize__':
        await get().finalize();
        break;
      default:
        break;
    }
  };

  return {
    items: [],
    currentBeatId: null,
    awaitingFree: false,
    returnToStoryAfter: false,
    phase: 'stage1',
    previewIndex: 0,
    started: false,

    start: () => {
      set({
        items: [],
        currentBeatId: null,
        awaitingFree: false,
        returnToStoryAfter: false,
        phase: 'stage1',
        previewIndex: 0,
        started: true,
      });
      useDirectorStore.getState().reset();
      runBeat(STAGE1_BEATS[0].id);
    },

    reset: () =>
      set({
        items: [],
        currentBeatId: null,
        awaitingFree: false,
        returnToStoryAfter: false,
        phase: 'stage1',
        previewIndex: 0,
        started: false,
      }),

    pick: (card) => {
      const kind = card.kind ?? 'choice';
      if (kind === 'action') {
        lockLastCards(`${card.emoji ?? ''}${card.label}`);
        if (card.value !== '__confirm__' && card.value !== '__change__') pushKid(card.label);
        void handleAction(card);
        return;
      }
      if (kind === 'free') {
        lockLastCards(`${card.emoji ?? ''}${card.label}`);
        pushAi('好呀，你来说！在下面打字或说给我听～🎤');
        set({ awaitingFree: true });
        return;
      }
      if (kind === 'stuck') {
        lockLastCards(`${card.emoji ?? ''}${card.label}`);
        const beat = getStage1Beat(get().currentBeatId ?? '');
        pushAi('没关系！我抛几个，你挑一个喜欢的？😊');
        const stuck = beat?.stuck ?? [];
        push({ id: nid('cards'), kind: 'cards', cards: [...stuck] });
        return;
      }
      if (kind === 'dice') {
        lockLastCards(`${card.emoji ?? ''}${card.label}`);
        get().dice();
        return;
      }
      // choice
      lockLastCards(`${card.emoji ?? ''}${card.label}`);
      pushKid(card.label);
      const beatId = get().currentBeatId;
      if (beatId) commitAnswer(beatId, card.label, card.value);
    },

    submitFree: (text) => {
      const trimmed = text.trim();
      if (!trimmed) return;
      pushKid(trimmed);
      const beatId = get().currentBeatId;
      if (get().awaitingFree && beatId) {
        set({ awaitingFree: false });
        commitAnswer(beatId, trimmed, trimmed);
      }
    },

    reEditSlot: (slot) => {
      set({ returnToStoryAfter: true });
      const beatId = SLOT_TO_BEAT[slot];
      pushAi('好，我们改改这一块～');
      runBeat(beatId);
    },

    goBackToStep1: () => {
      // W5 修复 ③: 顶部胶囊点回"点子", 重置对话流, 重放 4 个 beat.
      // directorStore 已还原 stage 1 的 story, 这里不重置 (让用户直接看到之前选的角色 + 可改).
      set({
        items: [],
        currentBeatId: null,
        awaitingFree: false,
        returnToStoryAfter: false,
        phase: 'stage1',
        previewIndex: 0,
        started: true,
      });
      pushAi('欢迎回来～ 我们来重新想一想这个故事吧 ✨');
      runBeat(STAGE1_BEATS[0].id);
    },

    dice: () => {
      const s = DICE_STORIES[Math.floor(Math.random() * DICE_STORIES.length)];
      const d = useDirectorStore.getState();
      (Object.keys(s) as StorySlot[]).forEach((k) => d.setStorySlot(k, s[k]));
      pushSystem('🎲 我随手抛了一个故事…');
      pushStoryCard();
    },

    confirmStory: async () => {
      pushKid('就这样开始！✅');
      set({ phase: 'plan' });
      const d = useDirectorStore.getState();
      pushSystem(STAGE_COPY.planLoading, true);
      await d.runPlanGeneration(d.assembledIdea());
      clearLoading();
      const after = useDirectorStore.getState();
      if (after.error && after.shots.length === 0) {
        pushAi(`哎呀，出了点小状况：${after.error}`);
        return;
      }
      pushAi(STAGE_COPY.planReady);
      // 进阶段2 主角 (runPlanGeneration 内部已经 goToStage(2) 把阶段1 入 history)
      set({ phase: 'character' });
      pushAi(STAGE_COPY.character);
      if (!after.character) pushAi('（没连上角色库，先用默认主角，之后也能换～）');
      actionCards([
        { id: 'confirm_char', label: '就这样定它', value: '__confirm__', emoji: '✅', kind: 'action' },
        { id: 'change_char', label: '换一个主角', value: '__change__', emoji: '🔄', kind: 'action' },
      ]);
    },

    previewCurrent: async () => {
      const d = useDirectorStore.getState();
      const idx = get().previewIndex;
      const shot = d.shots[idx];
      if (!shot) return;
      pushSystem('🎬 正在拍这一段…', true);
      await d.runPreviewShot(shot.id);
      clearLoading();
      const after = useDirectorStore.getState();
      if (after.error) {
        pushAi(`没拍成：${after.error}`);
        return;
      }
      pushAi(STAGE_COPY.previewDone);
    },

    finalize: async () => {
      const d = useDirectorStore.getState();
      pushSystem('✨ 正在把所有片段合成一部电影…', true);
      const title = d.story.who ? `${d.story.who}的冒险` : '我的小电影';
      await d.runFinalize(title);
      clearLoading();
      const after = useDirectorStore.getState();
      if (after.error) {
        pushAi(`定稿失败：${after.error}`);
        return;
      }
      set({ phase: 'done' });
      pushAi(STAGE_COPY.finalizeDone);
    },
  };
});

// 供阶段2/3 动态候选卡使用（读后端；失败/无数据返回空）
export async function loadCharacterCandidates(): Promise<Character[]> {
  try {
    const r = await listCharacters();
    return r ?? [];
  } catch {
    return [];
  }
}
export async function loadStyleCandidates(): Promise<StylePreset[]> {
  try {
    const r = await listStyles();
    return r ?? [];
  } catch {
    return [];
  }
}

export { CREDITS };
