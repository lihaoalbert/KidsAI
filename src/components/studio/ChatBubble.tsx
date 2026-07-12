interface ChatBubbleProps {
  role: 'ai' | 'kid' | 'system';
  text: string;
  loading?: boolean;
}

export default function ChatBubble({ role, text, loading }: ChatBubbleProps) {
  if (role === 'system') {
    return (
      <div className="flex justify-center my-1">
        <div className="inline-flex items-center gap-2 rounded-full bg-gray-100 px-3 py-1.5 text-xs text-gray-500">
          {loading && (
            <span className="inline-block w-3 h-3 border-2 border-brand-400 border-t-transparent rounded-full animate-spin" />
          )}
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
          isKid ? 'bg-warm-100' : 'bg-brand-100',
        ].join(' ')}
      >
        {isKid ? '🧒' : '🤖'}
      </div>
      <div
        className={[
          'max-w-[78%] rounded-2xl px-4 py-2.5 text-sm leading-relaxed whitespace-pre-wrap',
          isKid
            ? 'bg-warm-500 text-white rounded-br-sm'
            : 'bg-white text-gray-800 border border-gray-100 shadow-sm rounded-bl-sm',
        ].join(' ')}
      >
        {text}
      </div>
    </div>
  );
}
