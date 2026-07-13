import { useState, type ReactNode } from 'react';

interface StudioLayoutProps {
  center: ReactNode;
  right: ReactNode;
}

export default function StudioLayout({ center, right }: StudioLayoutProps) {
  const [rightOpen, setRightOpen] = useState(true);

  return (
    <div className="flex h-full w-full overflow-hidden">
      <div className="min-w-0 flex-1">{center}</div>

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
