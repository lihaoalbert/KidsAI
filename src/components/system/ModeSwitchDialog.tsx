// W10 Day 4 — Mode Switch Dialog (Part C)
//
// 弹层流程:
//   1. 显示当前 mode + 切到的目标 mode
//   2. 风险提示 (成人模式放宽安全过滤, 显示全部 skill)
//   3. 嵌 ParentPinDialog (verify mode) 验证 PIN
//   4. PIN 验证成功 → 调 userModeStore.switchTo(mode, pin)

import { useState } from 'react';
import { useUserModeStore } from '../../stores/userModeStore';
import type { UserMode } from '../../api/tauri';
import { ParentPinDialog } from './ParentPinDialog';

interface ModeSwitchDialogProps {
  open: boolean;
  targetMode: UserMode;
  onClose: () => void;
}

export function ModeSwitchDialog({
  open,
  targetMode,
  onClose,
}: ModeSwitchDialogProps) {
  const mode = useUserModeStore((s) => s.mode);
  const switchTo = useUserModeStore((s) => s.switchTo);
  const switching = useUserModeStore((s) => s.switching);
  const error = useUserModeStore((s) => s.error);
  const [pinStage, setPinStage] = useState<'overview' | 'pin'>('overview');

  if (!open) return null;

  const isAdult = targetMode === 'adult';
  const sameMode = mode === targetMode;

  const handleSwitchSuccess = async (pin: string) => {
    try {
      await switchTo(targetMode, pin);
      setPinStage('overview');
      onClose();
    } catch {
      // error 已写入 store, ParentPinDialog 内部已处理
    }
  };

  if (pinStage === 'pin') {
    return (
      <ParentPinDialog
        open={true}
        mode="verify"
        title={isAdult ? '解锁成人模式' : '回到儿童模式'}
        hint={
          isAdult
            ? '输入家长 PIN 切换到成人模式 (放宽安全过滤, 显示全部 skill)'
            : '输入家长 PIN 切回儿童模式 (重新启用安全过滤)'
        }
        onSuccess={handleSwitchSuccess}
        onCancel={() => setPinStage('overview')}
      />
    );
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      data-testid="mode-switch-dialog"
    >
      <div className="bg-surface rounded-2xl shadow-2xl p-6 w-[440px] max-w-[92vw] text-ink">
        <h2 className="text-xl font-semibold text-ink mb-2">
          切换到「{isAdult ? '成人模式' : '儿童模式'}」
        </h2>

        {sameMode ? (
          <p className="text-sm text-warning mb-4">
            当前已是「{isAdult ? '成人模式' : '儿童模式'}」, 无需切换
          </p>
        ) : isAdult ? (
          <div className="space-y-2 text-sm text-ink-2 mb-4">
            <p className="font-medium text-warning">成人模式说明</p>
            <ul className="list-disc list-inside space-y-1 text-ink-2">
              <li>安全词过滤大幅放宽 (仅拦截极端违法内容)</li>
              <li>显示全部 skill (包括成人专属商业广告 / 纪录片模板)</li>
              <li>学币配额放宽, 适合专业创作</li>
              <li>telemetry 默认仅上报元数据, 不含输入输出 hash (隐私优先)</li>
            </ul>
          </div>
        ) : (
          <div className="space-y-2 text-sm text-ink-2 mb-4">
            <p className="font-medium text-accent-ink">儿童模式说明</p>
            <ul className="list-disc list-inside space-y-1 text-ink-2">
              <li>恢复严格安全词过滤 (黑名单 + 白名单)</li>
              <li>只显示儿童 / 全年龄 skill (隐藏成人专属)</li>
              <li>学币配额按日限制</li>
              <li>telemetry 包含 input/output hash, 持续改进</li>
            </ul>
          </div>
        )}

        {error && (
          <p className="text-sm text-danger mb-3" data-testid="mode-switch-error">
            {error}
          </p>
        )}

        <div className="flex gap-2">
          <button
            type="button"
            className="flex-1 h-10 rounded-lg bg-surface-2 hover:bg-line text-ink-2"
            onClick={onClose}
            disabled={switching}
          >
            取消
          </button>
          <button
            type="button"
            className="flex-1 h-10 rounded-lg text-bg bg-accent hover:bg-accent-hover active:bg-accent-active disabled:opacity-50"
            onClick={() => setPinStage('pin')}
            disabled={sameMode || switching}
            data-testid="mode-switch-confirm"
          >
            {switching ? '切换中…' : '下一步: 输入 PIN'}
          </button>
        </div>
      </div>
    </div>
  );
}