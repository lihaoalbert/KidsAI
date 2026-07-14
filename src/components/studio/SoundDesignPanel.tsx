// W9: 声音设计 4 维面板 — BGM 情绪 + BGM 音量 + SFX 关键点 + 配音语气 + 静音节拍

import type { BgmMood, ShotSoundDesign, SfxCue } from '../../stores/directorStore';

interface SoundDesignPanelProps {
  value?: ShotSoundDesign;
  onChange: (patch: Partial<ShotSoundDesign>) => void;
}

const BGM_OPTIONS: Array<{ value: BgmMood; label: string; emoji: string }> = [
  { value: 'none', label: '无', emoji: '🔇' },
  { value: 'playful', label: '欢快', emoji: '🎈' },
  { value: 'tense', label: '紧张', emoji: '😰' },
  { value: 'epic', label: '史诗', emoji: '⚔️' },
  { value: 'sad', label: '悲伤', emoji: '😢' },
  { value: 'triumphant', label: '胜利', emoji: '🏆' },
  { value: 'mysterious', label: '神秘', emoji: '🔮' },
];

const SFX_KINDS = ['footstep', 'roar', 'wind', 'magic', 'fire', 'splash', 'magic-chime'];

export default function SoundDesignPanel({ value, onChange }: SoundDesignPanelProps) {
  const v = value ?? {
    bgm_mood: 'none', bgm_volume: 30, sfx_cues: [], voice_direction: '', silence_beat: false,
  };

  const addCue = () => {
    const kind = prompt(`SFX 类型 (${SFX_KINDS.join('/')})`, SFX_KINDS[0]);
    if (!kind) return;
    const timeSecStr = prompt('时间点 (秒, 0-5)', '1');
    if (!timeSecStr) return;
    const timeSec = parseFloat(timeSecStr);
    if (Number.isNaN(timeSec)) return;
    const description = prompt('描述', '') ?? '';
    const cue: SfxCue = { timeSec, kind, description };
    onChange({ sfx_cues: [...v.sfx_cues, cue] });
  };

  const removeCue = (idx: number) => {
    onChange({ sfx_cues: v.sfx_cues.filter((_, i) => i !== idx) });
  };

  return (
    <div>
      <h4 className="mb-2 text-xs font-semibold text-ink-2">🎵 声音设计 (4 维)</h4>
      <div className="space-y-2 rounded-lg bg-surface p-3">
        <div>
          <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-ink-2">BGM 情绪</div>
          <div className="flex flex-wrap gap-1">
            {BGM_OPTIONS.map((opt) => (
              <button
                key={opt.value}
                type="button"
                onClick={() => onChange({ bgm_mood: opt.value })}
                className={[
                  'rounded-md px-2 py-1 text-xs',
                  v.bgm_mood === opt.value
                    ? 'bg-accent text-bg'
                    : 'bg-surface text-ink-2 hover:bg-surface-2 border border-line',
                ].join(' ')}
              >
                <span className="mr-0.5">{opt.emoji}</span>
                {opt.label}
              </button>
            ))}
          </div>
        </div>

        <div>
          <div className="mb-1 flex items-center justify-between">
            <span className="text-[10px] font-semibold uppercase tracking-wide text-ink-2">BGM 音量</span>
            <span className="text-xs text-ink-2">{v.bgm_volume}%</span>
          </div>
          <input
            type="range"
            min={0}
            max={100}
            value={v.bgm_volume}
            onChange={(e) => onChange({ bgm_volume: parseInt(e.target.value, 10) })}
            className="w-full accent-accent"
          />
        </div>

        <div>
          <div className="mb-1 flex items-center justify-between">
            <span className="text-[10px] font-semibold uppercase tracking-wide text-ink-2">SFX 关键点</span>
            <button
              type="button"
              onClick={addCue}
              className="rounded px-1.5 py-0.5 text-[10px] text-accent-ink hover:bg-accent-soft"
            >
              ＋ 添加
            </button>
          </div>
          {v.sfx_cues.length === 0 ? (
            <p className="text-xs text-ink-3">（无 SFX）</p>
          ) : (
            <ul className="space-y-1">
              {v.sfx_cues.map((cue, i) => (
                <li key={i} className="flex items-center justify-between rounded bg-surface-2 px-2 py-1 text-xs">
                  <span>
                    <span className="font-mono text-ink-2">{cue.timeSec.toFixed(1)}s</span> · {cue.kind}
                    {cue.description && <span className="text-ink-2"> — {cue.description}</span>}
                  </span>
                  <button
                    type="button"
                    onClick={() => removeCue(i)}
                    className="text-danger hover:text-danger"
                  >
                    ✕
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div>
          <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-ink-2">配音语气</div>
          <input
            type="text"
            defaultValue={v.voice_direction}
            onBlur={(e) => onChange({ voice_direction: e.target.value })}
            placeholder="明亮、好奇、有吸引力"
            className="w-full rounded-md border border-line px-2 py-1 text-xs focus:border-accent focus:outline-none"
          />
        </div>

        <label className="flex items-center gap-2 text-xs text-ink-2">
          <input
            type="checkbox"
            checked={v.silence_beat}
            onChange={(e) => onChange({ silence_beat: e.target.checked })}
            className="rounded accent-accent"
          />
          <span>静音节拍 (这一镜前 1 秒静音)</span>
        </label>
      </div>
    </div>
  );
}