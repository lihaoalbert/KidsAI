// W10 Day 4 — Skill store
//
// 客户端缓存: 已装 skill 列表 + 可用 (server) 列表.
// install / uninstall / toggle 通过 IPC 走 Rust → MarketplaceClient → server.

import { create } from 'zustand';
import {
  installSkill,
  listAvailableSkills,
  listInstalledSkills,
  toggleSkill,
  uninstallSkill,
  type InstallReceipt,
  type MarketplaceSkill,
  type SkillSummary,
} from '../api/tauri';

interface SkillState {
  installed: SkillSummary[];
  available: MarketplaceSkill[];
  loadingInstalled: boolean;
  loadingAvailable: boolean;
  error: string | null;
  /// 正在 install/uninstall/toggle 的 skill_id (UI disable + spinner)
  busy: string | null;

  refreshInstalled: () => Promise<void>;
  refreshAvailable: () => Promise<void>;
  refreshAll: () => Promise<void>;
  install: (skillId: string, parentPin: string) => Promise<InstallReceipt>;
  uninstall: (skillId: string) => Promise<void>;
  toggle: (skillId: string, enabled: boolean) => Promise<void>;
  clearError: () => void;
}

export const useSkillStore = create<SkillState>((set, get) => ({
  installed: [],
  available: [],
  loadingInstalled: false,
  loadingAvailable: false,
  error: null,
  busy: null,

  refreshInstalled: async () => {
    set({ loadingInstalled: true });
    try {
      const installed = await listInstalledSkills();
      set({ installed, loadingInstalled: false });
    } catch (e) {
      set({
        loadingInstalled: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  refreshAvailable: async () => {
    set({ loadingAvailable: true });
    try {
      const available = await listAvailableSkills();
      set({ available, loadingAvailable: false });
    } catch (e) {
      set({
        loadingAvailable: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  refreshAll: async () => {
    await Promise.all([get().refreshInstalled(), get().refreshAvailable()]);
  },

  install: async (skillId, parentPin) => {
    set({ busy: skillId, error: null });
    try {
      const receipt = await installSkill(skillId, parentPin);
      await get().refreshAll();
      set({ busy: null });
      return receipt;
    } catch (e) {
      set({
        busy: null,
        error: e instanceof Error ? e.message : String(e),
      });
      throw e;
    }
  },

  uninstall: async (skillId) => {
    set({ busy: skillId, error: null });
    try {
      await uninstallSkill(skillId);
      await get().refreshAll();
      set({ busy: null });
    } catch (e) {
      set({
        busy: null,
        error: e instanceof Error ? e.message : String(e),
      });
      throw e;
    }
  },

  toggle: async (skillId, enabled) => {
    set({ busy: skillId, error: null });
    try {
      await toggleSkill(skillId, enabled);
      await get().refreshAll();
      set({ busy: null });
    } catch (e) {
      set({
        busy: null,
        error: e instanceof Error ? e.message : String(e),
      });
      throw e;
    }
  },

  clearError: () => set({ error: null }),
}));