// P0 fix: Pet MVP - 火苗 MVP, 在右下角常驻
// M1 起步: 单只宠物 + 基础呼吸动画 + hover 显示心情
// 不做: 完整等级 / 跨设备同步 / 复杂状态机 / 主动问候 (留给 M2)

import { useState } from 'react';
import { usePetStore } from '../../stores/petStore';

interface PetCornerProps {
  onNavigate?: (page: 'home' | 'library') => void;
}

const PET_EMOJI: Record<string, string> = {
  huomiao: '🔥', // 火苗 (默认 - 小月)
};

const PET_NAME: Record<string, string> = {
  huomiao: '火苗',
};

const PET_MESSAGES: Record<string, string[]> = {
  huomiao: [
    '咿呀~ 我在呢',
    '今天想做啥?',
    '我明天还在哦',
    '你不在的时候我会想你的',
  ],
};

export default function PetCorner(_props: PetCornerProps) {
  const petId = usePetStore((s) => s.petId);
  const mood = usePetStore((s) => s.mood);
  const lastSeen = usePetStore((s) => s.lastSeenAt);
  const bumpLastSeen = usePetStore((s) => s.bumpLastSeen);
  const [showBubble, setShowBubble] = useState(false);

  const emoji = PET_EMOJI[petId] ?? '🔥';
  const name = PET_NAME[petId] ?? '火苗';
  const daysSince = Math.floor((Date.now() - lastSeen) / 86_400_000);

  const handleClick = () => {
    bumpLastSeen();
    setShowBubble(true);
    setTimeout(() => setShowBubble(false), 3000);
  };

  // 根据心情选择呼吸色
  const moodColor: Record<string, string> = {
    happy: 'from-highlight to-yellow-400',
    sleepy: 'from-highlight/80 to-highlight-soft',
    thinking: 'from-accent-soft to-highlight/80',
  };
  const gradient = moodColor[mood] ?? moodColor.happy;

  return (
    <div className="fixed bottom-4 right-4 z-40 select-none">
      {/* Bubble */}
      {showBubble && (
        <div className="absolute bottom-20 right-0 w-44 rounded-2xl bg-surface px-3 py-2 text-xs text-ink-2 shadow-lg border border-highlight-soft animate-fade-in">
          <div className="font-semibold text-highlight mb-1">{name}</div>
          <div>{PET_MESSAGES[petId]?.[0] ?? '咿呀~'}</div>
          <div className="absolute -bottom-2 right-6 h-3 w-3 rotate-45 border-r border-b border-highlight-soft bg-surface" />
        </div>
      )}

      {/* Pet */}
      <button
        type="button"
        onClick={handleClick}
        title={`${name} - ${mood}${daysSince > 0 ? ` (${daysSince} 天没见)` : ''}`}
        className={[
          'group relative flex h-20 w-20 items-center justify-center rounded-full',
          'bg-gradient-to-br shadow-lg hover:shadow-xl transition-all duration-300',
          'animate-pulse-slow active:scale-95',
          gradient,
        ].join(' ')}
        aria-label={`宠物 ${name}, 当前心情 ${mood}`}
      >
        {/* 头顶小火苗呆毛 (装饰) */}
        <span className="absolute -top-1 left-1/2 -translate-x-1/2 text-2xl animate-flicker">
          ✨
        </span>
        {/* 主 emoji */}
        <span className="text-4xl drop-shadow-md group-hover:scale-110 transition-transform">
          {emoji}
        </span>
        {/* 名字标签 */}
        <span className="absolute -bottom-1 left-1/2 -translate-x-1/2 text-[10px] font-bold text-highlight bg-surface/90 px-1.5 py-0.5 rounded-full">
          {name}
        </span>
      </button>

      <style>{`
        @keyframes pulse-slow {
          0%, 100% { transform: scale(1); box-shadow: 0 0 0 0 rgba(255, 165, 0, 0.4); }
          50% { transform: scale(1.05); box-shadow: 0 0 0 8px rgba(255, 165, 0, 0); }
        }
        @keyframes flicker {
          0%, 100% { opacity: 1; transform: translateX(-50%) translateY(0); }
          50% { opacity: 0.7; transform: translateX(-50%) translateY(-2px); }
        }
        @keyframes fade-in {
          from { opacity: 0; transform: translateY(8px); }
          to { opacity: 1; transform: translateY(0); }
        }
        .animate-pulse-slow { animation: pulse-slow 3s ease-in-out infinite; }
        .animate-flicker { animation: flicker 1.5s ease-in-out infinite; }
        .animate-fade-in { animation: fade-in 0.3s ease-out; }
      `}</style>
    </div>
  );
}