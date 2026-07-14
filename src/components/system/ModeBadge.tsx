// W10 Day 4 — ModeBadge (顶栏常驻徽章)
//
// 设计:
//   - 永远可见 (娃和成人模式都常驻, 切回快捷)
//   - 当前是儿童模式显示 🧒 + "儿童模式"; 成人模式显示 🧑 + "成人模式"
//   - 点徽章弹下拉菜单: 切到另一模式 / Skill 市场 / 家长设置
//
// 与 App.tsx 集成: 父组件提供 onNavigate 回调 (避免引入 react-router-dom)

import { useEffect, useRef, useState } from 'react';
import { useUserModeStore } from '../../stores/userModeStore';
import { ModeSwitchDialog } from './ModeSwitchDialog';
import type { UserMode } from '../../api/tauri';

interface ModeBadgeProps {
  onNavigate?: (page: 'marketplace' | 'settings') => void;
}

export function ModeBadge({ onNavigate }: ModeBadgeProps = {}) {
  const mode = useUserModeStore((s) => s.mode);
  const loaded = useUserModeStore((s) => s.loaded);
  const load = useUserModeStore((s) => s.load);
  const [menuOpen, setMenuOpen] = useState(false);
  const [switchTo, setSwitchTo] = useState<UserMode | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  // 启动时 load
  useEffect(() => {
    if (!loaded) load();
  }, [loaded, load]);

  // 点击外部关闭
  useEffect(() => {
    if (!menuOpen) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [menuOpen]);

  const isAdult = mode === 'adult';
  const badgeText = isAdult ? '成人模式' : '儿童模式';
  const badgeEmoji = isAdult ? '🧑' : '🧒';
  // Mode-aware 自动通过 :root[data-mode] 翻转; 不写分支条件.
  const badgeColor =
    'bg-accent-soft text-accent-ink border border-accent-line hover:bg-accent-soft-2';

  if (!loaded) {
    return (
      <div className="px-3 py-1 rounded-full border border-line bg-surface-2 text-sm text-ink-3">
        ⋯
      </div>
    );
  }

  return (
    <>
      <div className="relative" ref={menuRef}>
        <button
          type="button"
          className={`flex items-center gap-1.5 px-3 py-1 rounded-full text-sm font-medium transition-colors ${badgeColor}`}
          onClick={() => setMenuOpen((o) => !o)}
          data-testid="mode-badge"
        >
          <span>{badgeEmoji}</span>
          <span>{badgeText}</span>
          <span className="text-xs">▾</span>
        </button>

        {menuOpen && (
          <div
            className="absolute right-0 top-full mt-1 w-48 bg-surface border border-line rounded-lg shadow-lg py-1 z-40"
            data-testid="mode-badge-menu"
          >
            <button
              type="button"
              className="w-full text-left px-3 py-2 text-sm text-ink-2 hover:bg-surface-2"
              onClick={() => {
                setMenuOpen(false);
                setSwitchTo(isAdult ? 'child' : 'adult');
              }}
              data-testid="mode-switch-trigger"
            >
              {isAdult ? '切到儿童模式' : '切到成人模式'}
            </button>
            {onNavigate && (
              <button
                type="button"
                className="w-full text-left px-3 py-2 text-sm text-ink-2 hover:bg-surface-2"
                onClick={() => {
                  setMenuOpen(false);
                  onNavigate('marketplace');
                }}
                data-testid="mode-skills-link"
              >
                Skill 市场
              </button>
            )}
            {onNavigate && (
              <button
                type="button"
                className="w-full text-left px-3 py-2 text-sm text-ink-2 hover:bg-surface-2"
                onClick={() => {
                  setMenuOpen(false);
                  onNavigate('settings');
                }}
              >
                家长设置
              </button>
            )}
          </div>
        )}
      </div>

      {switchTo && (
        <ModeSwitchDialog
          open={true}
          targetMode={switchTo}
          onClose={() => setSwitchTo(null)}
        />
      )}
    </>
  );
}