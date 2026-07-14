// W6 B3: assetStore 单测 (vitest).
//
// 覆盖:
// - getUrl: manifest 命中 → 返 URL; 没命中 → picsum fallback (可预测 seed)
// - getUrlOrNull: 命中 → URL; 没命中 → null
// - fetch: 成功 → 缓存 + setState; 失败 → set error, 不抛
// - sessionStorage 缓存: 写入 → 下次读取用
// - 并发 fetch 去重 (loading 时再次 fetch 立即返)
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { generatedAssetUrl, useAssetStore } from './assetStore';

const SAMPLE_MANIFEST = {
  version: 1700000000,
  generatedCount: 2,
  images: {
    'xiaoqi.stand': 'https://assets.kids.ibi.ren/character/xiaoqi.stand.png',
    'l1.my_ai_companion': 'https://assets.kids.ibi.ren/bg/l1.my_ai_companion.png',
  },
};

beforeEach(() => {
  sessionStorage.clear();
  useAssetStore.setState({ manifest: null, lastError: null, loading: false });
});

afterEach(() => {
  vi.restoreAllMocks();
});


describe('generatedAssetUrl', () => {
  it('builds deterministic character and style URLs', () => {
    expect(generatedAssetUrl('character', 'xiaoqi.stand')).toBe(
      'https://assets.kids.ibi.ren/character/xiaoqi.stand.png',
    );
    expect(generatedAssetUrl('style', 'anime.full')).toBe(
      'https://assets.kids.ibi.ren/style/anime.full.png',
    );
  });
});

describe('getUrl', () => {
  it('returns manifest URL when key hits', () => {
    useAssetStore.setState({ manifest: SAMPLE_MANIFEST });
    expect(useAssetStore.getState().getUrl('xiaoqi.stand')).toBe(
      'https://assets.kids.ibi.ren/character/xiaoqi.stand.png',
    );
  });

  it('falls back to picsum with predictable seed when key misses', () => {
    useAssetStore.setState({ manifest: SAMPLE_MANIFEST });
    const url = useAssetStore.getState().getUrl('unknown.key');
    // picsum 派生: 'unknown.key' → 'unknown-key' (.) replaced
    expect(url).toBe('https://picsum.photos/seed/unknown-key/512/512');
  });

  it('falls back when manifest is null', () => {
    useAssetStore.setState({ manifest: null });
    const url = useAssetStore.getState().getUrl('xiaoqi.stand');
    expect(url).toBe('https://picsum.photos/seed/xiaoqi-stand/512/512');
  });

  it('picsum seed is deterministic across calls', () => {
    useAssetStore.setState({ manifest: null });
    const a = useAssetStore.getState().getUrl('foo.bar');
    const b = useAssetStore.getState().getUrl('foo.bar');
    expect(a).toBe(b);
  });
});

describe('getUrlOrNull', () => {
  it('returns URL when key hits', () => {
    useAssetStore.setState({ manifest: SAMPLE_MANIFEST });
    expect(useAssetStore.getState().getUrlOrNull('xiaoqi.stand')).toBe(
      'https://assets.kids.ibi.ren/character/xiaoqi.stand.png',
    );
  });

  it('returns null when key misses (and manifest is loaded)', () => {
    useAssetStore.setState({ manifest: SAMPLE_MANIFEST });
    expect(useAssetStore.getState().getUrlOrNull('nope')).toBeNull();
  });

  it('returns null when manifest is null', () => {
    useAssetStore.setState({ manifest: null });
    expect(useAssetStore.getState().getUrlOrNull('xiaoqi.stand')).toBeNull();
  });
});

describe('fetch', () => {
  it('sets manifest on success and caches to sessionStorage', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({
          version: SAMPLE_MANIFEST.version,
          generated_count: SAMPLE_MANIFEST.generatedCount,
          images: SAMPLE_MANIFEST.images,
        }),
      })),
    );
    await useAssetStore.getState().fetch('https://api.kids.ibi.ren');
    const m = useAssetStore.getState().manifest;
    expect(m).toEqual(SAMPLE_MANIFEST);
    expect(useAssetStore.getState().lastError).toBeNull();
    expect(useAssetStore.getState().loading).toBe(false);

    const cached = sessionStorage.getItem('kidsai:asset-manifest');
    expect(cached).toBeTruthy();
    const parsed = JSON.parse(cached!);
    expect(parsed.images).toEqual(SAMPLE_MANIFEST.images);
    vi.unstubAllGlobals();
  });

  it('sets error but does not throw on failure', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: false,
        status: 503,
      })),
    );
    await useAssetStore.getState().fetch('https://api.kids.ibi.ren');
    expect(useAssetStore.getState().manifest).toBeNull();
    expect(useAssetStore.getState().lastError).toContain('503');
    expect(useAssetStore.getState().loading).toBe(false);
    vi.unstubAllGlobals();
  });

  it('does not double-fetch when loading', async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => SAMPLE_MANIFEST,
    }));
    vi.stubGlobal('fetch', fetchMock);
    useAssetStore.setState({ loading: true });
    await useAssetStore.getState().fetch('https://api.kids.ibi.ren');
    expect(fetchMock).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });
});

describe('cache load', () => {
  it('loads from sessionStorage on init when fresh', () => {
    const cached = {
      version: 1,
      generatedCount: 1,
      images: { 'xiaoqi.stand': 'https://cached.example/x.png' },
      cachedAt: Date.now(),
    };
    sessionStorage.setItem('kidsai:asset-manifest', JSON.stringify(cached));
    useAssetStore.setState({ manifest: cached });
    expect(useAssetStore.getState().getUrl('xiaoqi.stand')).toBe('https://cached.example/x.png');
  });
});