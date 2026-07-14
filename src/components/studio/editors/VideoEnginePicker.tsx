// W6 C4 + E3: 视频引擎选择器
// 阶段5 (试拍) 前的弹窗, 让孩子选 Seedance (默认) 或 MiniMax hailuo-02 (备用).
// 设计原则: 大按钮 + 预设, 不让孩自己改 model 参数; hailuo 不向孩默认推荐,
// 仅当 Seedance 不可用或 lihao 测试时手动切.

import { useDirectorStore } from '../../../stores/directorStore';

const ENGINES = [
  {
    id: 'seedance' as const,
    name: '火山 Seedance 2.0',
    emoji: '🎬',
    costPerShot: 9,
    note: '默认 · 稳',
    detail: '480P 4s 单镜试拍',
  },
  {
    id: 'hailuo' as const,
    name: 'MiniMax hailuo-02',
    emoji: '✨',
    costPerShot: 12,
    note: '备用',
    detail: '套餐额度 · 默认隐藏',
  },
];

export default function VideoEnginePicker({ onClose }: { onClose: () => void }) {
  const videoEngine = useDirectorStore((s) => s.videoEngine);
  const setVideoEngine = useDirectorStore((s) => s.setVideoEngine);

  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-2xl bg-surface p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="mb-1 text-base font-semibold text-ink-2">🎥 选视频引擎</h3>
        <p className="mb-4 text-xs text-ink-2">
          默认走火山 Seedance; MiniMax hailuo 是备用选项 (套餐里的额度, 不推荐给孩用).
        </p>

        <div className="grid grid-cols-2 gap-3">
          {ENGINES.map((e) => {
            const active = videoEngine === e.id;
            const isBackup = e.id === 'hailuo';
            return (
              <button
                key={e.id}
                type="button"
                onClick={() => setVideoEngine(e.id)}
                className={`flex flex-col items-start gap-1 rounded-xl border-2 p-3 text-left transition-all ${
                  active
                    ? 'border-accent bg-accent-soft'
                    : 'border-line bg-surface hover:border-accent-line'
                }`}
              >
                <div className="flex w-full items-center justify-between">
                  <span className="text-xl">{e.emoji}</span>
                  <span
                    className={`rounded-full px-1.5 py-0.5 text-[9px] ${
                      isBackup
                        ? 'bg-warning-soft text-warning'
                        : 'bg-success-soft text-success'
                    }`}
                  >
                    {e.note}
                  </span>
                </div>
                <div className="text-sm font-medium text-ink-2">{e.name}</div>
                <div className="text-[10px] text-ink-2">{e.detail}</div>
                <div className="mt-1 text-[10px] font-medium text-accent-ink">
                  {e.costPerShot} 学币 / 镜
                </div>
              </button>
            );
          })}
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-xl bg-accent px-4 py-1.5 text-sm font-medium text-bg hover:bg-accent"
          >
            选好啦 →
          </button>
        </div>
      </div>
    </div>
  );
}