// W9: 全局 CostBanner — 监听 directorStore.pendingConfirmation, 弹代价确认
// agent 或 UI 写入 pending → 这里弹层 → 用户确认 → 执行回调

import { useDirectorStore } from '../../stores/directorStore';
import type { PendingConfirmation } from '../../stores/directorStore';
import { formatCost, formatInvalidates } from '../../stores/costModel';

export default function PendingConfirmationBanner() {
  const pending = useDirectorStore((s) => s.pendingConfirmation);
  const cancelPending = useDirectorStore((s) => s.cancelPending);
  const confirmPending = useDirectorStore((s) => s.confirmPending);

  if (!pending) return null;

  return (
    <PendingDialog
      pending={pending}
      onCancel={cancelPending}
      onConfirm={confirmPending}
    />
  );
}

function PendingDialog({
  pending,
  onCancel,
  onConfirm,
}: {
  pending: PendingConfirmation;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const labels = formatInvalidates(pending.invalidates);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-96 rounded-2xl bg-white p-5 shadow-2xl">
        <div className="mb-3 flex items-center gap-2">
          <span className="text-2xl">⚠️</span>
          <h3 className="text-base font-semibold text-gray-800">{pending.description}</h3>
        </div>
        <p className="mb-3 text-sm text-gray-700">{pending.rationale}</p>
        {labels.length > 0 && (
          <div className="mb-3 space-y-1 rounded-lg bg-gray-50 p-3">
            <p className="text-xs font-semibold text-gray-600">下游影响:</p>
            <ul className="space-y-0.5">
              {labels.map((l, i) => (
                <li key={i} className="text-xs text-gray-700">• {l}</li>
              ))}
            </ul>
          </div>
        )}
        <div className="mb-4 rounded-lg bg-amber-50 px-3 py-2 text-sm font-semibold text-amber-800">
          {formatCost({ invalidates: pending.invalidates, credits: pending.credits, seconds: pending.seconds, requiresConfirm: pending.credits > 5, rationale: pending.rationale })}
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