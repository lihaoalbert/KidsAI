// P0 fix: 轻量内联提示弹层 — 替代 window.prompt()
// 单行输入 + OK/Cancel, Esc 取消, Enter 确认.

import { useEffect, useState } from 'react';

interface PromptDialogProps {
  open: boolean;
  title: string;
  defaultValue?: string;
  placeholder?: string;
  hint?: string;
  onCancel: () => void;
  onConfirm: (value: string) => void;
}

export default function PromptDialog({
  open,
  title,
  defaultValue = '',
  placeholder,
  hint,
  onCancel,
  onConfirm,
}: PromptDialogProps) {
  const [value, setValue] = useState(defaultValue);

  useEffect(() => {
    if (open) setValue(defaultValue);
  }, [open, defaultValue]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 px-4"
      onClick={onCancel}
    >
      <div
        className="w-full max-w-md rounded-xl bg-surface p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-3 text-base font-semibold text-ink">{title}</div>
        <input
          autoFocus
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') onConfirm(value.trim());
            if (e.key === 'Escape') onCancel();
          }}
          placeholder={placeholder}
          className="w-full rounded-lg border border-line px-3 py-2 text-sm focus:border-accent focus:outline-none"
        />
        {hint && <div className="mt-2 text-xs text-ink-2">{hint}</div>}
        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="rounded-md border border-line bg-surface px-3 py-1.5 text-sm text-ink-2 hover:bg-surface-2"
          >
            取消
          </button>
          <button
            type="button"
            onClick={() => onConfirm(value.trim())}
            className="rounded-md bg-accent px-3 py-1.5 text-sm font-semibold text-bg hover:bg-accent-hover"
          >
            确定
          </button>
        </div>
      </div>
    </div>
  );
}
