export default function LibraryPage() {
  return (
    <div className="p-8 max-w-6xl mx-auto">
      <h1 className="text-2xl font-bold text-gray-900 mb-2">📚 作品库</h1>
      <p className="text-sm text-gray-600 mb-8">
        你创作的所有作品都会保存在这里
      </p>
      <div className="rounded-lg bg-white border border-dashed border-gray-300 p-16 text-center">
        <div className="text-5xl mb-4">🎨</div>
        <div className="font-semibold text-gray-900 mb-1">还没有作品</div>
        <div className="text-sm text-gray-500">
          完成第一个关卡后，作品会出现在这里
        </div>
      </div>
    </div>
  );
}
