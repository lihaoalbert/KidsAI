import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import OptionCards from './OptionCards';

describe('OptionCards', () => {
  it('renders a visual option card and keeps it clickable', () => {
    const onPick = vi.fn();
    const card = {
      id: 'char_xiaoqi',
      label: '小启',
      value: 'char::xiaoqi',
      imageUrl: 'https://assets.kids.ibi.ren/character/xiaoqi.stand.png',
      imageAlt: '小启标准照',
    };

    render(<OptionCards cards={[card]} onPick={onPick} />);

    expect(screen.getByRole('img', { name: '小启标准照' }).getAttribute('src')).toBe(
      card.imageUrl,
    );
    fireEvent.click(screen.getByRole('button', { name: /小启/ }));
    expect(onPick).toHaveBeenCalledWith(card);
  });

  it('keeps text-only cards compact', () => {
    render(
      <OptionCards
        cards={[{ id: 'ok', label: '确认', value: '__confirm__', emoji: '✅' }]}
        onPick={vi.fn()}
      />,
    );

    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByRole('button', { name: /确认/ })).toBeDefined();
  });
});
