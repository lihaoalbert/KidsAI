import { useEffect, useState } from 'react';
import Card from '../components/Card';
import { listCreations, type CreationWithAssets } from '../api/tauri';
import { useLevelStore } from '../stores/levelStore';

export default function LibraryPage() {
  const [creations, setCreations] = useState<CreationWithAssets[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { levels } = useLevelStore();

  const refresh = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const list = await listCreations();
      setCreations(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const levelTitle = (id: string) =>
    levels.find((l) => l.id === id)?.title ?? id;

  return (
    <div className="p-8 max-w-6xl mx-auto">
      <div className="flex items-center justify-between mb-2">
        <h1 className="text-2xl font-bold text-gray-900">📚 作品库</h1>
        <button
          onClick={refresh}
          className="text-sm text-brand-600 hover:underline"
        >
          刷新
        </button>
      </div>
      <p className="text-sm text-gray-600 mb-8">
        你创作的所有作品都会保存在这里（W2.3: 已接入本地 SQLite）
      </p>

      {error && (
        <div className="mb-4 p-3 rounded-md bg-red-50 border border-red-200 text-sm text-red-800">
          ⚠️ {error}
        </div>
      )}

      {isLoading ? (
        <div className="text-sm text-gray-500 py-12 text-center">加载中…</div>
      ) : creations.length === 0 ? (
        <div className="rounded-lg bg-white border border-dashed border-gray-300 p-16 text-center">
          <div className="text-5xl mb-4">🎨</div>
          <div className="font-semibold text-gray-900 mb-1">还没有作品</div>
          <div className="text-sm text-gray-500">
            完成第一个关卡后，作品会出现在这里
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-4">
          {creations.map((c) => (
            <Card
              key={c.id}
              title={levelTitle(c.levelId)}
              description={c.userInput}
              footer={
                <div className="flex items-center justify-between text-xs text-gray-600">
                  <span>得分 {c.score ?? '-'}</span>
                  <span>{new Date(c.createdAt).toLocaleString()}</span>
                </div>
              }
            >
              <div className="space-y-2">
                {c.assets.length > 0 ? (
                  <div className="grid grid-cols-2 gap-2">
                    {c.assets.map((a, i) => (
                      <div
                        key={i}
                        className="aspect-video bg-gray-100 rounded-md flex items-center justify-center text-2xl"
                      >
                        {a.kind === 'image' && '🖼️'}
                        {a.kind === 'video' && '🎬'}
                        {a.kind === 'audio' && '🔊'}
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="aspect-video bg-gradient-to-br from-brand-50 to-warm-50 rounded-md flex items-center justify-center text-3xl">
                    🎨
                  </div>
                )}
                {c.feedback && (
                  <div className="text-xs text-gray-600 bg-gray-50 rounded p-2">
                    {c.feedback}
                  </div>
                )}
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
