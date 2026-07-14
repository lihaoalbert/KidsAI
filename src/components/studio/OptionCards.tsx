import type { OptionCard } from '../../data/videoScript';

interface OptionCardsProps {
  cards: OptionCard[];
  /** 已选 label — 仅作视觉高亮，不再锁死整组（W9: 卡片是建议，可点可改） */
  answered?: string;
  onPick: (card: OptionCard) => void;
}

export default function OptionCards({ cards, answered, onPick }: OptionCardsProps) {
  const isChosenLabel = answered !== undefined
    ? cards.find((c) => `${c.emoji ?? ''}${c.label}` === answered)?.id
    : undefined;
  return (
    <div className="flex flex-wrap gap-2 pl-11">
      {cards.map((card) => {
        const label = `${card.emoji ? card.emoji + ' ' : ''}${card.label}`;
        const isChosen = card.id === isChosenLabel;
        const hasPreview = Boolean(card.imageUrl);
        return (
          <button
            key={card.id}
            type="button"
            onClick={() => onPick(card)}
            className={[
              'overflow-hidden rounded-2xl border-2 text-sm font-semibold transition-all',
              hasPreview ? 'w-32 p-2' : 'px-4 py-2.5',
              isChosen
                ? 'border-accent bg-accent text-bg shadow-md'
                : 'border-accent-line bg-surface text-accent-ink hover:-translate-y-0.5 hover:border-accent hover:bg-accent-soft active:translate-y-0',
            ].join(' ')}
          >
            {card.imageUrl && (
              <img
                src={card.imageUrl}
                alt={card.imageAlt ?? card.label}
                className="mb-2 h-24 w-full rounded-xl object-cover"
              />
            )}
            <span className={hasPreview ? 'block truncate px-1 text-center' : undefined}>{label}</span>
          </button>
        );
      })}
    </div>
  );
}
