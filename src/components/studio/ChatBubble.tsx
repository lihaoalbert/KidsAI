import Skeleton from '../ui/Skeleton';

interface ChatBubbleProps {
  role: 'ai' | 'kid' | 'system';
  text: string;
  loading?: boolean;
}

export default function ChatBubble({ role, text, loading }: ChatBubbleProps) {
  if (role === 'system') {
    return (
      <div className="flex justify-center my-1">
        <div className="inline-flex items-center gap-2 rounded-full bg-surface-2 px-3 py-1.5 text-xs text-ink-2">
          {loading && <Skeleton variant="text" lines={1} className="w-24" />}
          <span className="whitespace-pre-wrap">{text}</span>
        </div>
      </div>
    );
  }

  const isKid = role === 'kid';
  return (
    <div className={['flex items-end gap-2', isKid ? 'flex-row-reverse' : 'flex-row'].join(' ')}>
      <div
        className={[
          'flex h-9 w-9 shrink-0 items-center justify-center rounded-full text-lg',
          isKid ? 'bg-accent-soft' : 'bg-accent-soft-2',
        ].join(' ')}
      >
        {isKid ? '🧒' : '🤖'}
      </div>
      <div
        className={[
          'max-w-[78%] rounded-2xl px-4 py-2.5 text-sm leading-relaxed whitespace-pre-wrap',
          isKid
            ? 'bg-highlight text-bg rounded-br-sm'
            : 'bg-surface text-ink-2 border border-line shadow-sm rounded-bl-sm',
        ].join(' ')}
      >
        {text}
      </div>
    </div>
  );
}
