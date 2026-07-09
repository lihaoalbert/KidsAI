import Card from '../components/Card';
import { LEVELS, getAvailableLevels } from '../data/levels';

interface HomePageProps {
  onOpenLevel?: (levelId: string) => void;
}

export default function HomePage({ onOpenLevel }: HomePageProps) {
  // MVP 阶段：用户尚未完成任何关卡，所有无前置依赖的关卡都可选
  // Week 2.3 接入 SQLite 后会读取真实进度
  const completedIds: string[] = [];
  const available = getAvailableLevels(completedIds);
  const availableIds = new Set(available.map((l) => l.id));

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

      {/* 推荐关卡 - 来自真实关卡数据 */}
      <div className="mb-6">
        <h2 className="text-lg font-semibold text-gray-900 mb-3">
          推荐关卡（MVP 共 {LEVELS.length} 个关卡，当前可选{' '}
          {available.length} 个）
        </h2>
        <div className="grid grid-cols-3 gap-4">
          {LEVELS.map((level) => {
            const isAvailable = availableIds.has(level.id);
            return (
              <Card
                key={level.id}
                variant={isAvailable ? 'bordered' : 'default'}
                className={
                  onOpenLevel && isAvailable
                    ? 'cursor-pointer hover:shadow-lg transition-shadow'
                    : ''
                }
                onClick={() => {
                  if (onOpenLevel && isAvailable) onOpenLevel(level.id);
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
                    isAvailable
                      ? 'bg-gradient-to-br from-brand-100 to-warm-100'
                      : 'bg-gray-100 grayscale'
                  }`}
                >
                  {isAvailable ? level.coverEmoji : '🔒'}
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
        ℹ️ Week 2 进行中：W2.1 关卡数据 + 详情页已就绪。点击上方关卡卡片可查看详情。
      </div>
    </div>
  );
}
