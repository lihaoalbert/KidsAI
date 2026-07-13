import type { PageKey } from '../App';
import { useTokenStore } from '../stores/tokenStore';
import ProjectsPane from './studio/ProjectsPane';

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
  { key: 'workshop', label: '作品工坊', icon: '🎨' },
  { key: 'library', label: '作品库', icon: '📚' },
  { key: 'agent', label: '我的 Agent', icon: '🤖' },
];

export default function Sidebar({ currentPage, onNavigate }: SidebarProps) {
  const balance = useTokenStore((s) => s.balance);
  // W8 反馈: 左侧只能一屏。在 Studio 页面用 ProjectsPane 顶替全局导航,
  // 保留 Logo + 学币角标, 顶部加"← 课程中心"回首页, 避免与课程中心侧栏视觉重复.
  if (currentPage === 'studio') {
    return (
      <aside className="w-60 shrink-0 border-r border-gray-200 bg-white">
        <ProjectsPane onBackHome={() => onNavigate('home')} />
        <div className="border-t border-gray-100 px-3 py-3">
          <div className="rounded-md bg-gradient-to-br from-warm-50 to-brand-50 px-3 py-2">
            <div className="text-[10px] text-gray-500">学币余额</div>
            <div className="text-base font-bold text-brand-700">💎 {balance}</div>
          </div>
        </div>
      </aside>
    );
  }

  return (
    <aside className="w-60 bg-white border-r border-gray-200 flex flex-col">
      {/* Logo */}
      <div className="px-6 py-5 border-b border-gray-200">
        <div className="flex items-center gap-2">
          <div className="w-9 h-9 rounded-lg bg-gradient-to-br from-brand-500 to-warm-500 flex items-center justify-center text-white text-lg font-bold">
            K
          </div>
          <div>
            <div className="font-semibold text-sm text-gray-900">KidsAI</div>
            <div className="text-xs text-gray-500">Studio</div>
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
                  ? 'bg-brand-50 text-brand-700'
                  : 'text-gray-700 hover:bg-gray-50',
              ].join(' ')}
            >
              <span className="text-base">{item.icon}</span>
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      {/* 底部 Token 余额 */}
      <div className="px-3 py-3 border-t border-gray-200">
        <div className="bg-gradient-to-br from-warm-50 to-brand-50 rounded-md px-3 py-2.5">
          <div className="text-xs text-gray-600">学币余额</div>
          <div className="text-lg font-bold text-brand-700 mt-0.5">
            💎 {balance}
          </div>
        </div>
      </div>
    </aside>
  );
}
