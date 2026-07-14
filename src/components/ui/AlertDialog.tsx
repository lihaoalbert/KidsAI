// P0 fix: 轻量结果提示 — 替代 window.alert()

import { useEffect } from 'react';

interface AlertDialogProps {
  open: boolean;
  title?: string;
  message: string;
  onClose: () => void;
}

export default function AlertDialog({ open, title, message, onClose }: AlertDialogProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' || e.key === 'Enter') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 px-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-sm rounded-xl bg-surface p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        {title && <div className="mb-2 text-base font-semibold text-ink">{title}</div>}
        <div className="mb-4 text-sm text-ink-2 whitespace-pre-line">{message}</div>
        <div className="flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md bg-accent px-4 py-1.5 text-sm font-semibold text-bg hover:bg-accent-hover"
          >
            好
          </button>
        </div>
      </div>
    </div>
  );
}
