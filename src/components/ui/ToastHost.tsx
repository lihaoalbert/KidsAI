// P1-2: 全局 toast 渲染层. 挂在 App 根, 右上角堆叠展示 + 自动消失.

import { useToastStore } from '../../stores/toastStore';

const LEVEL_STYLE: Record<string, string> = {
  info: 'bg-gray-800 text-white',
  success: 'bg-emerald-600 text-white',
  warn: 'bg-amber-500 text-white',
  error: 'bg-rose-600 text-white',
};

const LEVEL_GLYPH: Record<string, string> = {
  info: 'ℹ',
  success: '✓',
  warn: '⚠',
  error: '✕',
};

export default function ToastHost() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  return (
    <div
      className="pointer-events-none fixed top-4 right-4 z-50 flex flex-col gap-2 max-w-sm"
      role="status"
      aria-live="polite"
    >
      {toasts.map((t) => (
        <button
          key={t.id}
          type="button"
          onClick={() => dismiss(t.id)}
          className={`pointer-events-auto rounded-xl px-4 py-2.5 text-sm shadow-lg shadow-black/10 text-left animate-[fadeIn_120ms_ease-out] ${LEVEL_STYLE[t.level] ?? LEVEL_STYLE.info}`}
        >
          <span className="mr-2 font-bold">{LEVEL_GLYPH[t.level] ?? '•'}</span>
          {t.text}
        </button>
      ))}
    </div>
  );
}
