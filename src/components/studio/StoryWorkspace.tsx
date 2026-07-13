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
    <div className="flex h-full w-full flex-col border-r border-gray-200 bg-warm-50/60">
      {/* 项目头 */}
      <div className="border-b border-gray-200 bg-white px-3 py-3">
        <div className="flex items-center gap-3">
          <div
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-brand-200 to-brand-400 text-lg font-bold text-white"
            aria-label="项目缩略图"
          >
            🎬
          </div>
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-semibold text-gray-800">{projectName}</div>
            <div className="flex items-center gap-2 text-[10px] text-gray-500">
              <span>进度 {progress}%</span>
              <span>·</span>
              <span>{sessionCredits} 学币</span>
            </div>
          </div>
        </div>
        {/* 进度条 */}
        <div className="mt-2 h-1 w-full overflow-hidden rounded-full bg-gray-100">
          <div
            className="h-full bg-gradient-to-r from-brand-400 to-brand-600 transition-all"
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>

      {/* 故事状态网格 */}
      <div className="flex-1 overflow-auto px-3 py-3">
        <div className="mb-2 flex items-center justify-between">
          <h3 className="text-xs font-semibold uppercase tracking-wide text-gray-500">
            🎯 故事状态
          </h3>
          <button
            type="button"
            onClick={() => setAdvancedOpen((v) => !v)}
            className="rounded-md px-2 py-0.5 text-[10px] text-gray-400 hover:bg-gray-100 hover:text-gray-600"
          >
            {advancedOpen ? '收起高级' : '⚙️ 高级'}
          </button>
        </div>
        <StoryStateGrid />

        {advancedOpen && (
          <div className="mt-4 space-y-3 border-t border-gray-200 pt-3">
            {/* 多版本切换 */}
            <div>
              <h4 className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-gray-500">
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
                        ? 'bg-brand-100 text-brand-800'
                        : 'bg-white text-gray-600 hover:bg-gray-50',
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
                  className="w-full rounded-md border border-dashed border-gray-300 px-2 py-1 text-xs text-gray-500 hover:border-brand-300 hover:text-brand-600"
                >
                  ＋ 保存当前为版本
                </button>
              </div>
            </div>

            {/* JSON 视图占位 — B3 阶段实装 */}
            <div>
              <h4 className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-gray-500">
                🔍 原始数据 (JSON)
              </h4>
              <pre className="max-h-48 overflow-auto rounded-md bg-gray-900 p-2 text-[10px] text-gray-100">
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