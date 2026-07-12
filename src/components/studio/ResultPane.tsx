import { useState } from 'react';
import { useDirectorStore } from '../../stores/directorStore';
import { useStudioStore } from '../../stores/studioStore';
import CharacterEditor from './editors/CharacterEditor';
import ShotFxEditor from './editors/ShotFxEditor';

export default function ResultPane() {
  const cursor = useDirectorStore((s) => s.cursor);
  const character = useDirectorStore((s) => s.character);
  const style = useDirectorStore((s) => s.style);
  const shots = useDirectorStore((s) => s.shots);
  const finalVideoUrl = useDirectorStore((s) => s.finalVideoUrl);
  const previewIndex = useStudioStore((s) => s.previewIndex);
  const [fullscreen, setFullscreen] = useState(false);

  const charImg =
    character?.referenceImageUrl ??
    (character ? `https://picsum.photos/seed/${character.id}-ref/512/512` : null);

  const body = (() => {
    if (cursor >= 6 && finalVideoUrl) {
      return (
        <video src={finalVideoUrl} controls className="max-h-full max-w-full rounded-xl" />
      );
    }
    if (cursor === 5) {
      const shot = shots[previewIndex];
      if (shot?.previewUrl) {
        return <video src={shot.previewUrl} controls className="max-h-full max-w-full rounded-xl" />;
      }
      return (
        <div className="text-center text-sm text-gray-400">
          <div className="mb-2 text-4xl">🎬</div>
          第 {previewIndex + 1} 段还没拍<br />在左边点「开始拍」
        </div>
      );
    }
    if (cursor === 4) {
      return (
        <div className="flex flex-wrap items-center justify-center gap-2">
          {shots.map((s, i) => (
            <div key={s.id} className="flex items-center gap-2">
              <div className="w-24 rounded-lg border border-gray-200 bg-white p-2 text-center">
                <div className="text-lg">🖼️</div>
                <div className="mt-1 line-clamp-2 text-[10px] text-gray-500">{s.description}</div>
              </div>
              {i < shots.length - 1 && <span className="text-gray-300">→</span>}
            </div>
          ))}
        </div>
      );
    }
    if (cursor >= 2 && charImg) {
      return (
        <div className="text-center">
          <img
            src={charImg}
            alt={character?.name}
            className="mx-auto max-h-72 rounded-2xl border-4 border-white shadow-md"
          />
          <div className="mt-3 text-sm font-bold text-gray-700">📸 {character?.name}</div>
          {style && <div className="mt-1 text-xs text-gray-400">画风：{style.name}</div>}
        </div>
      );
    }
    return (
      <div className="text-center text-sm text-gray-400">
        <div className="mb-2 text-5xl">✨</div>
        你的电影会出现在这里
      </div>
    );
  })();

  return (
    <div className="flex h-full flex-col bg-gradient-to-b from-gray-50 to-white">
      <div className="flex items-center justify-between border-b border-gray-100 px-4 py-2.5">
        <span className="text-sm font-bold text-gray-700">🎥 作品预览</span>
        {(finalVideoUrl || shots[previewIndex]?.previewUrl) && (
          <button
            onClick={() => setFullscreen(true)}
            className="rounded-lg bg-gray-100 px-2.5 py-1 text-xs font-semibold text-gray-600 hover:bg-gray-200"
          >
            ⛶ 全屏
          </button>
        )}
      </div>

      <div className="flex flex-1 items-center justify-center overflow-auto p-5">{body}</div>

      {/* 阶段2-4: 主角微调器 (色块/大小/表情) */}
      {cursor >= 2 && cursor < 5 && character && <CharacterEditor />}

      {/* 阶段5: 单镜微调器 (速度/音效/滤镜) */}
      {cursor === 5 && shots[previewIndex] && (
        <ShotFxEditor shotId={shots[previewIndex].id} />
      )}

      {fullscreen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-8"
          onClick={() => setFullscreen(false)}
        >
          <button className="absolute right-6 top-6 text-2xl text-white">✕</button>
          <div className="max-h-full max-w-full" onClick={(e) => e.stopPropagation()}>
            {cursor >= 6 && finalVideoUrl ? (
              <video src={finalVideoUrl} controls autoPlay className="max-h-[85vh] rounded-xl" />
            ) : shots[previewIndex]?.previewUrl ? (
              <video src={shots[previewIndex].previewUrl!} controls autoPlay className="max-h-[85vh] rounded-xl" />
            ) : null}
          </div>
        </div>
      )}
    </div>
  );
}
