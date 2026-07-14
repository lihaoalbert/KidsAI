import { useEffect, useRef, useState } from 'react';
import { useStudioStore } from '../../stores/studioStore';
import { useDirectorStore } from '../../stores/directorStore';
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
  // W9: 长程 chat — agent 永远倾听，输入框常驻 enabled
  const chat = useDirectorStore((s) => s.chat);

  const [draft, setDraft] = useState('');
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
  }, [items]);

  useEffect(() => {
    inputRef.current?.focus();
  }, [awaitingFree]);

  // 最新一张故事卡（阶段1）才可操作
  const lastStoryIdx = (() => {
    for (let i = items.length - 1; i >= 0; i--) if (items[i].kind === 'story') return i;
    return -1;
  })();

  const send = () => {
    const text = draft.trim();
    if (!text) return;
    // W9: 输入框永远可用 — awaitingFree 走原 funnel，否则写 chatHistory 让 agent 听见
    if (awaitingFree) submitFree(text);
    else chat(text);
    setDraft('');
  };

  return (
    <div className="flex h-full flex-col bg-bg/40">
      <div className="border-b border-line bg-surface/80 backdrop-blur">
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
              // W9: 卡片是建议，不是 funnel 锁 — answered 仍可点
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

      <div className="border-t border-line bg-surface px-4 py-3">
        <div className="flex items-center gap-2">
          <input
            ref={inputRef}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && send()}
            placeholder={
              awaitingFree
                ? '打字告诉我，或按回车～'
                : '随时告诉我你的想法，Agent 一直都在听 🎧'
            }
            className="flex-1 rounded-2xl border-2 border-line bg-surface-2 px-4 py-2.5 text-sm focus:border-accent focus:bg-surface focus:outline-none"
          />
          <button
            onClick={send}
            disabled={!draft.trim()}
            className="rounded-2xl bg-accent px-5 py-2.5 text-sm font-semibold text-bg hover:bg-accent-hover disabled:opacity-40"
          >
            发送
          </button>
        </div>
      </div>
    </div>
  );
}
