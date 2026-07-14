// W11 Day 7 — Secrets store (frontend)
// 缓存当前已装 secret 版本 + 检查更新 wrapper

import { create } from 'zustand';
import {
  applySecretsUpdate,
  checkSecretsUpdate,
  getCurrentSecretVersion,
  rollbackSecrets,
  type UpdateInfo,
} from '../api/tauri';

interface SecretsState {
  /** profile → 已装 version (例 { child: "v1.a3f9c2", adult: "v1.adult01" }) */
  currentVersions: Record<string, string>;
  /** 最近一次检查更新的 server-side 待装版本 */
  availableUpdates: UpdateInfo[];
  /** 正在拉取 / 应用更新 */
  loading: boolean;
  /** 上次错误信息 */
  error: string | null;
  /** 上次检查时间 (ms) */
  lastCheckedAt: number | null;

  refreshVersions: () => Promise<void>;
  checkUpdates: () => Promise<void>;
  applyUpdate: (profile: string, parentPin: string) => Promise<string>;
  rollback: (profile: string, toVersion: string) => Promise<void>;
}

export const useSecretsStore = create<SecretsState>((set) => ({
  currentVersions: {},
  availableUpdates: [],
  loading: false,
  error: null,
  lastCheckedAt: null,

  refreshVersions: async () => {
    set({ loading: true, error: null });
    try {
      const versions = await getCurrentSecretVersion();
      set({ currentVersions: versions, loading: false });
    } catch (e) {
      set({ loading: false, error: String(e) });
    }
  },

  checkUpdates: async () => {
    set({ loading: true, error: null });
    try {
      const updates = await checkSecretsUpdate();
      set({
        availableUpdates: updates,
        loading: false,
        lastCheckedAt: Date.now(),
      });
    } catch (e) {
      set({ loading: false, error: String(e) });
    }
  },

  applyUpdate: async (profile: string, parentPin: string) => {
    set({ loading: true, error: null });
    try {
      const newVersion = await applySecretsUpdate(profile, parentPin);
      const versions = await getCurrentSecretVersion();
      set({
        currentVersions: versions,
        loading: false,
        availableUpdates: [],
      });
      return newVersion;
    } catch (e) {
      set({ loading: false, error: String(e) });
      throw e;
    }
  },

  rollback: async (profile: string, toVersion: string) => {
    set({ loading: true, error: null });
    try {
      await rollbackSecrets(profile, toVersion);
      const versions = await getCurrentSecretVersion();
      set({ currentVersions: versions, loading: false });
    } catch (e) {
      set({ loading: false, error: String(e) });
      throw e;
    }
  },
}));