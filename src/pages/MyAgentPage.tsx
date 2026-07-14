// Day 15-16: MyAgentPage 重写为 Skill Marketplace Launcher
//
// 设计: 这不再是"高级功能占位页", 而是 KidsAI Agent 内核的"skill 启动面板".
//   - 顶部: 当前模式 (child/adult) + 用户问候
//   - 已启用的 skill (来自 SkillStore.installed.filter(enabled)) → 一键启动
//   - 推荐 skill (来自 SkillStore.available.filter(!installed)) → 跳去 marketplace
//   - 底部: 跳到完整 marketplace / settings 入口
//
// 红线 8 体现: L1-L7 已删除, 这里就是新的"首页" — 进入 skill 才有创作能力.
// 与 HomePage 的区别: HomePage 是"课程中心" (面向小月的关卡), 这里面向 14+ 用户和成人
//   "我的 Agent" = 我装了哪些 skill, 我能用 AI 干什么.

import { useEffect, useMemo } from 'react';
import { useSkillStore } from '../stores/skillStore';
import { useUserModeStore } from '../stores/userModeStore';
import Card from '../components/Card';
import AppHeader from '../components/layout/AppHeader';

type PageRoute =
  | 'home'
  | 'library'
  | 'agent'
  | 'level'
  | 'runner'
  | 'studio'
  | 'marketplace'
  | 'settings';

interface MyAgentPageProps {
  onNavigate?: (page: PageRoute) => void;
}

const SKILL_ICONS: Record<string, string> = {
  'video-director': '🎬',
  'eng-adventure': '🔤',
  'guofeng-shuimo': '🎨',
  'coding-starter': '💻',
  'commercial-ad-director': '📺',
  'doc-shortfilm': '🎥',
  'resume-reel': '💼',
};

const SKILL_LAUNCH_ROUTES: Record<string, PageRoute> = {
  'video-director': 'studio',
};

const SKILL_LAUNCH_LABELS: Record<string, string> = {
  'video-director': '进入 Studio',
  'eng-adventure': '开始冒险',
};

