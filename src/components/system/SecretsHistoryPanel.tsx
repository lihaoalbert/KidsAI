// P1-5: Secrets 多版本回滚 UI
//
// 列出当前 profile 下所有已装 secret 版本 (含 history, 最多 3),
// 让家长可以主动回滚到上一个版本 (例如刚装的新版 prompt 把对话风格改坏了).
//
// 数据源:
//   - 当前版本: useSecretsStore.currentVersions
//   - 历史列表: listSecretVersions(profile) → 排除 current → 显示 [回滚到此版本] 按钮
//   - 触发回滚: useSecretsStore.rollback(profile, version)
//   - 错误反馈: useToastStore (任何 rollback 失败都立刻 toast)

import { useEffect, useState } from 'react';
import { listSecretVersions } from '../../api/tauri';
import { useSecretsStore } from '../../stores/secretsStore';
import { useToastStore } from '../../stores/toastStore';

interface Props {
  /** "child" 或 "adult" — 当前要展示历史的 profile */
  profile: 'child' | 'adult';
}

export default function SecretsHistoryPanel({ profile }: Props) {
  const currentVersions = useSecretsStore((s) => s.currentVersions);
  const rollback = useSecretsStore((s) => s.rollback);
  const refreshVersions = useSecretsStore((s) => s.refreshVersions);
  const [versions, setVersions] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    try {
      const v = await listSecretVersions(profile);
      setVersions(v);
    } catch (e) {
      // 列表拉不到 — 静默 (避免 SettingsPage 整页崩), 不影响其他 section
      setVersions([]);
    }
  };

  useEffect(() => {
    void refresh();
  }, [profile]); // reload when profile toggles

  const current = currentVersions[profile];
  // 回滚候选 = 全部历史里非当前的一个. 按 listVersions 返回顺序 (一般新→旧).
  const candidates = versions.filter((v) => v !== current);

  const handleRollback = async (toVersion: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await rollback(profile, toVersion);
      await refreshVersions();
      await refresh();
      useToastStore
        .getState()
        .push(`已回滚 ${profile} 到 ${toVersion}`, 'success');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      useToastStore.getState().push(`回滚失败: ${msg}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  if (!current) {
    return (
      <div className="text-xs text-ink-3 mt-2">
        {profile} 未安装 secret — 走源码 fallback
      </div>
    );
  }

  return (
    <div className="mt-2 space-y-1">
      <div className="text-xs text-ink-2">
        当前 <span className="font-mono">{current}</span>
        {candidates.length > 0 && (
          <span className="ml-2 text-ink-3">
            (历史 {candidates.length} 个)
          </span>
        )}
      </div>
      {candidates.length > 0 && (
        <div className="space-y-1">
          {candidates.map((v) => (
            <div
              key={v}
              className="flex items-center justify-between text-xs bg-surface-2 rounded px-2 py-1"
            >
              <span className="font-mono text-ink-2">{v}</span>
              <button
                type="button"
                disabled={busy}
                onClick={() => handleRollback(v)}
                className="text-rose-600 hover:text-rose-800 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                回滚到此
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
