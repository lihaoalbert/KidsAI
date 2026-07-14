// W9: 左栏 Story Workspace — 项目缩略图 + 进度环 + 故事状态网格
// 渐进披露：默认显示进度 + 状态网格；点 "⚙️ 高级" 展开 JSON/版本/导出

import { useState } from 'react';
import StoryStateGrid from './StoryStateGrid';
import { useDirectorStore } from '../../stores/directorStore';

export default function StoryWorkspace() {
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const idea = useDirectorStore((s) => s.idea);
  const shots = useDirectorStore((s) => s.shots);
  const finalVideoUrl = useDirectorStore((s) => s.finalVideoUrl);
  const sessionCredits = useDirectorStore((s) => s.sessionCredits);
  const versions = useDirectorStore((s) => s.versions);
  const activeVersionId = useDirectorStore((s) => s.activeVersionId);
  const saveVersion = useDirectorStore((s) => s.saveVersion);
  const switchVersion = useDirectorStore((s) => s.switchVersion);

  // 进度环: 用 shots + finalVideoUrl 算 0..100
  const progress = (() => {
    if (finalVideoUrl) return 100;
    if (shots.length > 0) return 60 + Math.min(shots.length * 4, 30);
    return 10;
  })();

  const projectName = idea.trim().slice(0, 12) || '我的小电影';

  return (
    <div className="flex h-full w-full flex-col border-r border-line bg-bg/60">
      {/* 项目头 */}
      <div className="border-b border-line bg-surface px-3 py-3">
        <div className="flex items-center gap-3">
          <div
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-accent-200 to-accent-400 text-lg font-bold text-bg"
            aria-label="项目缩略图"
          >
            🎬
          </div>
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-semibold text-ink-2">{projectName}</div>
            <div className="flex items-center gap-2 text-[10px] text-ink-2">
              <span>进度 {progress}%</span>
              <span>·</span>
              <span>{sessionCredits} 学币</span>
            </div>
          </div>
        </div>
        {/* 进度条 */}
        <div className="mt-2 h-1 w-full overflow-hidden rounded-full bg-surface-2">
          <div
            className="h-full bg-gradient-to-r from-accent-400 to-accent-600 transition-all"
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>

      {/* 故事状态网格 */}
      <div className="flex-1 overflow-auto px-3 py-3">
        <div className="mb-2 flex items-center justify-between">
          <h3 className="text-xs font-semibold uppercase tracking-wide text-ink-2">
            🎯 故事状态
          </h3>
          <button
            type="button"
            onClick={() => setAdvancedOpen((v) => !v)}
            className="rounded-md px-2 py-0.5 text-[10px] text-ink-3 hover:bg-surface-2 hover:text-ink-2"
          >
            {advancedOpen ? '收起高级' : '⚙️ 高级'}
          </button>
        </div>
        <StoryStateGrid />

        {advancedOpen && (
          <div className="mt-4 space-y-3 border-t border-line pt-3">
            {/* 多版本切换 */}
            <div>
              <h4 className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-ink-2">
                🗂️ 版本
              </h4>
              <div className="space-y-1">
                {Object.values(versions).map((v) => (
                  <button
                    key={v.id}
                    type="button"
                    onClick={() => switchVersion(v.id)}
                    className={[
                      'flex w-full items-center justify-between rounded-md px-2 py-1 text-left text-xs',
                      v.id === activeVersionId
                        ? 'bg-accent-soft-2 text-accent-ink'
                        : 'bg-surface text-ink-2 hover:bg-surface-2',
                    ].join(' ')}
                  >
                    <span className="truncate">{v.name}</span>
                    {v.id === activeVersionId && <span className="text-[10px]">●</span>}
                  </button>
                ))}
                <button
                  type="button"
                  onClick={() => {
                    const name = prompt('版本名', `版本 ${Object.keys(versions).length + 1}`);
                    if (name !== null) saveVersion(name);
                  }}
                  className="w-full rounded-md border border-dashed border-line px-2 py-1 text-xs text-ink-2 hover:border-accent hover:text-accent-ink"
                >
                  ＋ 保存当前为版本
                </button>
              </div>
            </div>

            {/* JSON 视图占位 — B3 阶段实装 */}
            <div>
              <h4 className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-ink-2">
                🔍 原始数据 (JSON)
              </h4>
              <pre className="max-h-48 overflow-auto rounded-md bg-code p-2 text-[10px] text-ink-3">
{JSON.stringify({
  idea,
  shots: shots.length,
  finalVideoUrl: finalVideoUrl ? '(已生成)' : null,
}, null, 2)}
              </pre>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}