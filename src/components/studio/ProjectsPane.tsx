import { useEffect, useState } from 'react';
import { listCreations, type CreationWithAssets } from '../../api/tauri';

type ProjectType = 'video' | 'game' | 'agent';

const TYPES: { key: ProjectType; emoji: string; label: string; ready: boolean }[] = [
  { key: 'video', emoji: '🎬', label: '视频', ready: true },
  { key: 'game', emoji: '🎮', label: '游戏', ready: false },
  { key: 'agent', emoji: '🤖', label: 'Agent', ready: false },
];

interface ProjectsPaneProps {
  onNewVideo: () => void;
}

export default function ProjectsPane({ onNewVideo }: ProjectsPaneProps) {
  const [type, setType] = useState<ProjectType>('video');
  const [works, setWorks] = useState<CreationWithAssets[]>([]);

  useEffect(() => {
    listCreations()
      .then(setWorks)
      .catch(() => setWorks([]));
  }, []);

  return (
    <div className="flex h-full flex-col bg-white">
      <div className="px-4 pt-4">
        <div className="text-sm font-bold text-gray-800">我的作品</div>
      </div>

      {/* 三类型 tab */}
      <div className="grid grid-cols-3 gap-1.5 px-3 py-3">
        {TYPES.map((t) => (
          <button
            key={t.key}
            onClick={() => t.ready && setType(t.key)}
            className={[
              'flex flex-col items-center gap-1 rounded-xl py-2.5 text-xs font-semibold transition-colors',
              type === t.key
                ? 'bg-brand-500 text-white'
                : t.ready
                  ? 'bg-brand-50 text-brand-700 hover:bg-brand-100'
                  : 'bg-gray-50 text-gray-300',
            ].join(' ')}
          >
            <span className="text-xl">{t.emoji}</span>
            <span>{t.label}</span>
          </button>
        ))}
      </div>

      {/* 作品墙 */}
      <div className="flex-1 overflow-auto px-3">
        {type === 'video' ? (
          works.length > 0 ? (
            <div className="space-y-2">
              {works.map((w) => (
                <div
                  key={w.id}
                  className="flex items-center gap-2 rounded-xl border border-gray-100 bg-gray-50 p-2"
                >
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-brand-100 text-lg">
                    🎬
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-xs font-semibold text-gray-800">
                      {w.userInput || '未命名作品'}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="mt-8 text-center text-xs text-gray-400">
              还没有作品，<br />开始你的第一部电影吧！
            </div>
          )
        ) : (
          <div className="mt-8 text-center text-xs text-gray-400">
            {TYPES.find((t) => t.key === type)?.label} 项目<br />即将开放 ✨
          </div>
        )}
      </div>

      {/* 新建 */}
      <div className="border-t border-gray-100 p-3">
        <button
          onClick={onNewVideo}
          className="w-full rounded-2xl bg-gradient-to-br from-brand-500 to-warm-500 px-4 py-3 text-sm font-bold text-white shadow-sm hover:opacity-95"
        >
          ➕ 开始新创作
        </button>
      </div>
    </div>
  );
}
