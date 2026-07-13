import { create } from 'zustand';
import {
  createProject,
  deleteProject,
  listProjects,
  loadProject,
  onAssetLocal,
  renameProject,
  saveProjectState,
  type ProjectFull,
  type ProjectMeta,
  type ProjectStatePatch,
  type ProjectSummary,
} from '../api/tauri';
import { useDirectorStore, type DirectorState } from './directorStore';
import { useStudioStore, type StudioState } from './studioStore';

const SAVE_DELAY_MS = 1_000;

type SaveTimer = ReturnType<typeof setTimeout>;

interface ProjectStoreState {
  current: ProjectMeta | null;
  list: ProjectSummary[];
  loading: boolean;
  saving: boolean;
  lastError: string | null;
  localMap: Record<string, string>;
  refresh(): Promise<void>;
  create(title: string, levelId?: string): Promise<ProjectMeta>;
  ensureCurrent(title?: string, levelId?: string): Promise<ProjectMeta>;
  open(id: string): Promise<ProjectFull>;
  rename(id: string, title: string): Promise<void>;
  remove(id: string): Promise<void>;
  scheduleSave(): void;
  flushSave(): Promise<void>;
  resolveLocal(url: string): string | null;
  registerLocal(url: string, localPath: string): void;
  reset(): void;
}

let saveTimer: SaveTimer | null = null;
let createPromise: Promise<ProjectMeta> | null = null;

export const useProjectStore = create<ProjectStoreState>((set, get) => ({
  current: null,
  list: [],
  loading: false,
  saving: false,
  lastError: null,
  localMap: {},

  refresh: async () => {
    set({ loading: true, lastError: null });
    try {
      const projects = await listProjects();
      set({ list: projects ?? [], loading: false });
    } catch (error) {
      set({ loading: false, lastError: errorMessage(error) });
    }
  },

  create: async (title, levelId) => {
    if (get().current) await get().flushSave();
    set({ loading: true, lastError: null });
    try {
      const meta = await createProject(title, levelId);
      const full = await loadProject(meta.id);
      hydrateProject(full);
      set((state) => ({
        current: full.meta,
        list: [full.meta, ...state.list.filter((project) => project.id !== full.meta.id)],
        localMap: {},
        loading: false,
      }));
      return full.meta;
    } catch (error) {
      set({ loading: false, lastError: errorMessage(error) });
      throw error;
    }
  },

  ensureCurrent: async (title = '我的小电影', levelId) => {
    const current = get().current;
    if (current) return current;
    if (!createPromise) {
      createPromise = get()
        .create(title, levelId)
        .finally(() => {
          createPromise = null;
        });
    }
    return createPromise;
  },

  open: async (id) => {
    if (get().current) await get().flushSave();
    set({ loading: true, lastError: null });
    try {
      const full = await loadProject(id);
      hydrateProject(full);
      set((state) => ({
        current: full.meta,
        list: [full.meta, ...state.list.filter((project) => project.id !== full.meta.id)],
        localMap: {},
        loading: false,
      }));
      return full;
    } catch (error) {
      set({ loading: false, lastError: errorMessage(error) });
      throw error;
    }
  },

  rename: async (id, title) => {
    const normalized = title.trim();
    if (!normalized) return;
    set({ lastError: null });
    try {
      await renameProject(id, normalized);
      set((state) => ({
        current:
          state.current?.id === id
            ? { ...state.current, title: normalized, updatedAt: Date.now() }
            : state.current,
        list: state.list.map((project) =>
          project.id === id
            ? { ...project, title: normalized, updatedAt: Date.now() }
            : project,
        ),
      }));
      await get().refresh();
    } catch (error) {
      set({ lastError: errorMessage(error) });
      throw error;
    }
  },

  remove: async (id) => {
    set({ lastError: null });
    try {
      await deleteProject(id);
      const removedCurrent = get().current?.id === id;
      if (removedCurrent) {
        clearSaveTimer();
        useDirectorStore.getState().reset();
        useStudioStore.getState().reset();
      }
      set((state) => ({
        current: removedCurrent ? null : state.current,
        list: state.list.filter((project) => project.id !== id),
      }));
    } catch (error) {
      set({ lastError: errorMessage(error) });
      throw error;
    }
  },

  scheduleSave: () => {
    if (!get().current) return;
    clearSaveTimer();
    saveTimer = setTimeout(() => {
      saveTimer = null;
      void get().flushSave();
    }, SAVE_DELAY_MS);
  },

  flushSave: async () => {
    clearSaveTimer();
    const current = get().current;
    if (!current) return;
    set({ saving: true, lastError: null });
    try {
      const meta = await saveProjectState(
        current.id,
        directorSnapshot(),
        studioSnapshot(),
        projectStatePatch(),
      );
      set((state) => ({
        current: meta,
        list: [meta, ...state.list.filter((project) => project.id !== meta.id)],
        saving: false,
      }));
    } catch (error) {
      set({ saving: false, lastError: errorMessage(error) });
    }
  },

  resolveLocal: (url) => get().localMap[url] ?? null,

  registerLocal: (url, localPath) =>
    set((state) => ({ localMap: { ...state.localMap, [url]: localPath } })),

  reset: () => {
    clearSaveTimer();
    createPromise = null;
    set({
      current: null,
      list: [],
      loading: false,
      saving: false,
      lastError: null,
      localMap: {},
    });
  },
}));

