import { useState, type ReactNode } from 'react';

interface StudioLayoutProps {
  left: ReactNode;
  center: ReactNode;
  right: ReactNode;
}

const TYPE_EMOJI = ['🎬', '🎮', '🤖'];

export default function StudioLayout({ left, center, right }: StudioLayoutProps) {
  const [leftOpen, setLeftOpen] = useState(true);
  const [rightOpen, setRightOpen] = useState(true);

  return (
    <div className="flex h-full w-full overflow-hidden">
      {/* 左：作品墙（可收窄成竖条） */}
      {leftOpen ? (
        <div className="relative w-56 shrink-0 border-r border-gray-200">
          {left}
          <button
            onClick={() => setLeftOpen(false)}
            title="收起"
            className="absolute -right-3 top-4 z-10 flex h-6 w-6 items-center justify-center rounded-full border border-gray-200 bg-white text-xs text-gray-500 shadow-sm hover:bg-gray-50"
          >
            ‹
          </button>
        </div>
      ) : (
        <button
          onClick={() => setLeftOpen(true)}
          className="flex w-12 shrink-0 flex-col items-center gap-3 border-r border-gray-200 bg-white py-4 text-lg hover:bg-gray-50"
        >
          <span className="text-gray-400">›</span>
          {TYPE_EMOJI.map((e) => (
            <span key={e}>{e}</span>
          ))}
        </button>
      )}

      {/* 中：对话流（主角，最大） */}
      <div className="min-w-0 flex-1">{center}</div>

      {/* 右：结果预览（可折叠） */}
      {rightOpen ? (
        <div className="relative w-[380px] shrink-0 border-l border-gray-200">
          <button
            onClick={() => setRightOpen(false)}
            title="收起"
            className="absolute -left-3 top-4 z-10 flex h-6 w-6 items-center justify-center rounded-full border border-gray-200 bg-white text-xs text-gray-500 shadow-sm hover:bg-gray-50"
          >
            ›
          </button>
          {right}
        </div>
      ) : (
        <button
          onClick={() => setRightOpen(true)}
          className="flex w-12 shrink-0 flex-col items-center gap-2 border-l border-gray-200 bg-white py-4 hover:bg-gray-50"
        >
          <span className="text-gray-400">‹</span>
          <span className="text-lg">🎥</span>
        </button>
      )}
    </div>
  );
}
