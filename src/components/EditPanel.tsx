// W3.5 指哪打哪画布交互 — 右侧抽屉组件
// 渲染：原图缩略 + 坐标标注 + 修改意图输入 + Submit/Cancel

import { useEffect, useRef, useState } from 'react';
import type { AgentAsset } from '../api/tauri';

export interface EditPanelProps {
  asset: AgentAsset;
  /// 归一化坐标 0-1
  clickX: number;
  clickY: number;
  /// true 表示 Agent 正在跑，提交按钮 disabled
  disabled?: boolean;
  onSubmit: (prompt: string) => void;
  onCancel: () => void;
}

export function EditPanel({
  asset,
  clickX,
  clickY,
  disabled = false,
  onSubmit,
  onCancel,
}: EditPanelProps) {
  const [text, setText] = useState('');
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // 打开时自动 focus 输入框
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // ESC 关闭
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [onCancel]);

  const handleSubmit = () => {
    const trimmed = text.trim();
    if (!trimmed || disabled) return;
    onSubmit(trimmed);
  };

  const xPercent = Math.round(clickX * 100);
  const yPercent = Math.round(clickY * 100);

  return (
    <>
      {/* 半透明 backdrop — 点击关闭 */}
      <div
        className="fixed inset-0 bg-ink/30 z-40"
        onClick={onCancel}
        aria-label="关闭编辑面板"
      />

      {/* 右侧抽屉 */}
      <div
        className="fixed right-0 top-0 bottom-0 w-[360px] bg-surface shadow-2xl z-50 flex flex-col"
        role="dialog"
        aria-label="指哪打哪 — 修改这块图"
      >
        {/* 头部 */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-line">
          <h3 className="font-semibold text-ink text-sm">✏️ 指哪打哪</h3>
          <button
            type="button"
            onClick={onCancel}
            className="text-ink-3 hover:text-ink-2 text-lg leading-none"
            aria-label="关闭"
          >
            ×
          </button>
        </div>

        {/* 原图缩略 + 点击位置标记 */}
        <div className="px-4 py-3 border-b border-line">
          <div className="relative w-full aspect-video bg-surface-2 rounded overflow-hidden">
            {asset.type === 'image' && (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={asset.thumbnailUrl ?? asset.url}
                alt={asset.prompt}
                className="w-full h-full object-cover"
              />
            )}
            {asset.type === 'video' && (
              <video
                src={asset.url}
                poster={asset.thumbnailUrl}
                className="w-full h-full object-cover"
                muted
              />
            )}
            {/* 红色十字标记点击位置 */}
            <div
              className="absolute pointer-events-none"
              style={{
                left: `${xPercent}%`,
                top: `${yPercent}%`,
                transform: 'translate(-50%, -50%)',
              }}
            >
              <div className="w-5 h-5 rounded-full border-2 border-danger bg-danger/30 animate-pulse" />
            </div>
          </div>
          <div className="mt-2 text-xs text-ink-2">
            📍 你点的位置：<span className="font-mono">{xPercent}%</span> ·{' '}
            <span className="font-mono">{yPercent}%</span>
          </div>
          {asset.prompt && (
            <div className="mt-1 text-xs text-ink-3 truncate">
              原 prompt：{asset.prompt}
            </div>
          )}
        </div>

        {/* 输入区 */}
        <div className="flex-1 flex flex-col px-4 py-3 min-h-0">
          <label className="text-xs font-medium text-ink-2 mb-1">
            想怎么改这块？
          </label>
          <textarea
            ref={inputRef}
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="例如：把这条裙子改成蓝色 / 加一顶帽子 / 表情更开心一点…"
            className="flex-1 text-sm border border-line rounded-md p-2 resize-none focus:outline-none focus:border-accent min-h-[80px]"
            disabled={disabled}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                handleSubmit();
              }
            }}
          />
          <div className="mt-1 text-[11px] text-ink-3">
            提示：按 ⌘+Enter 提交
          </div>
        </div>

        {/* 操作按钮 */}
        <div className="flex gap-2 px-4 py-3 border-t border-line">
          <button
            type="button"
            onClick={onCancel}
            className="flex-1 px-3 py-2 text-sm text-ink-2 bg-surface-2 rounded-md hover:bg-surface-2"
            disabled={disabled}
          >
            取消
          </button>
          <button
            type="button"
            onClick={handleSubmit}
            className="flex-1 px-3 py-2 text-sm text-bg bg-accent rounded-md hover:bg-accent-hover disabled:bg-surface-2 disabled:cursor-not-allowed"
            disabled={!text.trim() || disabled}
          >
            {disabled ? '生成中…' : '🚀 生成'}
          </button>
        </div>
      </div>
    </>
  );
}

export default EditPanel;