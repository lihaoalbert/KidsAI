// W10 Day 4 — Skill Card (MarketplacePage 单卡)
//
// 状态:
//   - 未安装:  [安装] 按钮 (需 PIN)
//   - 已装启用:  [禁用] [卸载] 按钮
//   - 已装禁用:  [启用] [卸载] 按钮
//   - 装/卸/切中: spinner

import { useState } from 'react';
import type { MarketplaceSkill } from '../../api/tauri';
import { useSkillStore } from '../../stores/skillStore';
import { ParentPinDialog } from '../system/ParentPinDialog';

interface SkillCardProps {
  skill: MarketplaceSkill;
}

const AUDIENCE_LABEL: Record<string, { text: string; color: string }> = {
  child: { text: '儿童', color: 'bg-success-soft text-success' },
  adult: { text: '成人', color: 'bg-accent-soft text-accent-ink' },
  both: { text: '通用', color: 'bg-accent-soft-2 text-accent-ink' },
};

export function SkillCard({ skill }: SkillCardProps) {
  const busy = useSkillStore((s) => s.busy);
  const install = useSkillStore((s) => s.install);
  const uninstall = useSkillStore((s) => s.uninstall);
  const toggle = useSkillStore((s) => s.toggle);
  const [pinOpen, setPinOpen] = useState(false);
  const [action, setAction] = useState<'install' | null>(null);

  const isBusy = busy === skill.id;
  const aud = AUDIENCE_LABEL[skill.audience] ?? AUDIENCE_LABEL.both;
  const sizeMb = (skill.sizeBytes / 1024 / 1024).toFixed(1);

  const handleInstallClick = () => {
    setAction('install');
    setPinOpen(true);
  };

  const handlePinSuccess = async (pin: string) => {
    setPinOpen(false);
    try {
      if (action === 'install') {
        await install(skill.id, pin);
      }
    } catch {
      // error 在 store 里
    }
    setAction(null);
  };

  const handleUninstall = async () => {
    try {
      await uninstall(skill.id);
    } catch {
      // error 在 store 里
    }
  };

  const handleToggle = async () => {
    try {
      await toggle(skill.id, !skill.enabled);
    } catch {
      // error 在 store 里
    }
  };

  return (
    <div
      className="border border-line rounded-xl p-4 bg-surface shadow-sm hover:shadow-md transition-shadow"
      data-testid={`skill-card-${skill.id}`}
    >
      <div className="flex items-start justify-between gap-3 mb-2">
        <div className="flex-1 min-w-0">
          <h3 className="font-semibold text-ink-2 truncate">
            {skill.name || skill.id}
          </h3>
          <p className="text-xs text-ink-2 mt-0.5">
            {skill.id} · v{skill.version}
          </p>
        </div>
        <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${aud.color}`}>
          {aud.text}
        </span>
      </div>

      {skill.description && (
        <p className="text-sm text-ink-2 line-clamp-2 mb-3">
          {skill.description}
        </p>
      )}

      <div className="flex flex-wrap gap-2 text-xs text-ink-2 mb-3">
        <span>📦 {sizeMb} MB</span>
        {skill.creditsPerUse > 0 && (
          <span>💎 {skill.creditsPerUse} 学币/次</span>
        )}
        {skill.dailyQuota > 0 && (
          <span>📊 {skill.dailyQuota}/天</span>
        )}
        {skill.ageTier.length > 0 && (
          <span>👶 {skill.ageTier.join('-')}</span>
        )}
      </div>

      <div className="flex gap-2">
        {skill.installed ? (
          <>
            <button
              type="button"
              className={`flex-1 h-8 rounded-lg text-sm font-medium ${
                skill.enabled
                  ? 'bg-surface-2 text-ink-2 hover:bg-surface-2'
                  : 'bg-success text-bg hover:bg-success/90'
              } disabled:opacity-50`}
              onClick={handleToggle}
              disabled={isBusy}
            >
              {isBusy ? '处理中…' : skill.enabled ? '禁用' : '启用'}
            </button>
            <button
              type="button"
              className="flex-1 h-8 rounded-lg bg-danger-soft text-danger hover:bg-danger-soft text-sm font-medium disabled:opacity-50"
              onClick={handleUninstall}
              disabled={isBusy}
              data-testid={`skill-uninstall-${skill.id}`}
            >
              卸载
            </button>
          </>
        ) : (
          <button
            type="button"
            className="w-full h-8 rounded-lg bg-accent text-bg hover:bg-accent-hover text-sm font-medium disabled:opacity-50"
            onClick={handleInstallClick}
            disabled={isBusy}
            data-testid={`skill-install-${skill.id}`}
          >
            {isBusy ? '安装中…' : '安装'}
          </button>
        )}
      </div>

      {pinOpen && (
        <ParentPinDialog
          open={true}
          mode="verify"
          title={`安装「${skill.name || skill.id}」`}
          hint="输入家长 PIN 授权安装 (含 manifest 验签 + 逐文件 sha256 校验)"
          onSuccess={handlePinSuccess}
          onCancel={() => {
            setPinOpen(false);
            setAction(null);
          }}
        />
      )}
    </div>
  );
}