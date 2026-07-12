import { useEffect, useRef, useState } from 'react';
import { useStudioStore } from '../../stores/studioStore';
import ProgressMap from './ProgressMap';
import ChatBubble from './ChatBubble';
import OptionCards from './OptionCards';
import StoryCard from './StoryCard';

export default function ConversationPane() {
  const items = useStudioStore((s) => s.items);
  const phase = useStudioStore((s) => s.phase);
  const awaitingFree = useStudioStore((s) => s.awaitingFree);
  const pick = useStudioStore((s) => s.pick);
  const submitFree = useStudioStore((s) => s.submitFree);

  const [draft, setDraft] = useState('');
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
  }, [items]);

  useEffect(() => {
    if (awaitingFree) inputRef.current?.focus();
  }, [awaitingFree]);

  // 最新一张故事卡（阶段1）才可操作
  const lastStoryIdx = (() => {
    for (let i = items.length - 1; i >= 0; i--) if (items[i].kind === 'story') return i;
    return -1;
  })();

  const send = () => {
    if (!draft.trim()) return;
    submitFree(draft);
    setDraft('');
  };

  return (
    <div className="flex h-full flex-col bg-warm-50/40">
      <div className="border-b border-gray-100 bg-white/80 backdrop-blur">
        <ProgressMap />
      </div>

      <div ref={scrollRef} className="flex-1 space-y-3 overflow-auto px-5 py-6">
        {items.map((item, idx) => {
          switch (item.kind) {
            case 'ai':
            case 'kid':
              return <ChatBubble key={item.id} role={item.kind} text={item.text} />;
            case 'system':
              return <ChatBubble key={item.id} role="system" text={item.text} loading={item.loading} />;
            case 'cards':
              return (
                <OptionCards key={item.id} cards={item.cards} answered={item.answered} onPick={pick} />
              );
            case 'story':
              return (
                <StoryCard key={item.id} active={idx === lastStoryIdx && phase === 'stage1'} />
              );
            default:
              return null;
          }
        })}
      </div>

      <div className="border-t border-gray-100 bg-white px-4 py-3">
        <div className="flex items-center gap-2">
          <input
            ref={inputRef}
            value={draft}
            disabled={!awaitingFree}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && send()}
            placeholder={awaitingFree ? '打字告诉我，或按回车～' : '点上面的选项卡，或按 🎤 我自己说'}
            className="flex-1 rounded-2xl border-2 border-gray-200 bg-gray-50 px-4 py-2.5 text-sm focus:border-brand-400 focus:bg-white focus:outline-none disabled:cursor-not-allowed"
          />
          <button
            onClick={send}
            disabled={!awaitingFree || !draft.trim()}
            className="rounded-2xl bg-brand-600 px-5 py-2.5 text-sm font-semibold text-white hover:bg-brand-700 disabled:opacity-40"
          >
            发送
          </button>
        </div>
      </div>
    </div>
  );
}
