// W9: 分镜 tab — 每镜卡片 + 镜头语言 + 声音设计
// 渐进披露：默认镜头描述 + 缩略图；点 ⚙️ 展开镜头语言 6 维 + 声音设计 4 维

import { useState } from 'react';
import { useDirectorStore } from '../../../stores/directorStore';
import CinematographyPanel from '../CinematographyPanel';
import SoundDesignPanel from '../SoundDesignPanel';
import { estimateCost, formatCost, formatInvalidates } from '../../../stores/costModel';

export default function StoryboardTab() {
  const shots = useDirectorStore((s) => s.shots);
  const setShotCinematography = useDirectorStore((s) => s.setShotCinematography);
  const setShotSoundDesign = useDirectorStore((s) => s.setShotSoundDesign);
  const editShotPrompt = useDirectorStore((s) => s.editShotPrompt);
  const insertShot = useDirectorStore((s) => s.insertShot);
  const deleteShot = useDirectorStore((s) => s.deleteShot);
  const moveShot = useDirectorStore((s) => s.moveShot);
  const reRenderShot = useDirectorStore((s) => s.reRenderShot);
  const story = useDirectorStore((s) => s.story);

  const [expandedShotId, setExpandedShotId] = useState<string | null>(null);
  const [confirmAction, setConfirmAction] = useState<{ kind: 'rerender' | 'delete'; shotId: string } | null>(null);

  if (shots.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-center text-sm text-gray-500">
        还没有分镜 — 先在「对话」tab 里和小启聊聊，agent 会自动生成分镜
      </div>
    );
  }

  const confirmReRender = (shotId: string) => {
    const cost = estimateCost('shot.prompt', { story, shots, shotIndex: shots.findIndex((s) => s.id === shotId) });
    if (cost.requiresConfirm) {
      setConfirmAction({ kind: 'rerender', shotId });
    } else {
      void reRenderShot(shotId);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-gray-100 bg-white px-4 py-3">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-gray-700">🎬 分镜 ({shots.length} 镜)</h2>
          <button
            type="button"
            onClick={() => {
              const desc = prompt('新镜描述', '');
              if (desc && desc.trim()) insertShot(shots.length, { description: desc.trim(), motion: desc.trim() });
            }}
            className="rounded-md border border-gray-300 px-3 py-1 text-xs font-semibold text-gray-700 hover:bg-gray-50"
          >
            ＋ 新镜
          </button>
        </div>
      </div>

      <div className="flex-1 space-y-3 overflow-auto px-4 py-4">
        {shots.map((shot, idx) => {
          const expanded = expandedShotId === shot.id;
          return (
            <div key={shot.id} className="rounded-xl border border-gray-100 bg-white shadow-sm">
              {/* Header */}
              <div className="flex items-start gap-3 p-3">
                <div className="flex h-16 w-24 shrink-0 items-center justify-center rounded-lg bg-gray-100 text-xs text-gray-400">
                  {shot.previewUrl ? (
                    <video src={shot.previewUrl} className="h-full w-full rounded-lg object-cover" muted />
                  ) : (
                    <span>🎬 #{idx + 1}</span>
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="mb-1 flex items-center gap-1.5">
                    <span className="rounded bg-brand-100 px-1.5 py-0.5 text-[10px] font-semibold text-brand-700">{shot.beat}</span>
                    <span className="rounded bg-gray-100 px-1.5 py-0.5 text-[10px] text-gray-600">{shot.mood}</span>
                    <span className="rounded bg-gray-100 px-1.5 py-0.5 text-[10px] text-gray-600">{shot.camera}</span>
                  </div>
                  <p className="line-clamp-2 text-sm text-gray-700">{shot.description}</p>
                </div>
                <div className="flex shrink-0 flex-col gap-1">
                  <button
                    type="button"
                    onClick={() => setExpandedShotId(expanded ? null : shot.id)}
                    className="rounded px-2 py-0.5 text-xs text-gray-500 hover:bg-gray-100"
                    title="展开镜头语言 + 声音设计"
                  >
                    {expanded ? '▲' : '⚙️'}
                  </button>
                  <button
                    type="button"
                    onClick={() => moveShot(shot.id, 'up')}
                    disabled={idx === 0}
                    className="rounded px-2 py-0.5 text-xs text-gray-500 hover:bg-gray-100 disabled:opacity-30"
                    title="上移"
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    onClick={() => moveShot(shot.id, 'down')}
                    disabled={idx === shots.length - 1}
                    className="rounded px-2 py-0.5 text-xs text-gray-500 hover:bg-gray-100 disabled:opacity-30"
                    title="下移"
                  >
                    ↓
                  </button>
                </div>
              </div>

              {/* Actions */}
              <div className="flex gap-2 border-t border-gray-100 px-3 py-2">
                <button
                  type="button"
                  onClick={() => confirmReRender(shot.id)}
                  disabled={shot.previewing}
                  className="flex-1 rounded-md bg-brand-600 px-2 py-1 text-xs font-semibold text-white hover:bg-brand-700 disabled:opacity-50"
                >
                  {shot.previewing ? '⏳ 试拍中' : shot.previewUrl ? '🔄 重拍' : '▶ 试拍'}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    if (confirm(`删除第 ${idx + 1} 镜？`)) {
                      const cost = estimateCost('shot.delete', { story, shots, shotIndex: idx });
                      if (cost.requiresConfirm) {
                        setConfirmAction({ kind: 'delete', shotId: shot.id });
                      } else {
                        deleteShot(shot.id);
                      }
                    }
                  }}
                  className="rounded-md px-2 py-1 text-xs text-red-500 hover:bg-red-50"
                  title="删除"
                >
                  🗑️
                </button>
              </div>

              {/* Expanded: cinematography + sound */}
              {expanded && (
                <div className="space-y-3 border-t border-gray-100 bg-warm-50/40 p-3">
                  <CinematographyPanel
                    value={shot.cinematography}
                    onChange={(patch) => setShotCinematography(shot.id, patch)}
                  />
                  <SoundDesignPanel
                    value={shot.soundDesign}
                    onChange={(patch) => setShotSoundDesign(shot.id, patch)}
                  />
                  <div>
                    <h4 className="mb-1 text-xs font-semibold text-gray-600">📝 提示词</h4>
                    <textarea
                      defaultValue={shot.motion}
                      onBlur={(e) => {
                        if (e.target.value !== shot.motion) {
                          editShotPrompt(shot.id, e.target.value, e.target.value);
                        }
                      }}
                      className="h-20 w-full rounded-lg border border-gray-200 p-2 text-xs focus:border-brand-400 focus:outline-none"
                    />
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Cost confirm dialog */}
      {confirmAction && (
        <CostConfirmDialog
          kind={confirmAction.kind}
          shotIndex={shots.findIndex((s) => s.id === confirmAction.shotId)}
          onCancel={() => setConfirmAction(null)}
          onConfirm={() => {
            if (confirmAction.kind === 'rerender') void reRenderShot(confirmAction.shotId);
            if (confirmAction.kind === 'delete') deleteShot(confirmAction.shotId);
            setConfirmAction(null);
          }}
        />
      )}
    </div>
  );
}

function CostConfirmDialog({ kind, shotIndex, onCancel, onConfirm }: { kind: 'rerender' | 'delete'; shotIndex: number; onCancel: () => void; onConfirm: () => void }) {
  const story = useDirectorStore((s) => s.story);
  const shots = useDirectorStore((s) => s.shots);
  const costKind = kind === 'rerender' ? 'shot.prompt' : 'shot.delete';
  const cost = estimateCost(costKind, { story, shots, shotIndex });
  const labels = formatInvalidates(cost.invalidates);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-96 rounded-2xl bg-white p-5 shadow-2xl">
        <div className="mb-3 flex items-center gap-2">
          <span className="text-2xl">⚠️</span>
          <h3 className="text-base font-semibold text-gray-800">确认改动?</h3>
        </div>
        <p className="mb-3 text-sm text-gray-700">{cost.rationale}</p>
        <div className="mb-3 space-y-1 rounded-lg bg-gray-50 p-3">
          <p className="text-xs font-semibold text-gray-600">下游影响:</p>
          <ul className="space-y-0.5">
            {labels.map((l, i) => (
              <li key={i} className="text-xs text-gray-700">• {l}</li>
            ))}
          </ul>
        </div>
        <div className="mb-4 rounded-lg bg-amber-50 px-3 py-2 text-sm font-semibold text-amber-800">
          {formatCost(cost)}
        </div>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="rounded-md px-4 py-1.5 text-sm text-gray-600 hover:bg-gray-100"
          >
            取消
          </button>
          <button
            type="button"
            onClick={onConfirm}
            className="rounded-md bg-brand-600 px-4 py-1.5 text-sm font-semibold text-white hover:bg-brand-700"
          >
            继续
          </button>
        </div>
      </div>
    </div>
  );
}