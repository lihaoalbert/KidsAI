import { useState, type ReactNode } from 'react';

interface StudioLayoutProps {
  left?: ReactNode;
  center: ReactNode;
  right: ReactNode;
}

export default function StudioLayout({ left, center, right }: StudioLayoutProps) {
  const [rightOpen, setRightOpen] = useState(true);
  const [leftOpen, setLeftOpen] = useState(true);

  return (
    <div className="flex h-full w-full overflow-hidden">
      {leftOpen && left ? (
        <div className="relative w-[280px] shrink-0">
          <button
            onClick={() => setLeftOpen(false)}
            title="收起左栏"
            className="absolute -right-3 top-4 z-10 flex h-6 w-6 items-center justify-center rounded-full border border-line bg-surface text-xs text-ink-2 shadow-sm hover:bg-surface-2"
          >
            ‹
          </button>
          {left}
        </div>
      ) : left ? (
        <button
          onClick={() => setLeftOpen(true)}
          className="flex w-10 shrink-0 flex-col items-center gap-2 border-r border-line bg-surface py-4 hover:bg-surface-2"
        >
          <span className="text-ink-3">›</span>
          <span className="text-lg">📖</span>
        </button>
      ) : null}

      <div className="min-w-0 flex-1">{center}</div>

      {rightOpen ? (
        <div className="relative w-[380px] shrink-0 border-l border-line">
          <button
            onClick={() => setRightOpen(false)}
            title="收起"
            className="absolute -left-3 top-4 z-10 flex h-6 w-6 items-center justify-center rounded-full border-line border bg-surface text-xs text-ink-2 shadow-sm hover:bg-surface-2"
          >
            ›
          </button>
          {right}
        </div>
      ) : (
        <button
          onClick={() => setRightOpen(true)}
          className="flex w-12 shrink-0 flex-col items-center gap-2 border-l border-line bg-surface py-4 hover:bg-surface-2"
        >
          <span className="text-ink-3">‹</span>
          <span className="text-lg">🎥</span>
        </button>
      )}
    </div>
  );
}
