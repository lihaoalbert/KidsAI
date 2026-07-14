// 顶栏组件 — 阶段2 共享组件库.
//
// DESIGN.md §5.3: "Top bar (h=56px) — page title (Forest: '我的故事') +
// breadcrumb + actions slot (反馈 / 帮助 / 通知). Persistent on every page."
//
// 用法:
//   <AppHeader
//     title="我的故事"
//     breadcrumb={['课程中心', 'L1 · 我的 AI 伙伴']}
//     actions={<Button size="sm">新建项目</Button>}
//   />
//
// 模式感知: 双模式自动通过 :root[data-mode] 翻转 — 颜色/border/radius 都走 token.

import type { ReactNode } from 'react';

interface AppHeaderProps {
  /** 主标题 — 当前页/项目名 */
  title: string;
  /** 面包屑 (可空) */
  breadcrumb?: string[];
  /** 右侧 actions slot — 按钮组/通知/帮助 */
  actions?: ReactNode;
  /** 自定义 className */
  className?: string;
}

export default function AppHeader({
  title,
  breadcrumb = [],
  actions,
  className = '',
}: AppHeaderProps) {
  return (
    <header
      className={[
        'h-14 shrink-0 flex items-center gap-4 px-6',
        'bg-surface border-b border-line',
        className,
      ].join(' ')}
      data-testid="app-header"
    >
      {/* 左: breadcrumb + title */}
      <div className="flex-1 min-w-0 flex items-center gap-2 text-sm">
        {breadcrumb.length > 0 && (
          <nav
            aria-label="breadcrumb"
            className="flex items-center gap-1.5 text-ink-3 text-meta min-w-0"
          >
            {breadcrumb.map((seg, i) => (
              <span key={i} className="flex items-center gap-1.5 min-w-0">
                {i > 0 && <span className="text-ink-3/50">/</span>}
                <span
                  className={
                    i === breadcrumb.length - 1
                      ? 'text-ink-2 font-medium'
                      : 'hover:text-ink-2 transition-colors'
                  }
                >
                  {seg}
                </span>
              </span>
            ))}
          </nav>
        )}
        {breadcrumb.length > 0 && (
          <span className="text-ink-3/50">·</span>
        )}
        <h1 className="font-semibold text-ink truncate">{title}</h1>
      </div>

      {/* 右: actions slot */}
      {actions && (
        <div className="flex items-center gap-2 shrink-0">{actions}</div>
      )}
    </header>
  );
}
