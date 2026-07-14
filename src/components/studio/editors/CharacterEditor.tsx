// 阶段2 主角微调：色块 🎨 / 大小档位 📏 / 表情预设 😀
// 严格遵守"成就感悖论"护栏：只允许大按钮 + 预设, 禁止数值输入/滑块/时间轴
import { useDirectorStore } from '../../../stores/directorStore';

const COLORS = [
  { id: 'sunshine', name: '阳光黄', hex: '#FACC15' },
  { id: 'sky', name: '天空蓝', hex: '#38BDF8' },
  { id: 'rose', name: '玫瑰粉', hex: '#FB7185' },
  { id: 'mint', name: '薄荷绿', hex: '#4ADE80' },
  { id: 'lavender', name: '薰衣紫', hex: '#A78BFA' },
  { id: 'cocoa', name: '可可棕', hex: '#A16207' },
] as const;

const SIZES = [
  { id: 'S' as const, label: '小小', emoji: '🐣' },
  { id: 'M' as const, label: '正好', emoji: '🐾' },
  { id: 'L' as const, label: '大大', emoji: '🦖' },
];

const EXPRESSIONS = [
  { id: 'happy', label: '开心', emoji: '😄' },
  { id: 'brave', label: '勇敢', emoji: '😎' },
  { id: 'curious', label: '好奇', emoji: '🤔' },
  { id: 'sleepy', label: '困困', emoji: '😴' },
];

export default function CharacterEditor() {
  const tweak = useDirectorStore((s) => s.characterTweak);
  const setTweak = useDirectorStore((s) => s.setCharacterTweak);

  const undo = () => setTweak({ color: undefined, size: undefined, expression: undefined });

  return (
    <div className="border-t border-line bg-surface/60 px-4 py-3 text-xs">
      <div className="mb-2 flex items-center justify-between">
        <span className="font-semibold text-ink-2">🎨 给主角加点料</span>
        <button
          type="button"
          onClick={undo}
          className="rounded-full bg-surface-2 px-2 py-0.5 text-[10px] text-ink-2 hover:bg-surface-2"
        >
          ↺ 还原
        </button>
      </div>

      {/* 颜色 */}
      <div className="mb-2.5">
        <div className="mb-1 text-[10px] text-ink-2">🎨 颜色</div>
        <div className="flex flex-wrap gap-1.5">
          {COLORS.map((c) => {
            const active = tweak.color === c.id;
            return (
              <button
                key={c.id}
                type="button"
                onClick={() => setTweak({ color: c.id })}
                title={c.name}
                className={`flex h-7 w-7 items-center justify-center rounded-full border-2 transition-all ${
                  active
                    ? 'scale-110 border-accent shadow-md'
                    : 'border-white hover:scale-105'
                }`}
                style={{ backgroundColor: c.hex }}
              >
                {active && <span className="text-[10px] text-bg">✓</span>}
              </button>
            );
          })}
        </div>
      </div>

      {/* 大小 */}
      <div className="mb-2.5">
        <div className="mb-1 text-[10px] text-ink-2">📏 大小</div>
        <div className="flex gap-1.5">
          {SIZES.map((s) => {
            const active = tweak.size === s.id;
            return (
              <button
                key={s.id}
                type="button"
                onClick={() => setTweak({ size: s.id })}
                className={`flex flex-1 items-center justify-center gap-1 rounded-xl border px-2 py-1.5 text-[11px] transition-all ${
                  active
                    ? 'border-accent bg-accent-soft text-accent-ink'
                    : 'border-line bg-surface text-ink-2 hover:border-accent-line'
                }`}
              >
                <span>{s.emoji}</span>
                <span>{s.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* 表情 */}
      <div>
        <div className="mb-1 text-[10px] text-ink-2">😊 表情</div>
        <div className="flex flex-wrap gap-1.5">
          {EXPRESSIONS.map((e) => {
            const active = tweak.expression === e.id;
            return (
              <button
                key={e.id}
                type="button"
                onClick={() => setTweak({ expression: e.id })}
                className={`flex items-center gap-1 rounded-xl border px-2 py-1 text-[11px] transition-all ${
                  active
                    ? 'border-accent bg-accent-soft text-accent-ink'
                    : 'border-line bg-surface text-ink-2 hover:border-accent-line'
                }`}
              >
                <span>{e.emoji}</span>
                <span>{e.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* 已选摘要(给家长/孩子回看) */}
      {(tweak.color || tweak.size || tweak.expression) && (
        <div className="mt-2 rounded-lg bg-accent-soft px-2 py-1 text-[10px] text-accent-ink">
          {tweak.color && COLORS.find((c) => c.id === tweak.color)?.name}
          {tweak.size && ` · ${SIZES.find((s) => s.id === tweak.size)?.label}`}
          {tweak.expression &&
            ` · ${EXPRESSIONS.find((e) => e.id === tweak.expression)?.label}`}
        </div>
      )}
    </div>
  );
}