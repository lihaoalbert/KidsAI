// 关卡状态：进度、解锁状态、当前正在玩的关卡
// W2.2: 内存状态 + Tauri 命令
// W2.3: 启动时从 SQLite 加载

import { create } from 'zustand';
import type {
  Level,
  LevelProgress,
  ScoringCriteria,
} from '../../shared/types/level';
import {
  completedLevelIds as tauriCompletedLevelIds,
  listLevels as tauriListLevels,
  listProgress as tauriListProgress,
  startLevel as tauriStartLevel,
  submitLevel as tauriSubmitLevel,
} from '../api/tauri';

interface LevelState {
  // 数据
  levels: Level[];
  progress: LevelProgress[];
  completedIds: Set<string>;

  // UI 状态
  isLoading: boolean;
  error: string | null;

  // 当前正在进行的关卡（详情页 / runner 用）
  activeLevelId: string | null;

  // actions
  refresh: () => Promise<void>;
  startLevel: (id: string) => Promise<void>;
  submitLevel: (
    id: string,
    score: number,
    rubric: ScoringCriteria,
    feedback: string,
  ) => Promise<void>;
  setActiveLevel: (id: string | null) => void;

  // 派生
  isUnlocked: (id: string) => boolean;
  getProgress: (id: string) => LevelProgress | undefined;
}

export const useLevelStore = create<LevelState>((set, get) => ({
  levels: [],
  progress: [],
  completedIds: new Set(),
  isLoading: false,
  error: null,
  activeLevelId: null,

  refresh: async () => {
    set({ isLoading: true, error: null });
    try {
      const [levels, progress, completedIds] = await Promise.all([
        tauriListLevels(),
        tauriListProgress(),
        tauriCompletedLevelIds(),
      ]);
      set({
        levels,
        progress,
        completedIds: new Set(completedIds),
        isLoading: false,
      });
    } catch (e) {
      set({ error: String(e), isLoading: false });
    }
  },

  startLevel: async (id) => {
    set({ error: null });
    try {
      const updated = await tauriStartLevel(id);
      set((s) => {
        const others = s.progress.filter((p) => p.levelId !== id);
        return { progress: [...others, updated], activeLevelId: id };
      });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  submitLevel: async (id, score, rubric, feedback) => {
    set({ error: null });
    try {
      const updated = await tauriSubmitLevel(id, score, rubric, feedback);
      set((s) => {
        const others = s.progress.filter((p) => p.levelId !== id);
        const next = new Set(s.completedIds);
        if (updated.status === 'completed') next.add(id);
        return {
          progress: [...others, updated],
          completedIds: next,
        };
      });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  setActiveLevel: (id) => set({ activeLevelId: id }),

  isUnlocked: (id) => {
    const { levels, completedIds } = get();
    const level = levels.find((l) => l.id === id);
    if (!level) return false;
    if (level.prerequisites.length === 0) return true;
    return level.prerequisites.every((p) => completedIds.has(p));
  },

  getProgress: (id) => {
    return get().progress.find((p) => p.levelId === id);
  },
}));
