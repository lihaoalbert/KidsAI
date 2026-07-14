// W9: 镜头语言 6 维面板 — 景别 / 角度 / 运镜 / 转场 in/out / 光线 / 调色

import type { ShotCinematography, ShotType, ShotAngle, ShotMovement, ShotTransitionStyle, ShotLighting, ShotColorGrade } from '../../stores/directorStore';

interface CinematographyPanelProps {
  value?: ShotCinematography;
  onChange: (patch: Partial<ShotCinematography>) => void;
}

const SHOT_TYPE_OPTIONS: Array<{ value: ShotType; label: string; emoji: string }> = [
  { value: 'extreme-wide', label: '远', emoji: '🌄' },
  { value: 'wide', label: '全景', emoji: '🏞️' },
  { value: 'medium', label: '中景', emoji: '👥' },
  { value: 'close-up', label: '近景', emoji: '👤' },
  { value: 'extreme-close-up', label: '特写', emoji: '👁️' },
  { value: 'over-shoulder', label: '过肩', emoji: '👥↔️' },
  { value: 'pov', label: 'POV', emoji: '👀' },
  { value: 'aerial', label: '俯拍', emoji: '🦅' },
];

const ANGLE_OPTIONS: Array<{ value: ShotAngle; label: string }> = [
  { value: 'eye-level', label: '平视' },
  { value: 'low', label: '仰拍' },
  { value: 'high', label: '俯视' },
  { value: 'dutch', label: '倾斜' },
  { value: 'birds-eye', label: '鸟瞰' },
  { value: 'worms-eye', label: '虫视' },
];

const MOVEMENT_OPTIONS: Array<{ value: ShotMovement; label: string }> = [
  { value: 'static', label: '静止' },
  { value: 'pan', label: '摇' },
  { value: 'tilt', label: '俯仰' },
  { value: 'dolly', label: '推拉' },
  { value: 'track', label: '横移' },
  { value: 'crane', label: '升降' },
  { value: 'handheld', label: '手持' },
  { value: 'zoom', label: '变焦' },
];

const TRANSITION_OPTIONS: Array<{ value: ShotTransitionStyle; label: string }> = [
  { value: 'cut', label: '硬切' },
  { value: 'fade', label: '淡入' },
  { value: 'dissolve', label: '溶解' },
  { value: 'wipe', label: '划' },
  { value: 'match', label: '匹配' },
  { value: 'jump', label: '跳切' },
];

const LIGHTING_OPTIONS: Array<{ value: ShotLighting; label: string }> = [
  { value: 'natural', label: '自然' },
  { value: 'golden-hour', label: '黄金' },
  { value: 'blue-hour', label: '蓝调' },
  { value: 'high-key', label: '高调' },
  { value: 'low-key', label: '低调' },
  { value: 'silhouette', label: '剪影' },
  { value: 'neon', label: '霓虹' },
];

const COLOR_GRADE_OPTIONS: Array<{ value: ShotColorGrade; label: string }> = [
  { value: 'warm', label: '暖' },
  { value: 'cool', label: '冷' },
  { value: 'desaturated', label: '灰' },
  { value: 'high-saturation', label: '饱和' },
  { value: 'noir', label: '黑白' },
  { value: 'pastel', label: '粉彩' },
];

function SegmentedControl<T extends string>({
  label, value, options, onChange,
}: {
  label: string;
  value: T;
  options: Array<{ value: T; label: string; emoji?: string }>;
  onChange: (v: T) => void;
}) {
  return (
    <div>
      <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-ink-2">{label}</div>
      <div className="flex flex-wrap gap-1">
        {options.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => onChange(opt.value)}
            className={[
              'rounded-md px-2 py-1 text-xs',
              value === opt.value
                ? 'bg-accent text-bg'
                : 'bg-surface text-ink-2 hover:bg-surface-2 border border-line',
            ].join(' ')}
          >
            {opt.emoji && <span className="mr-0.5">{opt.emoji}</span>}
            {opt.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export default function CinematographyPanel({ value, onChange }: CinematographyPanelProps) {
  const v = value ?? {
    shot_type: 'medium', angle: 'eye-level', movement: 'static',
    transition_in: 'cut', transition_out: 'cut',
    lighting: 'natural', color_grade: 'warm',
  };
  return (
    <div>
      <h4 className="mb-2 text-xs font-semibold text-ink-2">🎥 镜头语言 (6 维)</h4>
      <div className="space-y-2 rounded-lg bg-surface p-3">
        <SegmentedControl label="景别" value={v.shot_type} options={SHOT_TYPE_OPTIONS} onChange={(c) => onChange({ shot_type: c })} />
        <SegmentedControl label="角度" value={v.angle} options={ANGLE_OPTIONS} onChange={(c) => onChange({ angle: c })} />
        <SegmentedControl label="运镜" value={v.movement} options={MOVEMENT_OPTIONS} onChange={(c) => onChange({ movement: c })} />
        <div className="grid grid-cols-2 gap-2">
          <SegmentedControl label="入场转场" value={v.transition_in} options={TRANSITION_OPTIONS} onChange={(c) => onChange({ transition_in: c })} />
          <SegmentedControl label="出场转场" value={v.transition_out} options={TRANSITION_OPTIONS} onChange={(c) => onChange({ transition_out: c })} />
        </div>
        <div className="grid grid-cols-2 gap-2">
          <SegmentedControl label="光线" value={v.lighting} options={LIGHTING_OPTIONS} onChange={(c) => onChange({ lighting: c })} />
          <SegmentedControl label="调色" value={v.color_grade} options={COLOR_GRADE_OPTIONS} onChange={(c) => onChange({ color_grade: c })} />
        </div>
      </div>
    </div>
  );
}