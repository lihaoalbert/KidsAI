// W6 B3: 资产 manifest zustand store.
//
// 桌面端启动时拉一次 `GET /api/v1/asset-manifest`, 缓存 key→URL 映射.
// 后续 `getUrl('character.xiaoqi.stand')` 直接命中, 找不到返 picsum fallback.
//
// 设计原则:
// - 失败不阻断 — 拉不到 manifest 也能跑 (走 fallback)
// - sessionStorage 二次缓存 — 避免每次刷新都打 server
// - 占位图统一走 picsum, 跨组件保持一致
// - 6 分钟 stale time — server 1h cache, 客户端 6 分钟够用又不频繁

import { create } from 'zustand';
import { fetchAssetManifest, type AssetManifest } from '../api/tauri';

const STORAGE_KEY = 'kidsai:asset-manifest';
const STALE_MS = 6 * 60 * 1000;
const ASSET_BASE_URL = (
  import.meta.env.VITE_KIDSAI_ASSETS_URL ?? 'https://assets.kids.ibi.ren'
).replace(/\/$/, '');

export function generatedAssetUrl(kind: string, key: string): string {
  return `${ASSET_BASE_URL}/${encodeURIComponent(kind)}/${encodeURIComponent(key)}.png`;
}

interface CachedManifest {
  version: number;
  images: Record<string, string>;
  cachedAt: number;
}

function loadCache(): CachedManifest | null {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as CachedManifest;
    if (typeof parsed.cachedAt !== 'number' || Date.now() - parsed.cachedAt > STALE_MS) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

function saveCache(m: AssetManifest): void {
  try {
    const payload: CachedManifest = {
      version: m.version,
      images: m.images,
      cachedAt: Date.now(),
    };
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // sessionStorage 不可用 (隐私模式等) — 跳过, 下次再拉
  }
}

export interface AssetStore {
  /** 当前 manifest. null = 还没拉 / 拉失败 */
  manifest: AssetManifest | null;
  /** 最后一次 fetch 的 error (用于调试) */
  lastError: string | null;
  /** 是否正在 fetch */
  loading: boolean;

  /** 从 server 拉 manifest, 缓存到 sessionStorage. 失败不抛, 静默 fallback. */
  fetch: (serverUrl: string) => Promise<void>;

  /** 拿 key 对应的 URL. 找不到或 manifest 还没好 → 返 picsum placeholder. */
  getUrl: (key: string) => string;

  /** 拿 key 对应的 URL, 但找不到时返 null (用于"该 key 真没有"的场景). */
  getUrlOrNull: (key: string) => string | null;
}

export const useAssetStore = create<AssetStore>((set, get) => ({
  manifest: loadCache() as AssetManifest | null,
  lastError: null,
  loading: false,

  fetch: async (serverUrl: string) => {
    if (get().loading) return;
    set({ loading: true, lastError: null });
    try {
      const m = await fetchAssetManifest(serverUrl);
      saveCache(m);
      set({ manifest: m, loading: false });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ loading: false, lastError: msg });
      // 不抛 — 让 UI 走 picsum fallback
    }
  },

  getUrl: (key: string) => {
    const m = get().manifest;
    return m?.images[key] ?? picsumFallback(key);
  },

  getUrlOrNull: (key: string) => {
    const m = get().manifest;
    return m?.images[key] ?? null;
  },
}));

/**
 * picsum fallback: 用 key 派生 seed, 同 key 永远返同图 (cacheable).
 * 跟"manifest URL 在浏览器里走 immutable cache"的语义对齐.
 */
function picsumFallback(key: string): string {
  // seed 限 ASCII 字符; key 含 '.' (e.g. "xiaoqi.stand") 也兼容
  const seed = key.replace(/[^a-zA-Z0-9_-]/g, '-');
  return `https://picsum.photos/seed/${seed}/512/512`;
}

/// 关卡 L1-L7 主题背景 key 映射 (W6 E1 用)
export const LEVEL_BG_KEYS: Record<string, string> = {
  L1: 'l1.my_ai_companion',
  L2: 'l2.storybook',
  L3: 'l3.adventure',
  L4: 'l4.science',
  L5: 'l5.music',
  L6: 'l6.friendship',
  L7: 'l7.dream',
};

/// 关卡封面 key 映射 (W6 E1 用)
export const LEVEL_COVER_KEYS: Record<string, string> = {
  L1: 'cover.l1',
  L2: 'cover.l2',
  L3: 'cover.l3',
  L4: 'cover.l4',
  L5: 'cover.l5',
  L6: 'cover.l6',
  L7: 'cover.l7',
};

/// 角色立绘 key 映射 — 4 种姿势 (W6 E3 用)
export const CHARACTER_POSE_KEYS = ['stand', 'sit', 'run', 'fly', 'smile'] as const;
export type CharacterPose = typeof CHARACTER_POSE_KEYS[number];

export function characterImageKey(characterId: string, pose: CharacterPose): string {
  return `${characterId}.${pose}`;
}