/// W8 M1-D: 优先用 local, 没有就回退到远端.
/// 订阅 localMap 的变化, 本地下载完成后自动 re-render 上层.
export function useLocalAsset(remoteUrl: string | null | undefined): string | null {
  const localMap = useProjectStore((s) => s.localMap);
  if (!remoteUrl) return null;
  return localMap[remoteUrl] ?? remoteUrl;
}

export function initializeProjectPersistence(): () => void {
  const unsubscribeDirector = useDirectorStore.subscribe(() => {
    useProjectStore.getState().scheduleSave();
  });
  const unsubscribeStudio = useStudioStore.subscribe(() => {
    useProjectStore.getState().scheduleSave();
  });
  let disposed = false;
  let unlisten: (() => void) | null = null;
  void onAssetLocal((event) => {
    if (event.status === 'downloaded') {
      useProjectStore.getState().registerLocal(event.url, event.localPath);
    }
  })
    .then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    })
    .catch(() => undefined);

  return () => {
    disposed = true;
    clearSaveTimer();
    unsubscribeDirector();
    unsubscribeStudio();
    unlisten?.();
  };
}

function directorSnapshot(): Record<string, unknown> {
  const state = useDirectorStore.getState();
  return {
    cursor: state.cursor,
    history: state.history,
    idea: state.idea,
    story: state.story,
    character: state.character,
    characterTweak: state.characterTweak,
    style: state.style,
    shots: state.shots,
    finalVideoUrl: state.finalVideoUrl,
    locked_props: state.locked_props,
    videoEngine: state.videoEngine,
    voiceId: state.voiceId,
  };
}

function studioSnapshot(): Record<string, unknown> {
  const state = useStudioStore.getState();
  return {
    items: state.items,
    currentBeatId: state.currentBeatId,
    awaitingFree: state.awaitingFree,
    returnToStoryAfter: state.returnToStoryAfter,
    phase: state.phase,
    previewIndex: state.previewIndex,
    started: state.started,
  };
}

function projectStatePatch(): ProjectStatePatch {
  return {
    cursor: useDirectorStore.getState().cursor,
    totalCredits: useProjectStore.getState().current?.totalCredits ?? 0,
  };
}

function hydrateProject(full: ProjectFull): void {
  useDirectorStore.getState().reset();
  useStudioStore.getState().reset();
  const plan = full.plan;
  if (isRecord(plan)) {
    const patch: Partial<DirectorState> = {};
    if (isDirectorCursor(plan.cursor)) patch.cursor = plan.cursor;
    if (Array.isArray(plan.history)) patch.history = plan.history as DirectorState['history'];
    if (typeof plan.idea === 'string') patch.idea = plan.idea;
    if (isRecord(plan.story)) patch.story = plan.story as unknown as DirectorState['story'];
    if (plan.character === null || isRecord(plan.character)) {
      patch.character = plan.character as DirectorState['character'];
    }
    if (isRecord(plan.characterTweak)) {
      patch.characterTweak = plan.characterTweak as DirectorState['characterTweak'];
    }
    if (plan.style === null || isRecord(plan.style)) {
      patch.style = plan.style as DirectorState['style'];
    }
    if (Array.isArray(plan.shots)) patch.shots = plan.shots as DirectorState['shots'];
    if (plan.finalVideoUrl === null || typeof plan.finalVideoUrl === 'string') {
      patch.finalVideoUrl = plan.finalVideoUrl;
    }
    if (isRecord(plan.locked_props)) {
      patch.locked_props = plan.locked_props as DirectorState['locked_props'];
    }
    if (plan.videoEngine === 'seedance' || plan.videoEngine === 'hailuo') {
      patch.videoEngine = plan.videoEngine;
    }
    if (plan.voiceId === null || typeof plan.voiceId === 'string') patch.voiceId = plan.voiceId;
    useDirectorStore.setState({
      ...patch,
      isLLMRunning: false,
      isVideoRunning: false,
      error: null,
    });
  }

  const transcript = Array.isArray(full.transcript)
    ? { items: full.transcript, started: full.transcript.length > 0 }
    : full.transcript;
  if (isRecord(transcript)) {
    const patch: Partial<StudioState> = {};
    if (Array.isArray(transcript.items)) patch.items = transcript.items as StudioState['items'];
    if (transcript.currentBeatId === null || typeof transcript.currentBeatId === 'string') {
      patch.currentBeatId = transcript.currentBeatId;
    }
    if (typeof transcript.awaitingFree === 'boolean') patch.awaitingFree = transcript.awaitingFree;
    if (typeof transcript.returnToStoryAfter === 'boolean') {
      patch.returnToStoryAfter = transcript.returnToStoryAfter;
    }
    if (isStudioPhase(transcript.phase)) patch.phase = transcript.phase;
    if (typeof transcript.previewIndex === 'number') patch.previewIndex = transcript.previewIndex;
    if (typeof transcript.started === 'boolean') patch.started = transcript.started;
    useStudioStore.setState(patch);
  }
}

function clearSaveTimer(): void {
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isDirectorCursor(value: unknown): value is DirectorState['cursor'] {
  return typeof value === 'number' && value >= 1 && value <= 6;
}

function isStudioPhase(value: unknown): value is StudioState['phase'] {
  return [
    'stage1',
    'plan',
    'character',
    'style',
    'storyboard',
    'preview',
    'finalize',
    'done',
  ].includes(String(value));
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
