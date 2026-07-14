// Pet Corner — DESIGN.md §5.6 (Forest 陪伴)
//
// 唯一的常驻角落 (bottom-right, 56px 圆形). 后端 PetEngine 通过
// useAgentStore.petMood / petRecall 推数据;本组件只读, 不维护 shadow state.
// - §5.6.2: 三种 mood → 不同渐变背景 (happy / sleepy / thinking)
// - §5.6.4: Recall 气泡 = honey-soft 底 + Forest ink 字, 6s 自动关
// - §5.6.5: 本组件只在 Forest (child) 模式下挂载, 父级 App.tsx 控制
// - §8.2: breathing 4s 循环 (animate-pet-breathe, 已在 globals.css 注册, 尊重 reduced-motion)
// - §8.4: 仅 transform / opacity
//
// 不做 (留 M2 / 后续): 3D 立绘 standee, bounded wandering path, 多等级.
// 现在仍用 🔥 emoji stand-in — 资产准备好后只需替 emoji 节点.

import { useEffect, useState } from 'react';
import { useAgentStore } from '../../stores/agentStore';
import { useUserModeStore } from '../../stores/userModeStore';

interface PetCornerProps {
  onNavigate?: (page: 'home' | 'library') => void;
}

const PET_EMOJI: Record<string, string> = {
  huomiao: '🔥',
};

const PET_NAME: Record<string, string> = {
  huomiao: '火苗',
};

// mood → tailwind gradient (token 化, 全部走 --c-highlight / --c-accent)
const MOOD_GRADIENT: Record<string, string> = {
  happy: 'bg-gradient-to-br from-highlight to-highlight/60',
  sleepy: 'bg-gradient-to-br from-highlight/40 to-highlight-soft',
  thinking: 'bg-gradient-to-br from-accent-soft to-highlight/70',
};

// mood → 一个字符标签 (a11y 用)
const MOOD_LABEL: Record<string, string> = {
  happy: '开心',
  sleepy: '犯困',
  thinking: '在想事情',
};

export default function PetCorner(_props: PetCornerProps) {
  // 从 agentStore 读后端 PetEngine 真值 — petStore 留作 M2 等级系统的本地补充
  const petMood = useAgentStore((s) => s.petMood);
  const petRecall = useAgentStore((s) => s.petRecall);
  const clearPetRecall = useAgentStore((s) => s.clearPetRecall);
  const mode = useUserModeStore((s) => s.mode);

  // petId: 目前 onboarding 写死 huomiao, 后续 persona 多样化时改读 Identity.petId
  const [petId] = useState<string>('huomiao');

  // Recall 气泡自管理显示 — 6s 后自动调 clearPetRecall
  useEffect(() => {
    if (!petRecall) return;
    const t = setTimeout(() => {
      clearPetRecall();
    }, 6000);
    return () => clearTimeout(t);
  }, [petRecall, clearPetRecall]);

  // petId 走白名单 — 不认识的 pet 兜底回 huomiao, 防止后端乱推数据炸 UI
  const emoji = PET_EMOJI[petId] ?? PET_EMOJI.huomiao;
  const name = PET_NAME[petId] ?? PET_NAME.huomiao;
  const gradient = MOOD_GRADIENT[petMood] ?? MOOD_GRADIENT.happy;
  const moodLabel = MOOD_LABEL[petMood] ?? MOOD_LABEL.happy;

  const handleClick = () => {
    if (petRecall) clearPetRecall();
  };

  const showRecall = petRecall !== null;

  // Coast 模式冗余防御 — App.tsx 已不挂载, 这里再 return null 保证视觉一致
  if (mode === 'adult') return null;

  return (
    <div
      className="fixed bottom-4 right-4 z-40 select-none"
      data-testid="pet-corner"
      data-mode={mode}
    >
      {/* Recall 气泡 — §5.6.4: 弹出 + wiggle 一次 */}
      {showRecall && (
        <div
          role="status"
          aria-live="polite"
          className="absolute bottom-20 right-0 w-44 rounded-2xl bg-surface px-3 py-2 text-xs text-ink shadow-lg border border-highlight-soft animate-pet-pop-in"
        >
          <div className="font-semibold text-highlight mb-1">{name}</div>
          <div>{petRecall}</div>
          <div className="absolute -bottom-1.5 right-6 h-3 w-3 rotate-45 border-r border-b border-highlight-soft bg-surface" />
        </div>
      )}

      {/* Pet standee — §8.2 breathing 4s 循环 */}
      <button
        type="button"
        onClick={handleClick}
        title={`${name} · ${moodLabel}`}
        aria-label={`宠物 ${name}, 当前心情 ${moodLabel}`}
        className={[
          'group relative flex h-14 w-14 items-center justify-center rounded-full',
          'shadow-lg hover:shadow-xl transition-shadow duration-300',
          'active:scale-95',
          'animate-pet-breathe',
          gradient,
        ].join(' ')}
      >
        {/* 主 emoji stand-in — 资产到位后换成 SVG / 图片 */}
        <span
          className={[
            'text-3xl drop-shadow-md transition-transform duration-300',
            showRecall ? 'animate-pet-wiggle' : 'group-hover:scale-110',
          ].join(' ')}
          aria-hidden
        >
          {emoji}
        </span>
        {/* 名字标签 */}
        <span className="absolute -bottom-1.5 left-1/2 -translate-x-1/2 text-[10px] font-bold text-highlight bg-surface px-1.5 py-0.5 rounded-full whitespace-nowrap shadow-sm">
          {name}
        </span>
      </button>
    </div>
  );
}