import { useEffect, useState } from 'react';
import Card from '../components/Card';
import { listCreations, type CreationWithAssets } from '../api/tauri';
import { useLevelStore } from '../stores/levelStore';
import { useProjectStore } from '../stores/projectStore';
import { convertFileSrc } from '@tauri-apps/api/core';

export default function LibraryPage() {
  const [creations, setCreations] = useState<CreationWithAssets[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { levels } = useLevelStore();
  const projects = useProjectStore((s) => s.list);
  const projectsLoading = useProjectStore((s) => s.loading);
  const refreshProjects = useProjectStore((s) => s.refresh);

  useEffect(() => {
    void refreshProjects();
  }, [refreshProjects]);

  const refresh = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const list = await listCreations();
      setCreations(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const levelTitle = (id: string) =>
    levels.find((l) => l.id === id)?.title ?? id;

  return (
    <div className="p-8 max-w-6xl mx-auto">
      <div className="flex items-center justify-between mb-2">
        <h1 className="text-2xl font-bold text-ink">📚 作品库</h1>
        <button
          onClick={refresh}
          className="text-sm text-accent-ink hover:underline"
        >
          刷新
        </button>
      </div>
      <p className="text-sm text-ink-2 mb-8">
        你创作的所有作品都会保存在这里（W2.3: 已接入本地 SQLite）
      </p>

      {error && (
        <div className="mb-4 p-3 rounded-md bg-danger-soft border border-danger-soft text-sm text-danger">
          ⚠️ {error}
        </div>
      )}

      {isLoading ? (
        <div className="text-sm text-ink-2 py-12 text-center">加载中…</div>
      ) : (
        <>
          {/* W8: 我的项目区 (源 = projectStore) */}
          <section className="mb-8">
            <h2 className="text-lg font-semibold text-ink mb-3">我的项目</h2>
            {projectsLoading && projects.length === 0 ? (
              <div className="text-sm text-ink-2 py-6 text-center bg-surface rounded-lg border border-dashed border-line">
                正在读取项目…
              </div>
            ) : projects.length === 0 ? (
              <div className="rounded-lg bg-surface border border-dashed border-line p-8 text-center">
                <div className="text-3xl mb-2">🎬</div>
                <div className="text-sm font-semibold text-ink-2">还没有项目</div>
                <div className="text-xs text-ink-3 mt-1">
                  进入「作品工坊」开始一个故事后，会在这里列出。
                </div>
              </div>
            ) : (
              <div className="grid grid-cols-3 gap-3">
                {projects.map((p) => (
                  <div
                    key={p.id}
                    className="rounded-xl border border-line bg-surface p-3 hover:border-accent hover:shadow-sm transition"
                  >
                    <div className="aspect-video rounded-lg bg-gradient-to-br from-accent-50 to-highlight/50 mb-2 overflow-hidden flex items-center justify-center">
                      {p.thumbPath ? (
                        <img
                          src={localThumb(p.thumbPath) ?? ''}
                          alt=""
                          className="w-full h-full object-cover"
                        />
                      ) : (
                        <span className="text-3xl">🎬</span>
                      )}
                    </div>
                    <div className="text-sm font-semibold text-ink truncate">{p.title}</div>
                    <div className="mt-1 flex items-center justify-between text-xs text-ink-2">
                      <span className="rounded-full bg-surface-2 px-1.5 py-0.5">
                        {stageLabel(p.cursor)}
                      </span>
                      <span>已用 {p.totalCredits} 学币</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>

          {/* 老 W2.3 creations 区 (兼容历史数据) */}
          <section>
            <h2 className="text-lg font-semibold text-ink mb-3">历史作品（W2.3）</h2>
            {creations.length === 0 ? (
              <div className="rounded-lg bg-surface border border-dashed border-line p-12 text-center">
                <div className="text-3xl mb-2">🎨</div>
                <div className="text-sm font-semibold text-ink-2">还没有历史作品</div>
                <div className="text-xs text-ink-3 mt-1">
                  完成旧版关卡后，作品会出现在这里。
                </div>
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-4">
                {creations.map((c) => (
                  <Card
                    key={c.id}
                    title={levelTitle(c.levelId)}
                    description={c.userInput}
                    footer={
                      <div className="flex items-center justify-between text-xs text-ink-2">
                        <span>得分 {c.score ?? '-'}</span>
                        <span>{new Date(c.createdAt).toLocaleString()}</span>
                      </div>
                    }
                  >
                    <div className="space-y-2">
                      {c.assets.length > 0 ? (
                        <div className="grid grid-cols-2 gap-2">
                          {c.assets.map((a, i) => (
                            <div
                              key={i}
                              className="aspect-video bg-surface-2 rounded-md flex items-center justify-center text-2xl"
                            >
                              {a.kind === 'image' && '🖼️'}
                              {a.kind === 'video' && '🎬'}
                              {a.kind === 'audio' && '🔊'}
                            </div>
                          ))}
                        </div>
                      ) : (
                        <div className="aspect-video bg-gradient-to-br from-accent-50 to-highlight/50 rounded-md flex items-center justify-center text-3xl">
                          🎨
                        </div>
                      )}
                      {c.feedback && (
                        <div className="text-xs text-ink-2 bg-surface-2 rounded p-2">
                          {c.feedback}
                        </div>
                      )}
                    </div>
                  </Card>
                ))}
              </div>
            )}
          </section>
        </>
      )}
    </div>
  );
}

function localThumb(path: string | null): string | null {
  if (!path) return null;
  if (/^(https?:|data:|blob:)/.test(path)) return path;
  return '__TAURI_INTERNALS__' in window ? convertFileSrc(path) : path;
}

function stageLabel(cursor: number): string {
  return ['刚开始', '点子', '主角', '画风', '分镜', '试拍', '定稿'][cursor] ?? '创作中';
}
