// W10 Day 4 — userModeStore 单测
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { useUserModeStore } from './userModeStore';

vi.mock('../api/tauri', () => ({
  getUserMode: vi.fn(),
  setUserMode: vi.fn(),
}));

import * as tauri from '../api/tauri';

describe('userModeStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useUserModeStore.setState({
      mode: 'child',
      loaded: false,
      lastSwitchedAt: null,
      switching: false,
      error: null,
    });
  });

  it('initial state is child mode not loaded', () => {
    const s = useUserModeStore.getState();
    expect(s.mode).toBe('child');
    expect(s.loaded).toBe(false);
  });

  it('load() updates mode from server', async () => {
    (tauri.getUserMode as ReturnType<typeof vi.fn>).mockResolvedValue('adult');
    await useUserModeStore.getState().load();
    const s = useUserModeStore.getState();
    expect(s.mode).toBe('adult');
    expect(s.loaded).toBe(true);
  });

  it('load() failure keeps default child but marks loaded', async () => {
    (tauri.getUserMode as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('no license'));
    await useUserModeStore.getState().load();
    const s = useUserModeStore.getState();
    expect(s.mode).toBe('child');
    expect(s.loaded).toBe(true);
  });

  it('switchTo() updates mode on success', async () => {
    (tauri.setUserMode as ReturnType<typeof vi.fn>).mockResolvedValue({
      deviceId: 'd',
      mode: 'adult',
      switchedAt: 1718234567890,
    });
    await useUserModeStore.getState().switchTo('adult', '1234');
    const s = useUserModeStore.getState();
    expect(s.mode).toBe('adult');
    expect(s.lastSwitchedAt).toBe(1718234567890);
    expect(s.switching).toBe(false);
  });

  it('switchTo() records error on failure', async () => {
    (tauri.setUserMode as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('wrong pin'));
    await expect(
      useUserModeStore.getState().switchTo('adult', '9999'),
    ).rejects.toThrow('wrong pin');
    const s = useUserModeStore.getState();
    expect(s.error).toBe('wrong pin');
    expect(s.switching).toBe(false);
  });

  it('clearError() clears error', () => {
    useUserModeStore.setState({ error: 'old error' });
    useUserModeStore.getState().clearError();
    expect(useUserModeStore.getState().error).toBeNull();
  });
});