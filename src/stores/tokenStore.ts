// 学币余额状态（MVP 阶段纯前端模拟，后续接入后端账户）

import { create } from 'zustand';

interface TokenState {
  balance: number;
  earned: number;
  spent: number;
  addTokens: (amount: number) => void;
  spendTokens: (amount: number) => boolean; // 余额不足返回 false
  reset: () => void;
}

export const useTokenStore = create<TokenState>((set, get) => ({
  balance: 500,
  earned: 0,
  spent: 0,
  addTokens: (amount) =>
    set((s) => ({
      balance: s.balance + amount,
      earned: s.earned + amount,
    })),
  spendTokens: (amount) => {
    const { balance } = get();
    if (balance < amount) return false;
    set((s) => ({
      balance: s.balance - amount,
      spent: s.spent + amount,
    }));
    return true;
  },
  reset: () => set({ balance: 500, earned: 0, spent: 0 }),
}));
