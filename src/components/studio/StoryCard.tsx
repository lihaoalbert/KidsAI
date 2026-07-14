import { useState } from 'react';
import { useDirectorStore, type StorySlot } from '../../stores/directorStore';
import { useStudioStore } from '../../stores/studioStore';

const SLOT_META: { slot: StorySlot; emoji: string; label: string }[] = [
  { slot: 'who', emoji: '🐉', label: '主角' },
  { slot: 'wants', emoji: '🎯', label: '它想' },
  { slot: 'but', emoji: '⛰️', label: '但是' },
  { slot: 'ending', emoji: '💫', label: '最后' },
];

interface StoryCardProps {
  active: boolean; // 仅最新且仍在阶段1时可操作
}

export default function StoryCard({ active }: StoryCardProps) {
  const story = useDirectorStore((s) => s.story);
  const confirmStory = useStudioStore((s) => s.confirmStory);
  const reEditSlot = useStudioStore((s) => s.reEditSlot);
  const dice = useStudioStore((s) => s.dice);
  const [editing, setEditing] = useState(false);

  return (
    <div className="pl-11">
      <div className="max-w-md rounded-2xl border-2 border-accent-line bg-gradient-to-br from-accent-50 to-highlight/50 p-4 shadow-sm">
        <div className="mb-3 flex items-center gap-2 text-sm font-bold text-accent-ink">
          🎴 你的故事
        </div>
        <div className="space-y-2">
          {SLOT_META.map(({ slot, emoji, label }) => (
            <div key={slot} className="flex items-start gap-2 text-sm">
              <span className="w-14 shrink-0 font-semibold text-ink-2">
                {emoji} {label}
              </span>
              <span className="text-ink-2">{story[slot] || '…'}</span>
            </div>
          ))}
        </div>

        {active && !editing && (
          <div className="mt-4 flex flex-wrap gap-2">
            <button
              onClick={() => confirmStory()}
              className="rounded-xl bg-accent px-4 py-2 text-sm font-semibold text-bg hover:bg-accent-hover"
            >
              就这样开始! ✅
            </button>
            <button
              onClick={() => setEditing(true)}
              className="rounded-xl border-2 border-accent-line bg-surface px-4 py-2 text-sm font-semibold text-accent-ink hover:bg-accent-soft"
            >
              再改改 ✏️
            </button>
            <button
              onClick={() => dice()}
              className="rounded-xl border-2 border-line bg-surface px-4 py-2 text-sm font-semibold text-ink-2 hover:bg-surface-2"
            >
              🎲 换一个
            </button>
          </div>
        )}

        {active && editing && (
          <div className="mt-4">
            <div className="mb-2 text-xs font-semibold text-ink-2">想改哪一块？点一下就行～</div>
            <div className="flex flex-wrap gap-2">
              {SLOT_META.map(({ slot, emoji, label }) => (
                <button
                  key={slot}
                  onClick={() => {
                    setEditing(false);
                    reEditSlot(slot);
                  }}
                  className="rounded-xl border-2 border-accent-line bg-surface px-3 py-1.5 text-sm font-semibold text-accent-ink hover:bg-accent-soft"
                >
                  {emoji} {label}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
