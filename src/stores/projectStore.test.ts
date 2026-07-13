// projectStore 测试 (W8 M1-B / M1-E)
// 覆盖:
//   - refresh: 拉远端列表 → 写入 store
//   - create: 写元数据 + 调 loadProject 水合 director + studio
//   - ensureCurrent: 第二次并发调用复用第一次的 create promise
//   - open: 替换 current, 重置 localMap
//   - rename: 改 list + current, 调远端
//   - remove: 从 list 删除; 若删的是 current → reset director + studio
//   - scheduleSave / flushSave: debounce 1s, flush 立即触发
//   - resolveLocal / registerLocal: 映射表
//   - asset://local 事件 → registerLocal
//   - 错误路径: refresh 失败 → lastError; flushSave 失败 → lastError

import { describe, it, expect, beforeEach, afterEach, vi, type Mock } from 'vitest';
import type {
  AssetLocalEvent,
  ProjectFull,
  ProjectMeta,
} from '../api/tauri';

// ============ Mocks ============
let assetHandler: ((e: AssetLocalEvent) => void) | null = null;
let assetUnlisten: (() => void) | null = null;

const listProjectsMock: Mock = vi.fn();
const loadProjectMock: Mock = vi.fn();
const createProjectMock: Mock = vi.fn();
const renameProjectMock: Mock = vi.fn();
const deleteProjectMock: Mock = vi.fn();
const saveProjectStateMock: Mock = vi.fn();

vi.mock('../api/tauri', () => ({
  listProjects: (...args: unknown[]) => listProjectsMock(...args),
  loadProject: (...args: unknown[]) => loadProjectMock(...args),
  createProject: (...args: unknown[]) => createProjectMock(...args),
  renameProject: (...args: unknown[]) => renameProjectMock(...args),
  deleteProject: (...args: unknown[]) => deleteProjectMock(...args),
  saveProjectState: (...args: unknown[]) => saveProjectStateMock(...args),
  onAssetLocal: async (handler: (e: AssetLocalEvent) => void) => {
    assetHandler = handler;
    assetUnlisten = () => {
      assetHandler = null;
    };
    return assetUnlisten;
  },
}));

import { useProjectStore } from './projectStore';
import { useDirectorStore } from './directorStore';
import { useStudioStore } from './studioStore';

function meta(overrides: Partial<ProjectMeta> = {}): ProjectMeta {
  return {
    id: 'p1',
    title: '我的小电影',
    levelId: null,
    cursor: 0,
    thumbPath: null,
    totalCredits: 0,
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_000,
    ...overrides,
  };
}

function full(overrides: Partial<ProjectFull> = {}): ProjectFull {
  return {
    meta: meta(overrides.meta),
    plan: { cursor: 1, idea: 'a' },
    transcript: { items: [], started: false },
    ...overrides,
  };
}

function emitAsset(event: AssetLocalEvent) {
  if (!assetHandler) throw new Error('handler not captured');
  assetHandler(event);
}

