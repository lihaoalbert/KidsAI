// P1-2: 验证 toast store 基本能力 — push / 队列上限 / dismiss.

import { describe, it, expect, beforeEach, vi } from 'vitest';

describe('toastStore', () => {
  beforeEach(async () => {
    vi.resetModules();
    const { useToastStore } = await import('./toastStore');
    useToastStore.setState({ toasts: [] });
  });

  it('push 加一条 toast, 4s 后自动消失', async () => {
    vi.useFakeTimers();
    const { useToastStore } = await import('./toastStore');
    const id = useToastStore.getState().push('hello', 'info');
    expect(useToastStore.getState().toasts).toHaveLength(1);
    expect(useToastStore.getState().toasts[0].text).toBe('hello');
    vi.advanceTimersByTime(4100);
    expect(useToastStore.getState().toasts).toHaveLength(0);
    expect(id).toMatch(/^t-/);
    vi.useRealTimers();
  });

  it('queue 上限 MAX_TOASTS=4, 超出 drop 最旧', async () => {
    const { useToastStore } = await import('./toastStore');
    useToastStore.getState().push('1');
    useToastStore.getState().push('2');
    useToastStore.getState().push('3');
    useToastStore.getState().push('4');
    useToastStore.getState().push('5');
    const texts = useToastStore.getState().toasts.map((t) => t.text);
    expect(texts).toEqual(['2', '3', '4', '5']);
  });

  it('level 默认 info, 可显式 error/warn/success', async () => {
    const { useToastStore } = await import('./toastStore');
    useToastStore.getState().push('a');
    useToastStore.getState().push('b', 'error');
    const ts = useToastStore.getState().toasts;
    expect(ts[0].level).toBe('info');
    expect(ts[1].level).toBe('error');
  });

  it('dismiss(id) 立刻移除', async () => {
    const { useToastStore } = await import('./toastStore');
    const id = useToastStore.getState().push('x');
    expect(useToastStore.getState().toasts).toHaveLength(1);
    useToastStore.getState().dismiss(id);
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });
});
