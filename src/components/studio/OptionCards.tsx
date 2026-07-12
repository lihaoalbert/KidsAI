import type { OptionCard } from '../../data/videoScript';

interface OptionCardsProps {
  cards: OptionCard[];
  answered?: string; // 已选 label → 整组禁用
  onPick: (card: OptionCard) => void;
}

export default function OptionCards({ cards, answered, onPick }: OptionCardsProps) {
  const locked = answered !== undefined;
  return (
    <div className="flex flex-wrap gap-2 pl-11">
      {cards.map((card) => {
        const label = `${card.emoji ? card.emoji + ' ' : ''}${card.label}`;
        const isChosen = locked && answered === `${card.emoji ?? ''}${card.label}`;
        return (
          <button
            key={card.id}
            type="button"
            disabled={locked}
            onClick={() => onPick(card)}
            className={[
              'rounded-2xl border-2 px-4 py-2.5 text-sm font-semibold transition-all',
              locked
                ? isChosen
                  ? 'border-brand-500 bg-brand-500 text-white'
                  : 'border-gray-100 bg-gray-50 text-gray-300'
                : 'border-brand-200 bg-white text-brand-700 hover:border-brand-400 hover:bg-brand-50 hover:-translate-y-0.5 active:translate-y-0',
            ].join(' ')}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}