describe('projectStore', () => {
  beforeEach(() => {
    listProjectsMock.mockReset();
    loadProjectMock.mockReset();
    createProjectMock.mockReset();
    renameProjectMock.mockReset();
    deleteProjectMock.mockReset();
    saveProjectStateMock.mockReset();
    assetHandler = null;
    assetUnlisten = null;

    useProjectStore.getState().reset();
    useDirectorStore.getState().reset();
    useStudioStore.getState().reset();
  });

  // ---------- refresh ----------
  describe('refresh', () => {
    it('拉远端列表写入 list', async () => {
      const projects = [meta({ id: 'a' }), meta({ id: 'b' })];
      listProjectsMock.mockResolvedValueOnce(projects);
      await useProjectStore.getState().refresh();
      expect(useProjectStore.getState().list).toHaveLength(2);
      expect(useProjectStore.getState().list[0].id).toBe('a');
      expect(useProjectStore.getState().loading).toBe(false);
    });

    it('失败 → lastError', async () => {
      listProjectsMock.mockRejectedValueOnce(new Error('boom'));
      await useProjectStore.getState().refresh();
      expect(useProjectStore.getState().lastError).toBe('boom');
      expect(useProjectStore.getState().loading).toBe(false);
    });
  });

  // ---------- create ----------
  describe('create', () => {
    it('写元数据 + 调 loadProject 水合 stores', async () => {
      createProjectMock.mockResolvedValueOnce(meta({ id: 'new1' }));
      loadProjectMock.mockResolvedValueOnce(full({ meta: meta({ id: 'new1' }) }));
      const result = await useProjectStore.getState().create('新项目');
      expect(result.id).toBe('new1');
      const state = useProjectStore.getState();
      expect(state.current?.id).toBe('new1');
      expect(state.list[0].id).toBe('new1');
      // 触发了 directorStore 水合
      expect(useDirectorStore.getState().cursor).toBe(1);
    });

    it('失败 → throw + lastError', async () => {
      createProjectMock.mockRejectedValueOnce(new Error('create fail'));
      await expect(useProjectStore.getState().create('x')).rejects.toThrow('create fail');
      expect(useProjectStore.getState().lastError).toBe('create fail');
    });
  });

  // ---------- ensureCurrent ----------
  describe('ensureCurrent', () => {
    it('已有 current 直接返回', async () => {
      createProjectMock.mockResolvedValueOnce(meta({ id: 'once' }));
      loadProjectMock.mockResolvedValueOnce(full({ meta: meta({ id: 'once' }) }));
      await useProjectStore.getState().create('first');
      createProjectMock.mockClear();
      const m = await useProjectStore.getState().ensureCurrent();
      expect(m.id).toBe('once');
      expect(createProjectMock).not.toHaveBeenCalled();
    });

    it('并发 ensureCurrent 复用同一个 create promise', async () => {
      let resolveCreate!: (v: ProjectMeta) => void;
      createProjectMock.mockImplementationOnce(
        () => new Promise<ProjectMeta>((res) => { resolveCreate = res; }),
      );
      loadProjectMock.mockResolvedValueOnce(full({ meta: meta({ id: 'one' }) }));
      const p1 = useProjectStore.getState().ensureCurrent();
      const p2 = useProjectStore.getState().ensureCurrent();
      resolveCreate(meta({ id: 'one' }));
      const [a, b] = await Promise.all([p1, p2]);
      expect(createProjectMock).toHaveBeenCalledTimes(1);
      expect(a.id).toBe('one');
      expect(b.id).toBe('one');
    });
  });

  // ---------- open ----------
  describe('open', () => {
    it('替换 current, 重置 localMap', async () => {
      useProjectStore.setState({ localMap: { 'old': '/x' } });
      loadProjectMock.mockResolvedValueOnce(full({ meta: meta({ id: 'p2' }) }));
      const result = await useProjectStore.getState().open('p2');
      expect(result.meta.id).toBe('p2');
      expect(useProjectStore.getState().current?.id).toBe('p2');
      expect(useProjectStore.getState().localMap).toEqual({});
    });
  });

  // ---------- rename ----------
  describe('rename', () => {
    it('更新 list + current 标题', async () => {
      loadProjectMock.mockResolvedValueOnce(full());
      await useProjectStore.getState().open('p1');
      useProjectStore.setState({
        list: [meta({ id: 'p1' }), meta({ id: 'p2' })],
      });
      renameProjectMock.mockResolvedValueOnce(undefined);
      listProjectsMock.mockResolvedValueOnce([meta({ id: 'p1', title: '新' })]);
      await useProjectStore.getState().rename('p1', '  新  ');
      expect(useProjectStore.getState().current?.title).toBe('新');
      expect(useProjectStore.getState().list[0].title).toBe('新');
      expect(renameProjectMock).toHaveBeenCalledWith('p1', '新');
    });

    it('空字符串 → 不发请求', async () => {
      await useProjectStore.getState().rename('p1', '   ');
      expect(renameProjectMock).not.toHaveBeenCalled();
    });
  });

  // ---------- remove ----------
  describe('remove', () => {
    it('从 list 删除; 删的是 current → 重置 director + studio', async () => {
      loadProjectMock.mockResolvedValueOnce(full());
      await useProjectStore.getState().open('p1');
      useProjectStore.setState({ list: [meta({ id: 'p1' })] });
      useDirectorStore.getState().setStorySlot('who', '小猫');
      deleteProjectMock.mockResolvedValueOnce(undefined);
      await useProjectStore.getState().remove('p1');
      expect(useProjectStore.getState().current).toBeNull();
      expect(useProjectStore.getState().list).toHaveLength(0);
      expect(useDirectorStore.getState().story.who).toBe('');
    });
  });

  // ---------- scheduleSave / flushSave ----------
  describe('save', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });
    afterEach(() => {
      vi.useRealTimers();
    });

    it('scheduleSave debounce 1s 后 flush', async () => {
      loadProjectMock.mockResolvedValueOnce(full());
      await useProjectStore.getState().open('p1');
      saveProjectStateMock.mockResolvedValueOnce(meta({ id: 'p1' }));
      useProjectStore.getState().scheduleSave();
      expect(saveProjectStateMock).not.toHaveBeenCalled();
      vi.advanceTimersByTime(999);
      expect(saveProjectStateMock).not.toHaveBeenCalled();
      vi.advanceTimersByTime(2);
      expect(saveProjectStateMock).toHaveBeenCalledTimes(1);
    });

    it('多次 scheduleSave → 只触发 1 次', async () => {
      loadProjectMock.mockResolvedValueOnce(full());
      await useProjectStore.getState().open('p1');
      saveProjectStateMock.mockResolvedValueOnce(meta({ id: 'p1' }));
      useProjectStore.getState().scheduleSave();
      useProjectStore.getState().scheduleSave();
      useProjectStore.getState().scheduleSave();
      vi.advanceTimersByTime(1100);
      expect(saveProjectStateMock).toHaveBeenCalledTimes(1);
    });

    it('flushSave 立即写, 失败 → lastError', async () => {
      loadProjectMock.mockResolvedValueOnce(full());
      await useProjectStore.getState().open('p1');
      saveProjectStateMock.mockRejectedValueOnce(new Error('disk full'));
      await useProjectStore.getState().flushSave();
      expect(useProjectStore.getState().lastError).toBe('disk full');
      expect(useProjectStore.getState().saving).toBe(false);
    });

    it('无 current 时 flushSave 是 noop', async () => {
      await useProjectStore.getState().flushSave();
      expect(saveProjectStateMock).not.toHaveBeenCalled();
    });
  });

  // ---------- localMap ----------
  describe('localMap', () => {
    it('registerLocal + resolveLocal 闭环', () => {
      useProjectStore.getState().registerLocal('https://x/a.png', '/local/a.png');
      expect(useProjectStore.getState().resolveLocal('https://x/a.png')).toBe('/local/a.png');
      expect(useProjectStore.getState().resolveLocal('https://nope')).toBeNull();
    });

    it('initializeProjectPersistence 订阅 asset://local 事件', async () => {
      const { initializeProjectPersistence } = await import('./projectStore');
      const stop = initializeProjectPersistence();
      // 等待 onAssetLocal promise resolve
      await new Promise((r) => setTimeout(r, 0));
      emitAsset({
        projectId: 'p1',
        url: 'https://x/v.mp4',
        localPath: '/local/v.mp4',
        status: 'downloaded',
      });
      expect(useProjectStore.getState().resolveLocal('https://x/v.mp4')).toBe('/local/v.mp4');
      stop();
    });
  });
});
