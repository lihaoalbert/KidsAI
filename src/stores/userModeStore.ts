// W10 Day 4 — User Mode store (Part C)
//
// 当前 mode + 切换流程:
//   - 启动时 load() 调 get_user_mode
//   - 切换: switchTo(mode, pin) → set_user_mode(mode, pin) → server + 本地
//   - mode 变化触发订阅 (UI 安全过滤 / skill list 重渲染 / etc.)

import { create } from 'zustand';
import {
  getUserMode,
  setUserMode as invokeSetUserMode,
  type UserMode,
} from '../api/tauri';

interface UserModeState {
  mode: UserMode;
  loaded: boolean;
  /// 上次切换时间 (ms), 用于徽章动效
  lastSwitchedAt: number | null;
  /// loading state
  switching: boolean;
  /// 最近一次切换错误
  error: string | null;

  load: () => Promise<void>;
  switchTo: (mode: UserMode, parentPin: string) => Promise<void>;
  clearError: () => void;
}

export const useUserModeStore = create<UserModeState>((set) => ({
  mode: 'child',
  loaded: false,
  lastSwitchedAt: null,
  switching: false,
  error: null,

  load: async () => {
    try {
      const mode = await getUserMode();
      set({ mode, loaded: true });
    } catch (e) {
      // 拿不到 mode (无 license 等) → 保持默认 child, 不阻塞 UI
      console.warn('[userModeStore] load failed:', e);
      set({ loaded: true });
    }
  },

  switchTo: async (mode, parentPin) => {
    set({ switching: true, error: null });
    try {
      const resp = await invokeSetUserMode(mode, parentPin);
      set({
        mode: resp.mode,
        lastSwitchedAt: resp.switchedAt,
        switching: false,
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ switching: false, error: msg });
      throw e;
    }
  },

  clearError: () => set({ error: null }),
}));