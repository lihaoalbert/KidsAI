// P1-2: 轻量 toast store — 解决 spend 失败 / 错误 静默问题.
//
// 设计: zustand 单例 + 队列, ToastHost 自动渲染 + 4s 后 dismiss.
// 复用模式: 任何地方 `useToastStore.getState().push('没拍成...', 'error')`,
// 不用关心组件树位置. 旧 chat 路径继续保留, 不破坏现有 pushAi 流程.

import { create } from 'zustand';

export type ToastLevel = 'info' | 'warn' | 'error' | 'success';

export interface Toast {
  id: string;
  level: ToastLevel;
  text: string;
  createdAt: number;
}

interface ToastState {
  toasts: Toast[];
  push: (text: string, level?: ToastLevel) => string;
  dismiss: (id: string) => void;
}

const DEFAULT_TTL_MS = 4000;
const MAX_TOASTS = 4;

let toastSeq = 0;
const nextId = () => `t-${++toastSeq}-${Date.now().toString(36)}`;

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  push: (text, level = 'info') => {
    const id = nextId();
    const toast: Toast = { id, level, text, createdAt: Date.now() };
    set((s) => {
      const next = [...s.toasts, toast];
      if (next.length > MAX_TOASTS) next.shift();
      return { toasts: next };
    });
    setTimeout(() => get().dismiss(id), DEFAULT_TTL_MS);
    return id;
  },
  dismiss: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));
