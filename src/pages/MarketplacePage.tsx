// W10 Day 4 — MarketplacePage
//
// 家长专属页面 /skills:
//   - 顶部 mode 提示 (儿童 / 成人 / 通用)
//   - 已上架 skill (来自 server index, 按当前 mode 过滤)
//   - 已装 skill 列表 (本地)
//   - 安装 / 卸载 / 启用 / 禁用

import { useEffect, useMemo } from 'react';
import { useSkillStore } from '../stores/skillStore';
import { useUserModeStore } from '../stores/userModeStore';
import { SkillCard } from '../components/marketplace/SkillCard';

export default function MarketplacePage() {
  const installed = useSkillStore((s) => s.installed);
  const available = useSkillStore((s) => s.available);
  const loadingInstalled = useSkillStore((s) => s.loadingInstalled);
  const loadingAvailable = useSkillStore((s) => s.loadingAvailable);
  const error = useSkillStore((s) => s.error);
  const refreshAll = useSkillStore((s) => s.refreshAll);
  const clearError = useSkillStore((s) => s.clearError);
  const mode = useUserModeStore((s) => s.mode);

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  // 按 audience 分桶
  const byAudience = useMemo(() => {
    const child = available.filter((s) => s.audience === 'child' || s.audience === 'both');
    const adult = available.filter((s) => s.audience === 'adult' || s.audience === 'both');
    return { child, adult };
  }, [available]);

  return (
    <div className="max-w-5xl mx-auto p-6">
      <header className="mb-6">
        <h1 className="text-2xl font-bold text-gray-800 mb-1">📦 Skill 市场</h1>
        <p className="text-sm text-gray-600">
          {mode === 'child'
            ? '儿童模式: 仅显示儿童 / 通用 skill (隐藏成人专属). 安装需家长 PIN.'
            : '成人模式: 显示全部 skill. 安装需家长 PIN.'}
        </p>
      </header>

      {error && (
        <div
          className="bg-red-50 border border-red-200 rounded-lg p-3 mb-4 flex items-center justify-between"
          data-testid="marketplace-error"
        >
          <p className="text-sm text-red-700">{error}</p>
          <button
            type="button"
            className="text-xs text-red-600 hover:underline"
            onClick={clearError}
          >
            关闭
          </button>
        </div>
      )}

      {/* 已装 */}
      <section className="mb-8">
        <h2 className="text-lg font-semibold text-gray-800 mb-3">
          📂 我的 Skill ({installed.length})
        </h2>
        {loadingInstalled ? (
          <p className="text-sm text-gray-500">加载中…</p>
        ) : installed.length === 0 ? (
          <p className="text-sm text-gray-500">还没有安装任何 skill</p>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {installed.map((s) => (
              <div
                key={s.id}
                className="border border-gray-200 rounded-xl p-3 bg-white shadow-sm"
              >
                <div className="flex items-center justify-between">
                  <div>
                    <p className="font-medium text-gray-800">{s.name || s.id}</p>
                    <p className="text-xs text-gray-500">
                      v{s.version} · {s.enabled ? '启用中' : '已禁用'}
                    </p>
                  </div>
                  <span
                    className={`px-2 py-0.5 rounded-full text-xs ${
                      s.audience === 'adult'
                        ? 'bg-purple-100 text-purple-700'
                        : s.audience === 'both'
                          ? 'bg-blue-100 text-blue-700'
                          : 'bg-green-100 text-green-700'
                    }`}
                  >
                    {s.audience}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* 儿童 / 通用 */}
      <section className="mb-8">
        <h2 className="text-lg font-semibold text-gray-800 mb-3">
          🛒 可用 Skill ({byAudience.child.length})
        </h2>
        {loadingAvailable ? (
          <p className="text-sm text-gray-500">加载中…</p>
        ) : byAudience.child.length === 0 ? (
          <p className="text-sm text-gray-500">没有可用的 skill</p>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {byAudience.child.map((s) => (
              <SkillCard key={s.id} skill={s} />
            ))}
          </div>
        )}
      </section>

      {/* 成人专属 (仅成人模式可见) */}
      {mode === 'adult' && (
        <section className="mb-8">
          <h2 className="text-lg font-semibold text-gray-800 mb-3">
            🎬 成人专属 Skill ({byAudience.adult.length})
          </h2>
          <p className="text-xs text-amber-600 mb-3">
            ⚠️ 仅成人模式可见. 这些 skill 设计用于商业广告 / 纪录片 / 求职作品集等专业场景.
          </p>
          {byAudience.adult.length === 0 ? (
            <p className="text-sm text-gray-500">没有可用的成人 skill</p>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
              {byAudience.adult.map((s) => (
                <SkillCard key={s.id} skill={s} />
              ))}
            </div>
          )}
        </section>
      )}
    </div>
  );
}