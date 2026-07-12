// 学币余额状态 (W4.5 B2: 保留本地乐观记账 + server 同步)
//
// 设计:
// - 本地 store 继续做 optimistic UI 扣/退 (directorStore 的 spend→call→refund 流程依赖此)
// - W4.5 B2: 新增 setBalance, 供 BalanceWidget 用 server /me/balance 真实值覆盖本地
// - server 是 authoritative, 后台 record_spend fire-and-forget 上报;
//   UI 短暂不一致可接受, 下一轮 sync 自动校正
// - 余额 < 0 时 server 返 402, UI 弹"找管理员加学币"

import { create } from 'zustand';

interface TokenState {
  balance: number;
  earned: number;
  spent: number;
  /// W4.5 B2: server 同步入口 — BalanceWidget 拉 /me/balance 后调
  setBalance: (amount: number) => void;
  addTokens: (amount: number) => void;
  spendTokens: (amount: number) => boolean;
  reset: () => void;
}

const DEFAULT_BALANCE = 500;

export const useTokenStore = create<TokenState>((set, get) => ({
  balance: DEFAULT_BALANCE,
  earned: 0,
  spent: 0,
  setBalance: (amount) => set({ balance: amount }),
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
  reset: () => set({ balance: DEFAULT_BALANCE, earned: 0, spent: 0 }),
}));
