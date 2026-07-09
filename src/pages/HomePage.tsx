import Card from '../components/Card';

export default function HomePage() {
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

      {/* 推荐关卡 */}
      <div className="mb-6">
        <h2 className="text-lg font-semibold text-gray-900 mb-3">推荐关卡</h2>
        <div className="grid grid-cols-3 gap-4">
          {[1, 2, 3].map((n) => (
            <Card
              key={n}
              title={`L${n} · 入门关卡 ${n}`}
              description="学习 AI 视频创作的第一步"
              variant="default"
              footer={
                <div className="flex items-center justify-between text-xs text-gray-600">
                  <span>💎 30 学币</span>
                  <span>⏱ 20 分钟</span>
                </div>
              }
            >
              <div className="aspect-video bg-gradient-to-br from-brand-100 to-warm-100 rounded-md flex items-center justify-center text-3xl">
                🎬
              </div>
            </Card>
          ))}
        </div>
      </div>

      {/* 骨架提示 */}
      <div className="mt-12 p-4 rounded-md bg-blue-50 border border-blue-200 text-sm text-blue-900">
        ℹ️ 这是 Week 1 骨架。关卡内容、编辑器、家长端等将在后续 Sprint 接入。
      </div>
    </div>
  );
}
