import { convertFileSrc } from '@tauri-apps/api/core';
import { useProjectStore } from '../../stores/projectStore';
import { useStudioStore } from '../../stores/studioStore';
import type { ProjectSummary } from '../../api/tauri';

interface ProjectsPaneProps {
  onBackHome: () => void;
}

export default function ProjectsPane({ onBackHome }: ProjectsPaneProps) {
  const current = useProjectStore((state) => state.current);
  const projects = useProjectStore((state) => state.list);
  const loading = useProjectStore((state) => state.loading);
  const saving = useProjectStore((state) => state.saving);
  const lastError = useProjectStore((state) => state.lastError);

  const openProject = async (id: string) => {
    try {
      await useProjectStore.getState().open(id);
    } catch {
      return;
    }
  };

  const createNew = async () => {
    try {
      await useProjectStore.getState().create(`我的小电影 ${projects.length + 1}`);
      useStudioStore.getState().start();
    } catch {
      return;
    }
  };

  const rename = async (project: ProjectSummary) => {
    const title = window.prompt('给项目起个名字', project.title)?.trim();
    if (!title || title === project.title) return;
    try {
      await useProjectStore.getState().rename(project.id, title);
    } catch {
      return;
    }
  };

  const remove = async (project: ProjectSummary) => {
    if (!window.confirm(`删除“${project.title}”？项目会先移到本机回收区。`)) return;
    try {
      await useProjectStore.getState().remove(project.id);
    } catch {
      return;
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center justify-between px-4 pb-2 pt-4">
        <div>
          <div className="text-sm font-bold text-gray-900">我的项目</div>
          <div className="mt-0.5 text-[11px] text-gray-400">
            {saving ? '正在保存…' : '自动保存在本机'}
          </div>
        </div>
        <button
          type="button"
          onClick={onBackHome}
          className="rounded-lg px-2 py-1 text-xs text-gray-500 hover:bg-gray-100"
        >
          首页
        </button>
      </div>

      {current && (
        <div className="mx-3 mb-3 rounded-xl border border-brand-200 bg-brand-50 p-3">
          <div className="flex gap-2.5">
            <ProjectThumb project={current} large />
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-bold text-gray-900">{current.title}</div>
              <div className="mt-1 flex flex-wrap gap-1.5 text-[11px]">
                <span className="rounded-full bg-white px-2 py-0.5 text-brand-700">
                  {stageLabel(current.cursor)}
                </span>
                <span className="rounded-full bg-white px-2 py-0.5 text-gray-600">
                  已用 {current.totalCredits} 学币
                </span>
              </div>
            </div>
          </div>
          <div className="mt-2 flex justify-end gap-1">
            <button
              type="button"
              onClick={() => void rename(current)}
              className="rounded-md px-2 py-1 text-[11px] text-gray-600 hover:bg-white"
            >
              重命名
            </button>
            <button
              type="button"
              onClick={() => void remove(current)}
              className="rounded-md px-2 py-1 text-[11px] text-red-500 hover:bg-red-50"
            >
              删除
            </button>
          </div>
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto px-3 pb-3">
        {loading && projects.length === 0 ? (
          <div className="py-8 text-center text-xs text-gray-400">正在读取项目…</div>
        ) : projects.length === 0 ? (
          <div className="rounded-xl border border-dashed border-gray-200 px-3 py-8 text-center">
            <div className="text-2xl">🎬</div>
            <div className="mt-2 text-xs font-semibold text-gray-700">还没有项目</div>
            <div className="mt-1 text-[11px] leading-5 text-gray-400">
              开始一个故事后，聊天和作品都会留在本机。
            </div>
          </div>
        ) : (
          <div className="space-y-1.5">
            {projects.map((project) => {
              const active = current?.id === project.id;
              return (
                <div
                  key={project.id}
                  className={`group flex items-center gap-1 rounded-xl border p-1.5 ${
                    active
                      ? 'border-brand-200 bg-brand-50'
                      : 'border-transparent hover:border-gray-200 hover:bg-gray-50'
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => void openProject(project.id)}
                    className="flex min-w-0 flex-1 items-center gap-2 text-left"
                    aria-label={`打开项目 ${project.title}`}
                  >
                    <ProjectThumb project={project} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-xs font-semibold text-gray-800">
                        {project.title}
                      </span>
                      <span className="mt-0.5 block truncate text-[10px] text-gray-400">
                        {stageLabel(project.cursor)} · {relativeTime(project.updatedAt)}
                      </span>
                    </span>
                  </button>
                  <button
                    type="button"
                    onClick={() => void rename(project)}
                    title="重命名"
                    className="hidden h-7 w-7 items-center justify-center rounded-md text-xs text-gray-400 hover:bg-white group-hover:flex"
                  >
                    ✎
                  </button>
                </div>
              );
            })}
          </div>
        )}

        {lastError && (
          <div className="mt-2 rounded-lg bg-red-50 px-2 py-1.5 text-[11px] text-red-600">
            {lastError}
          </div>
        )}
      </div>

      <div className="border-t border-gray-100 p-3">
        <button
          type="button"
          onClick={() => void createNew()}
          disabled={loading}
          className="w-full rounded-xl bg-brand-600 px-3 py-2 text-xs font-semibold text-white hover:bg-brand-700 disabled:opacity-50"
        >
          ＋ 新建项目
        </button>
      </div>
    </div>
  );
}

function ProjectThumb({ project, large = false }: { project: ProjectSummary; large?: boolean }) {
  const source = thumbSource(project.thumbPath);
  const size = large ? 'h-12 w-16' : 'h-10 w-12';
  if (source) {
    return (
      <img
        src={source}
        alt=""
        className={`${size} shrink-0 rounded-lg bg-gray-100 object-cover`}
      />
    );
  }
  return (
    <span
      className={`${size} flex shrink-0 items-center justify-center rounded-lg bg-white text-lg shadow-sm`}
    >
      🎬
    </span>
  );
}

function thumbSource(path: string | null): string | null {
  if (!path) return null;
  if (/^(https?:|data:|blob:)/.test(path)) return path;
  return '__TAURI_INTERNALS__' in window ? convertFileSrc(path) : path;
}

function stageLabel(cursor: number): string {
  return ['刚开始', '点子', '主角', '画风', '分镜', '试拍', '定稿'][cursor] ?? '创作中';
}

function relativeTime(timestamp: number): string {
  const elapsed = Math.max(0, Date.now() - timestamp);
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return '刚刚';
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  return new Date(timestamp).toLocaleDateString('zh-CN');
}
