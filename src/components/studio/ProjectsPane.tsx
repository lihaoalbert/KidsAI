import { useEffect, useState } from 'react';
import { listCreations, type CreationWithAssets } from '../../api/tauri';

interface ProjectsPaneProps {
  onBackHome: () => void;
}

// 左屏去重 (W5 修复): 不再做"视频/游戏/Agent"tab + 学币栏 + 开始新创作按钮 ——
// 顶部全局 Sidebar 已经管了导航和学币, Studio 左屏只留极简的"作品历史 + 退出"。
export default function ProjectsPane({ onBackHome }: ProjectsPaneProps) {
  const [works, setWorks] = useState<CreationWithAssets[]>([]);

  useEffect(() => {
    listCreations()
      .then(setWorks)
      .catch(() => setWorks([]));
  }, []);

  return (
    <div className="flex h-full flex-col bg-white">
      <div className="flex items-center justify-between px-4 pt-4 pb-2">
        <div className="text-sm font-bold text-gray-800">我的作品</div>
        <button
          onClick={onBackHome}
          title="返回首页"
          className="flex h-7 w-7 items-center justify-center rounded-md text-gray-500 hover:bg-gray-100"
        >
          ✕
        </button>
      </div>

      <div className="flex-1 overflow-auto px-3 pb-3">
        {works.length > 0 ? (
          <div className="space-y-1.5">
            {works.map((w) => (
              <div
                key={w.id}
                className="flex items-center gap-2 rounded-lg border border-gray-100 bg-gray-50 p-2"
              >
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-brand-100 text-base">
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
          <div className="mt-6 text-center text-xs text-gray-400">
            还没有作品，
            <br />
            开始你的第一部电影吧！
          </div>
        )}
      </div>
    </div>
  );
}