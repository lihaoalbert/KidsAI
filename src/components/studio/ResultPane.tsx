import { useEffect, useState } from 'react';
import { useDirectorStore } from '../../stores/directorStore';
import { useStudioStore } from '../../stores/studioStore';
import { generatedAssetUrl, useAssetStore } from '../../stores/assetStore';
import { useLocalAsset } from '../../stores/projectStore';
import CharacterEditor from './editors/CharacterEditor';
import ShotFxEditor from './editors/ShotFxEditor';

const STYLE_PREVIEW_VARIANTS = [
  ['full', '全身'],
  ['half', '半身'],
  ['expression', '表情'],
  ['still', '场景'],
] as const;

export default function ResultPane() {
  const cursor = useDirectorStore((s) => s.cursor);
  const character = useDirectorStore((s) => s.character);
  const style = useDirectorStore((s) => s.style);
  const shots = useDirectorStore((s) => s.shots);
  const finalVideoUrl = useDirectorStore((s) => s.finalVideoUrl);
  const previewIndex = useStudioStore((s) => s.previewIndex);
  const manifestImages = useAssetStore((s) => s.manifest?.images);
  const [fullscreen, setFullscreen] = useState(false);

  const characterAssetUrl = character
    ? manifestImages?.[`${character.id}.stand`] ??
      generatedAssetUrl('character', `${character.id}.stand`)
    : null;
  const [charImg, setCharImg] = useState<string | null>(characterAssetUrl);

  useEffect(() => {
    setCharImg(characterAssetUrl);
  }, [characterAssetUrl]);

  const stylePreviews = style
    ? STYLE_PREVIEW_VARIANTS.map(([variant, label]) => ({
        label,
        url:
          manifestImages?.[`${style.id}.${variant}`] ??
          generatedAssetUrl('style', `${style.id}.${variant}`),
      }))
    : [];

  const body = (() => {
    if (cursor >= 6 && finalVideoUrl) {
      return <FinalVideoPlayer url={finalVideoUrl} />;
    }
    if (cursor === 5) {
      const shot = shots[previewIndex];
      if (shot?.previewUrl) {
        return <PreviewVideoPlayer url={shot.previewUrl} />;
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
    if (cursor === 3 && style) {
      return (
        <div className="w-full text-center">
          <div className="grid grid-cols-2 gap-2">
            {stylePreviews.map((preview) => (
              <figure key={preview.label} className="overflow-hidden rounded-xl bg-white shadow-sm">
                <img
                  src={preview.url}
                  alt={`${style.name}${preview.label}预览`}
                  className="h-32 w-full object-cover"
                />
                <figcaption className="py-1.5 text-[11px] font-semibold text-gray-500">
                  {preview.label}
                </figcaption>
              </figure>
            ))}
          </div>
          <div className="mt-3 text-sm font-bold text-gray-700">🎨 {style.name}</div>
          <div className="mt-1 text-xs text-gray-400">四宫格画风参考</div>
        </div>
      );
    }
    if (cursor === 2 && charImg) {
      return (
        <div className="text-center">
          <img
            src={charImg}
            alt={`${character?.name ?? '主角'}标准照`}
            onError={() => {
              const fallback = character?.referenceImageUrl ?? null;
              setCharImg(fallback && fallback !== charImg ? fallback : null);
            }}
            className="mx-auto max-h-72 rounded-2xl border-4 border-white shadow-md"
          />
          <div className="mt-3 text-sm font-bold text-gray-700">📸 {character?.name}标准照</div>
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

      {cursor >= 2 && cursor < 5 && character && <CharacterEditor />}

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
              <FinalVideoPlayer url={finalVideoUrl} fullscreen />
            ) : shots[previewIndex]?.previewUrl ? (
              <PreviewVideoPlayer url={shots[previewIndex].previewUrl!} fullscreen />
            ) : null}
          </div>
        </div>
      )}
    </div>
  );
}

function FinalVideoPlayer({ url, fullscreen = false }: { url: string; fullscreen?: boolean }) {
  const src = useLocalAsset(url);
  return (
    <video
      src={src ?? ''}
      controls
      autoPlay={fullscreen}
      className={fullscreen ? 'max-h-[85vh] rounded-xl' : 'max-h-full max-w-full rounded-xl'}
    />
  );
}

function PreviewVideoPlayer({ url, fullscreen = false }: { url: string; fullscreen?: boolean }) {
  const src = useLocalAsset(url);
  return (
    <video
      src={src ?? ''}
      controls
      autoPlay={fullscreen}
      className={fullscreen ? 'max-h-[85vh] rounded-xl' : 'max-h-full max-w-full rounded-xl'}
    />
  );
}
