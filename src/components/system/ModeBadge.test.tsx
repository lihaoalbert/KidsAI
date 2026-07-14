// W10 Day 4 — ModeBadge 单测
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ModeBadge } from './ModeBadge';
import { useUserModeStore } from '../../stores/userModeStore';

vi.mock('../../api/tauri', () => ({
  getUserMode: vi.fn(),
  setUserMode: vi.fn(),
}));

import * as tauri from '../../api/tauri';

describe('ModeBadge', () => {
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

  it('shows child mode badge', async () => {
    (tauri.getUserMode as ReturnType<typeof vi.fn>).mockResolvedValue('child');
    render(<ModeBadge />);
    await waitFor(() => {
      expect(screen.getByTestId('mode-badge').textContent).toMatch(/儿童模式/);
    });
  });

  it('shows adult mode badge when mode=adult', async () => {
    (tauri.getUserMode as ReturnType<typeof vi.fn>).mockResolvedValue('adult');
    render(<ModeBadge />);
    await waitFor(() => {
      expect(screen.getByTestId('mode-badge').textContent).toMatch(/成人模式/);
    });
  });

  it('clicking badge opens menu', async () => {
    (tauri.getUserMode as ReturnType<typeof vi.fn>).mockResolvedValue('child');
    render(<ModeBadge />);
    await waitFor(() => screen.getByTestId('mode-badge'));
    fireEvent.click(screen.getByTestId('mode-badge'));
    expect(screen.getByTestId('mode-badge-menu')).toBeTruthy();
  });

  it('menu shows switch trigger', async () => {
    (tauri.getUserMode as ReturnType<typeof vi.fn>).mockResolvedValue('child');
    render(<ModeBadge />);
    await waitFor(() => screen.getByTestId('mode-badge'));
    fireEvent.click(screen.getByTestId('mode-badge'));
    expect(screen.getByTestId('mode-switch-trigger').textContent).toMatch(
      /切到成人模式/,
    );
  });

  it('clicking switch trigger opens dialog', async () => {
    (tauri.getUserMode as ReturnType<typeof vi.fn>).mockResolvedValue('child');
    render(<ModeBadge />);
    await waitFor(() => screen.getByTestId('mode-badge'));
    fireEvent.click(screen.getByTestId('mode-badge'));
    fireEvent.click(screen.getByTestId('mode-switch-trigger'));
    await waitFor(() => {
      expect(screen.getByTestId('mode-switch-dialog')).toBeTruthy();
    });
  });

  it('navigate callback fires for skills', async () => {
    (tauri.getUserMode as ReturnType<typeof vi.fn>).mockResolvedValue('child');
    const onNavigate = vi.fn();
    render(<ModeBadge onNavigate={onNavigate} />);
    await waitFor(() => screen.getByTestId('mode-badge'));
    fireEvent.click(screen.getByTestId('mode-badge'));
    fireEvent.click(screen.getByTestId('mode-skills-link'));
    expect(onNavigate).toHaveBeenCalledWith('marketplace');
  });
});