export default function MyAgentPage({ onNavigate }: MyAgentPageProps) {
  const installed = useSkillStore((s) => s.installed);
  const available = useSkillStore((s) => s.available);
  const loadingInstalled = useSkillStore((s) => s.loadingInstalled);
  const refreshAll = useSkillStore((s) => s.refreshAll);
  const mode = useUserModeStore((s) => s.mode);
  const isAdult = mode === 'adult';

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  const enabled = useMemo(
    () => installed.filter((s) => s.enabled),
    [installed],
  );
  const disabled = useMemo(
    () => installed.filter((s) => !s.enabled),
    [installed],
  );
  const notInstalled = useMemo(
    () =>
      available.filter(
        (s) => !installed.some((i) => i.id === s.id),
      ),
    [available, installed],
  );

  const launchable = enabled.filter((s) => SKILL_LAUNCH_ROUTES[s.id]);

  return (
    <div className="flex flex-col h-full bg-bg">
      <AppHeader
        title={isAdult ? 'My Agent' : '我的 Agent'}
        breadcrumb={[isAdult ? 'Home' : '课程中心', isAdult ? 'My Agent' : '我的 Agent']}
      />
      <div className="p-8 max-w-6xl mx-auto flex-1 overflow-y-auto w-full">
      {/* 顶部: 模式 + 问候 */}
      <header className="mb-8">
        <h1 className="text-2xl font-bold text-ink">🤖 我的 Agent</h1>
        <p className="text-sm text-ink-2 mt-1">
          当前模式:{' '}
          <span
            className={
              mode === 'adult' ? 'text-accent-ink' : 'text-success'
            }
          >
            {mode === 'adult' ? '🧑 成人模式' : '🧒 儿童模式'}
          </span>
          {' · '}
          {enabled.length > 0
            ? `${enabled.length} 个 skill 已就绪`
            : '还没有启用的 skill'}
        </p>
      </header>

      {/* 快速启动: 已启用且可路由的 skill */}
      {launchable.length > 0 && (
        <section className="mb-8">
          <h2 className="text-base font-semibold text-ink-2 mb-3">
            ⚡ 快速启动
          </h2>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {launchable.map((s) => {
              const route = SKILL_LAUNCH_ROUTES[s.id];
              const label = SKILL_LAUNCH_LABELS[s.id] ?? '启动';
              return (
                <Card
                  key={s.id}
                  variant="bordered"
                  className="cursor-pointer hover:shadow-lg hover:border-accent transition-all"
                  onClick={() => onNavigate?.(route)}
                >
                  <div className="flex items-center gap-3">
                    <div className="text-3xl">
                      {SKILL_ICONS[s.id] ?? '✨'}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="font-semibold text-ink">
                        {s.name || s.id}
                      </div>
                      <div className="text-xs text-ink-2">
                        v{s.version} · 启用中
                      </div>
                    </div>
                    <div className="text-accent-ink text-sm font-medium">
                      {label} →
                    </div>
                  </div>
                </Card>
              );
            })}
          </div>
        </section>
      )}

      {/* 已启用但无快捷入口的 skill */}
      {enabled.length > launchable.length && (
        <section className="mb-8">
          <h2 className="text-base font-semibold text-ink-2 mb-3">
            📂 已启用 ({enabled.length})
          </h2>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {enabled
              .filter((s) => !SKILL_LAUNCH_ROUTES[s.id])
              .map((s) => (
                <Card key={s.id} variant="bordered">
                  <div className="flex items-center gap-3">
                    <div className="text-3xl">
                      {SKILL_ICONS[s.id] ?? '✨'}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="font-medium text-ink">
                        {s.name || s.id}
                      </div>
                      <div className="text-xs text-ink-2">
                        v{s.version} · 通过 skill 接口调用
                      </div>
                    </div>
                  </div>
                </Card>
              ))}
          </div>
        </section>
      )}

      {/* 已禁用 */}
      {disabled.length > 0 && (
        <section className="mb-8">
          <h2 className="text-base font-semibold text-ink-2 mb-3">
            ⏸ 已禁用 ({disabled.length})
          </h2>
          <p className="text-xs text-ink-2 mb-3">
            在 Skill 市场可重新启用
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {disabled.map((s) => (
              <Card key={s.id} variant="default" className="opacity-60">
                <div className="flex items-center gap-3">
                  <div className="text-3xl grayscale">
                    {SKILL_ICONS[s.id] ?? '✨'}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="font-medium text-ink-2">
                      {s.name || s.id}
                    </div>
                    <div className="text-xs text-ink-3">
                      v{s.version} · 已禁用
                    </div>
                  </div>
                </div>
              </Card>
            ))}
          </div>
        </section>
      )}

      {/* 加载中 */}
      {loadingInstalled && enabled.length === 0 && (
        <div className="text-sm text-ink-2 text-center py-12">
          加载 skill 列表…
        </div>
      )}

      {/* 空状态: 引导装第一个 skill */}
      {!loadingInstalled && installed.length === 0 && (
        <Card className="mb-8 text-center py-12 bg-gradient-to-br from-accent-50 to-highlight/50">
          <div className="text-5xl mb-3">🎁</div>
          <h2 className="text-lg font-semibold text-ink mb-2">
            还没有装任何 skill
          </h2>
          <p className="text-sm text-ink-2 mb-4">
            去 Skill 市场装第一个 skill, 让你的 Agent 拥有创作能力
          </p>
          <button
            type="button"
            onClick={() => onNavigate?.('marketplace')}
            className="px-6 py-2.5 bg-accent text-bg rounded-lg hover:bg-accent-hover transition-colors"
          >
            📦 打开 Skill 市场
          </button>
        </Card>
      )}

      {/* 推荐 */}
      {notInstalled.length > 0 && (
        <section className="mb-8">
          <h2 className="text-base font-semibold text-ink-2 mb-3">
            💡 推荐 ({notInstalled.length})
          </h2>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {notInstalled.slice(0, 3).map((s) => (
              <Card
                key={s.id}
                variant="default"
                className="cursor-pointer hover:shadow-md transition-shadow"
                onClick={() => onNavigate?.('marketplace')}
              >
                <div className="flex items-center gap-3">
                  <div className="text-3xl">{SKILL_ICONS[s.id] ?? '✨'}</div>
                  <div className="flex-1 min-w-0">
                    <div className="font-medium text-ink">
                      {s.name}
                    </div>
                    <div className="text-xs text-ink-2 truncate">
                      {s.description ?? '去市场看看 →'}
                    </div>
                  </div>
                </div>
              </Card>
            ))}
          </div>
        </section>
      )}

      {/* 底部入口 */}
      <section className="border-t border-line pt-6 mt-8">
        <div className="flex flex-wrap gap-3 justify-center text-sm">
          <button
            type="button"
            onClick={() => onNavigate?.('marketplace')}
            className="text-accent-ink hover:underline"
          >
            📦 浏览完整 Skill 市场
          </button>
          <span className="text-ink-3">·</span>
          <button
            type="button"
            onClick={() => onNavigate?.('settings')}
            className="text-ink-2 hover:underline"
          >
            ⚙️ 家长设置
          </button>
        </div>
      </section>
      </div>
    </div>
  );
}