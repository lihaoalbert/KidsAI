// W3.7+ FrameSelector — 在已抽好的帧里选 1 帧(L6)或一键整段(L7)复刻
// UI 区别:
//   - single: 单选高亮 + 「🚀 复刻这一帧」按钮,必须选 1 帧才能点
//   - batch:  无单选 + 进度条 + 「▶ 整段复刻 (N 帧)」按钮
// 角色/风格 picker 由上层页提供,这里只透传状态 + 复用 props

import { useState } from 'react';
import Button from './Button';
import type { ExtractedFrame } from '../utils/frameExtractor';
import type { ReferenceRecreateMode } from '../../shared/types/level';

export interface FrameSelectorProps {
  levelId: string;
  frames: ExtractedFrame[];
  mode: ReferenceRecreateMode;
  /// 'batch' 模式显示的进度
  progress?: { done: number; total: number };
  isRunning: boolean;
  /// 关卡原始 system_prompt — 会在此基础上拼 [Reference context]
  systemPrompt: string;
  tools: string[];
  characterId?: string;
  styleId?: string;
  /// 触发复刻;single 模式参数里只 1 帧,batch 模式传全部
  onRun: (params: {
    frames: ExtractedFrame[];
    systemPrompt: string;
    tools: string[];
    characterId?: string;
    styleId?: string;
  }) => void;
}

export function FrameSelector({
  frames,
  mode,
  progress,
  isRunning,
  systemPrompt,
  tools,
  characterId,
  styleId,
  onRun,
}: FrameSelectorProps) {
  const [selectedId, setSelectedId] = useState<string | null>(
    mode === 'single' ? (frames[0]?.id ?? null) : null,
  );

  const handleRun = () => {
    let toRun: ExtractedFrame[];
    if (mode === 'single') {
      const f = frames.find((x) => x.id === selectedId) ?? frames[0];
      if (!f) return;
      toRun = [f];
    } else {
      toRun = frames;
    }
    onRun({
      frames: toRun,
      systemPrompt,
      tools,
      characterId,
      styleId,
    });
  };

  const canRunSingle = mode === 'single' && !!selectedId;

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold text-gray-900">
          {mode === 'single' ? '🎯 选 1 帧复刻' : '🎬 整段分镜复刻'}
        </h4>
        <span className="text-xs text-gray-500">
          {mode === 'single'
            ? '点 1 帧 — 只有它会被复刻'
            : `${frames.length} 帧都会按你的角色+风格统一复刻`}
        </span>
      </div>

      {/* 帧网格 + 选择 */}
      <div
        className={
          mode === 'single'
            ? 'grid grid-cols-3 sm:grid-cols-5 gap-2'
            : 'grid grid-cols-2 sm:grid-cols-3 gap-2'
        }
      >
        {frames.map((f, i) => {
          const isSelected = mode === 'single' && selectedId === f.id;
          return (
            <button
              key={f.id}
              type="button"
              disabled={isRunning}
              onClick={() => mode === 'single' && setSelectedId(f.id)}
              data-testid={`frame-tile-${i}`}
              className={[
                'relative overflow-hidden rounded-md border-2 transition-all',
                mode === 'single'
                  ? isSelected
                    ? 'border-brand-500 ring-2 ring-brand-300'
                    : 'border-gray-200 hover:border-brand-300'
                  : 'border-gray-200',
                isRunning ? 'opacity-60 cursor-not-allowed' : '',
              ].join(' ')}
            >
              {/* eslint-disable-next-line @next/next/no-img-element */}
              <img
                src={f.dataUrl}
                alt={`frame ${i + 1}`}
                className="w-full aspect-video object-cover bg-gray-100"
              />
              <div className="absolute bottom-0 left-0 right-0 bg-black/60 text-white text-[10px] px-1 py-0.5 text-center font-mono">
                # {i + 1} · {((f.timestampMs ?? 0) / 1000).toFixed(1)}s
              </div>
              {isSelected && (
                <div className="absolute top-1 right-1 bg-brand-600 text-white text-[10px] px-1.5 py-0.5 rounded">
                  ✓ 已选
                </div>
              )}
            </button>
          );
        })}
      </div>

      {/* 进度条 (batch 模式 + 运行中) */}
      {mode === 'batch' && isRunning && progress && (
        <div>
          <div className="flex items-center justify-between text-xs text-gray-600 mb-1">
            <span>复刻进度</span>
            <span className="font-mono">
              {progress.done} / {progress.total}
            </span>
          </div>
          <div className="h-2 bg-gray-100 rounded overflow-hidden">
            <div
              className="h-full bg-brand-500 transition-all"
              style={{ width: `${(progress.done / Math.max(progress.total, 1)) * 100}%` }}
            />
          </div>
        </div>
      )}

      {/* 操作按钮 */}
      <div className="flex items-center justify-between">
        <div className="text-xs text-gray-500">
          复刻时,系统会把当前帧的图像作为参考传给 AI,保留构图 + 迁移风格
        </div>
        <Button
          variant="primary"
          size="sm"
          onClick={handleRun}
          disabled={
            isRunning ||
            (mode === 'single' && !canRunSingle) ||
            frames.length === 0
          }
          data-testid="frame-run-button"
        >
          {isRunning
            ? `复刻中…${progress ? ` (${progress.done}/${progress.total})` : ''}`
            : mode === 'single'
            ? '🚀 复刻这一帧'
            : `▶ 整段复刻 (${frames.length} 帧)`}
        </Button>
      </div>
    </div>
  );
}

export default FrameSelector;
