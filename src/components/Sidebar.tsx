import type { PageKey } from '../App';
import { useTokenStore } from '../stores/tokenStore';
import { ModeBadge } from './system/ModeBadge';

interface SidebarProps {
  currentPage: PageKey;
  onNavigate: (page: PageKey) => void;
}

interface NavItem {
  key: PageKey;
  label: string;
  icon: string;
}

const navItems: NavItem[] = [
  { key: 'home', label: '课程中心', icon: '🏠' },
  { key: 'library', label: '作品库', icon: '📚' },
  { key: 'studio', label: '视频创作', icon: '🎬' },
  { key: 'agent', label: '我的 Agent', icon: '🤖' },
  { key: 'marketplace', label: 'Skill 市场', icon: '📦' },
];

export default function Sidebar({ currentPage, onNavigate }: SidebarProps) {
  const balance = useTokenStore((s) => s.balance);

  // P0 fix: Sidebar 永远显示完整导航, 不再在 studio 路由时替换为 ProjectsPane.
  // ProjectsPane 现在作为 StudioPage 内部组件渲染.
  return (
    <aside className="w-60 bg-surface border-r border-line flex flex-col">
      {/* Logo */}
      <div className="px-6 py-5 border-b border-line">
        <div className="flex items-center gap-2">
          <div className="w-9 h-9 rounded-lg bg-gradient-to-br from-accent to-highlight flex items-center justify-center text-bg text-lg font-bold">
            K
          </div>
          <div>
            <div className="font-semibold text-sm text-ink">KidsAI</div>
            <div className="text-xs text-ink-3">Studio</div>
          </div>
        </div>
      </div>

      {/* 导航 */}
      <nav className="flex-1 px-3 py-4 space-y-1">
        {navItems.map((item) => {
          const isActive = currentPage === item.key;
          return (
            <button
              key={item.key}
              onClick={() => onNavigate(item.key)}
              className={[
                'w-full flex items-center gap-3 px-3 py-2.5 rounded-md text-sm font-medium transition-colors',
                isActive
                  ? 'bg-accent-soft text-accent-ink'
                  : 'text-ink-2 hover:bg-surface-2 hover:text-ink',
              ].join(' ')}
            >
              <span className="text-base">{item.icon}</span>
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      {/* 底部 Token 余额 + Mode 徽章 */}
      <div className="px-3 py-3 border-t border-line space-y-2">
        <div className="bg-gradient-to-br from-highlight/20 to-accent-soft rounded-md px-3 py-2.5">
          <div className="text-xs text-ink-2">学币余额</div>
          <div className="text-lg font-bold text-accent-ink mt-0.5">
            💎 {balance}
          </div>
        </div>
        <div className="flex justify-center">
          <ModeBadge onNavigate={(p) => onNavigate(p)} />
        </div>
      </div>
    </aside>
  );
}