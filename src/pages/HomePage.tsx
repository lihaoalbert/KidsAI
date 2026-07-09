import { useEffect, useMemo } from 'react';
import Card from '../components/Card';
import { useLevelStore } from '../stores/levelStore';

interface HomePageProps {
  onOpenLevel?: (levelId: string) => void;
}

export default function HomePage({ onOpenLevel }: HomePageProps) {
  const { levels, isUnlocked, refresh, isLoading, error } = useLevelStore();

  useEffect(() => {
    refresh();
  }, [refresh]);

  const stats = useMemo(() => {
    const total = levels.length;
    const unlocked = levels.filter((l) => isUnlocked(l.id)).length;
    return { total, unlocked };
  }, [levels, isUnlocked]);

  return (
    <div className="p-8 max-w-6xl mx-auto">
      {/* 欢迎区 */}
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-gray-900">👋 你好，小创作者！</h1>
        <p className="text-sm text-gray-600 mt-1">
          欢迎来到 AI 创作世界。今天想做点什么？
        </p>
      </div>

      {/* 三大主菜单卡片 */}
      <div className="grid grid-cols-3 gap-4 mb-10">
        <Card variant="elevated">
          <div className="text-center py-4">
            <div className="text-4xl mb-2">🎮</div>
            <div className="font-semibold text-gray-900">游戏开发</div>
            <div className="text-xs text-gray-500 mt-1">用 AI 制作小游戏</div>
          </div>
        </Card>
        <Card variant="elevated">
          <div className="text-center py-4">
            <div className="text-4xl mb-2">🎬</div>
            <div className="font-semibold text-gray-900">视频创作</div>
            <div className="text-xs text-gray-500 mt-1">让 AI 帮你做视频</div>
          </div>
        </Card>
        <Card variant="elevated">
          <div className="text-center py-4">
            <div className="text-4xl mb-2">🤖</div>
            <div className="font-semibold text-gray-900">我的 Agent</div>
            <div className="text-xs text-gray-500 mt-1">打造专属 AI 助手</div>
          </div>
        </Card>
      </div>

      {/* 推荐关卡 - 来自真实关卡数据 + Tauri store */}
      <div className="mb-6">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-semibold text-gray-900">
            推荐关卡（已解锁 {stats.unlocked} / 共 {stats.total}）
          </h2>
          {isLoading && (
            <span className="text-xs text-gray-500">加载中…</span>
          )}
        </div>

        {error && (
          <div className="mb-3 p-3 rounded-md bg-red-50 border border-red-200 text-sm text-red-800">
            ⚠️ 加载关卡失败：{error}
          </div>
        )}

        <div className="grid grid-cols-3 gap-4">
          {levels.map((level) => {
            const available = isUnlocked(level.id);
            return (
              <Card
                key={level.id}
                variant={available ? 'bordered' : 'default'}
                className={
                  onOpenLevel && available
                    ? 'cursor-pointer hover:shadow-lg transition-shadow'
                    : ''
                }
                onClick={() => {
                  if (onOpenLevel && available) onOpenLevel(level.id);
                }}
                footer={
                  <div className="flex items-center justify-between text-xs text-gray-600">
                    <span>💎 {level.rewardTokens} 学币</span>
                    <span>⏱ {level.estimatedMinutes} 分钟</span>
                  </div>
                }
              >
                <div
                  className={`aspect-video rounded-md flex items-center justify-center text-5xl mb-2 ${
                    available
                      ? 'bg-gradient-to-br from-brand-100 to-warm-100'
                      : 'bg-gray-100 grayscale'
                  }`}
                >
                  {available ? level.coverEmoji : '🔒'}
                </div>
                <div className="text-xs font-mono text-brand-600 mb-1">
                  {level.id} · 难度{' '}
                  {'★'.repeat(level.difficulty)}
                </div>
                <div className="text-sm font-semibold text-gray-900 line-clamp-1">
                  {level.title}
                </div>
                <div className="text-xs text-gray-500 line-clamp-2 mt-1">
                  {level.description}
                </div>
              </Card>
            );
          })}
        </div>
      </div>

      {/* 骨架提示 */}
      <div className="mt-12 p-4 rounded-md bg-blue-50 border border-blue-200 text-sm text-blue-900">
        ℹ️ Week 2 进行中：W2.1 数据 + 详情页 ✓ ｜ W2.2 Tauri 命令 + Zustand
        stores ✓
      </div>
    </div>
  );
}
