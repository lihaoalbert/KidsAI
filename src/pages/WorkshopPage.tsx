export default function WorkshopPage() {
  return (
    <div className="p-8 max-w-6xl mx-auto">
      <h1 className="text-2xl font-bold text-gray-900 mb-2">🎨 作品工坊</h1>
      <p className="text-sm text-gray-600 mb-8">
        在这里开始你的 AI 创作
      </p>
      <div className="grid grid-cols-2 gap-4">
        <div className="p-6 rounded-lg bg-white border border-gray-200 text-center">
          <div className="text-4xl mb-3">🖼️</div>
          <div className="font-semibold text-gray-900 mb-1">文生图</div>
          <div className="text-xs text-gray-500">用文字描述生成图片</div>
        </div>
        <div className="p-6 rounded-lg bg-white border border-gray-200 text-center">
          <div className="text-4xl mb-3">🎬</div>
          <div className="font-semibold text-gray-900 mb-1">图生视频</div>
          <div className="text-xs text-gray-500">把图片变成动画</div>
        </div>
      </div>
    </div>
  );
}
