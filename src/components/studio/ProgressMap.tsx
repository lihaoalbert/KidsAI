import { useDirectorStore } from '../../stores/directorStore';
import { useStudioStore } from '../../stores/studioStore';

const STAGES = ['点子', '主角', '画风', '分镜', '试拍', '定稿'] as const;
const EMOJI = ['💡', '🌟', '🎨', '🎞️', '🎬', '🏆'] as const;

// W5 修复 ③: 顶部 6 个胶囊变成"可回退导航"
// - ✓ (历史已决策 + 未 stale): 可点击, 跳回该阶段
// - ✓ 但 stale: 可点击, 跳回 + 提示"下游会重生成"
// - 当前 active: 高亮, 不可点(已经在)
// - ◯ (未到): 灰, 不可点
export default function ProgressMap() {
  const cursor = useDirectorStore((s) => s.cursor);
  const history = useDirectorStore((s) => s.history);
  const goBackTo = useDirectorStore((s) => s.goBackTo);
  const goBackToStep1 = useStudioStore((s) => s.goBackToStep1);
  const phase = useStudioStore((s) => s.phase);

  // 仅在 stage 1 完成 (story locked + plan ready) 后才允许点中间阶段 ——
  // 否则用户在阶段1 还没故事就跳到分镜, 会拿到空 shots.
  const allowBack = phase !== 'stage1';

  return (
    <div className="flex items-center gap-1 overflow-x-auto px-3 py-2">
      {STAGES.map((label, i) => {
        const step = (i + 1) as 1 | 2 | 3 | 4 | 5 | 6;
        const active = step === cursor;
        const entry = history.find((e) => e.stage === step);
        const done = !!entry && !entry.stale && step < cursor;
        const stale = !!entry && entry.stale;
        const reachable = allowBack && (done || stale) && !active;
        const className = [
          'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-semibold transition-colors',
          active
            ? 'bg-accent text-bg shadow-sm'
            : done
              ? 'bg-success-soft text-success hover:bg-success-soft cursor-pointer'
              : stale
                ? 'bg-warning-soft text-warning hover:bg-warning-soft cursor-pointer'
                : 'bg-surface-2 text-ink-3 cursor-not-allowed',
        ].join(' ');
        const emoji = done ? '✅' : stale ? '⚪' : EMOJI[i];
        const title = active
          ? '当前阶段'
          : stale
            ? '之前已生成, 但下游有新改动 — 点这里跳回去'
            : done
              ? '已完成, 点这里可回去改'
              : '还没到这一步';
        if (reachable) {
          return (
            <div key={label} className="flex items-center gap-1">
              <button
                onClick={() => {
                  goBackTo(step);
                  if (step === 1) goBackToStep1();
                }}
                className={className}
                title={title}
              >
                <span>{emoji}</span>
                <span className="hidden sm:inline">{label}</span>
              </button>
              {i < STAGES.length - 1 && (
                <span className={done ? 'text-success-soft' : 'text-warning-soft'}>–</span>
              )}
            </div>
          );
        }
        return (
          <div key={label} className="flex items-center gap-1">
            <div className={className} title={title}>
              <span>{emoji}</span>
              <span className="hidden sm:inline">{label}</span>
            </div>
            {i < STAGES.length - 1 && (
              <span className={done ? 'text-success-soft' : 'text-ink-3'}>–</span>
            )}
          </div>
        );
      })}
    </div>
  );
}