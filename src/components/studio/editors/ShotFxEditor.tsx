// 阶段5 单镜轻量微调：速度档位 ⏩ / 音效预设 🔊 / 滤镜预设 ✨
// 严格遵守"成就感悖论"护栏：只允许大按钮 + 预设, 禁止数值输入/滑块/时间轴
import { useDirectorStore } from '../../../stores/directorStore';

const SPEEDS = [
  { id: 'slow' as const, label: '慢动作', emoji: '🐢' },
  { id: 'normal' as const, label: '正常', emoji: '🐰' },
  { id: 'fast' as const, label: '超快', emoji: '🚀' },
];

const SOUNDS = [
  { id: 'forest', label: '森林鸟鸣', emoji: '🌳' },
  { id: 'ocean', label: '海浪拍岸', emoji: '🌊' },
  { id: 'magic', label: '魔法叮咚', emoji: '✨' },
  { id: 'wind', label: '呼呼风声', emoji: '🌬️' },
  { id: 'laughter', label: '孩子笑声', emoji: '😆' },
];

const FILTERS = [
  { id: 'sunny', label: '暖阳光', emoji: '☀️' },
  { id: 'moonlight', label: '月光夜', emoji: '🌙' },
  { id: 'sakura', label: '樱花雨', emoji: '🌸' },
  { id: 'rainbow', label: '彩虹糖', emoji: '🌈' },
  { id: 'dream', label: '梦境软', emoji: '💭' },
];

export default function ShotFxEditor({ shotId }: { shotId: string }) {
  const shot = useDirectorStore((s) => s.shots.find((sh) => sh.id === shotId));
  const setShotFx = useDirectorStore((s) => s.setShotFx);

  if (!shot) return null;
  const fx = shot.fx ?? {};

  const undo = () => setShotFx(shotId, { speed: undefined, sound: undefined, filter: undefined });

  return (
    <div className="border-t border-gray-100 bg-white/60 px-4 py-3 text-xs">
      <div className="mb-2 flex items-center justify-between">
        <span className="font-semibold text-gray-700">✨ 给这一镜加点魔法</span>
        <button
          type="button"
          onClick={undo}
          className="rounded-full bg-gray-100 px-2 py-0.5 text-[10px] text-gray-500 hover:bg-gray-200"
        >
          ↺ 还原
        </button>
      </div>

      {/* 速度 */}
      <div className="mb-2.5">
        <div className="mb-1 text-[10px] text-gray-500">⏩ 速度</div>
        <div className="flex gap-1.5">
          {SPEEDS.map((sp) => {
            const active = fx.speed === sp.id;
            return (
              <button
                key={sp.id}
                type="button"
                onClick={() => setShotFx(shotId, { speed: sp.id })}
                className={`flex flex-1 items-center justify-center gap-1 rounded-xl border px-2 py-1.5 text-[11px] transition-all ${
                  active
                    ? 'border-brand-500 bg-brand-50 text-brand-700'
                    : 'border-gray-200 bg-white text-gray-600 hover:border-brand-200'
                }`}
              >
                <span>{sp.emoji}</span>
                <span>{sp.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* 音效 */}
      <div className="mb-2.5">
        <div className="mb-1 text-[10px] text-gray-500">🔊 声音</div>
        <div className="flex flex-wrap gap-1.5">
          {SOUNDS.map((sd) => {
            const active = fx.sound === sd.id;
            return (
              <button
                key={sd.id}
                type="button"
                onClick={() => setShotFx(shotId, { sound: sd.id })}
                className={`flex items-center gap-1 rounded-xl border px-2 py-1 text-[11px] transition-all ${
                  active
                    ? 'border-brand-500 bg-brand-50 text-brand-700'
                    : 'border-gray-200 bg-white text-gray-600 hover:border-brand-200'
                }`}
              >
                <span>{sd.emoji}</span>
                <span>{sd.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* 滤镜 */}
      <div>
        <div className="mb-1 text-[10px] text-gray-500">✨ 滤镜</div>
        <div className="flex flex-wrap gap-1.5">
          {FILTERS.map((f) => {
            const active = fx.filter === f.id;
            return (
              <button
                key={f.id}
                type="button"
                onClick={() => setShotFx(shotId, { filter: f.id })}
                className={`flex items-center gap-1 rounded-xl border px-2 py-1 text-[11px] transition-all ${
                  active
                    ? 'border-brand-500 bg-brand-50 text-brand-700'
                    : 'border-gray-200 bg-white text-gray-600 hover:border-brand-200'
                }`}
              >
                <span>{f.emoji}</span>
                <span>{f.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {(fx.speed || fx.sound || fx.filter) && (
        <div className="mt-2 rounded-lg bg-brand-50 px-2 py-1 text-[10px] text-brand-700">
          {fx.speed && SPEEDS.find((sp) => sp.id === fx.speed)?.label}
          {fx.sound && ` · ${SOUNDS.find((sd) => sd.id === fx.sound)?.label}`}
          {fx.filter && ` · ${FILTERS.find((f) => f.id === fx.filter)?.label}`}
        </div>
      )}
    </div>
  );
}