// W10 Day 4 — ParentPinDialog 单测
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ParentPinDialog } from './ParentPinDialog';

// mock tauri api
vi.mock('../../api/tauri', () => ({
  isParentPinSet: vi.fn(),
  setParentPin: vi.fn(),
  verifyParentPin: vi.fn(),
}));

import * as tauri from '../../api/tauri';

describe('ParentPinDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders 4 pin dots when open', async () => {
    (tauri.isParentPinSet as ReturnType<typeof vi.fn>).mockResolvedValue(true);
    render(<ParentPinDialog open={true} onSuccess={() => {}} onCancel={() => {}} />);
    await waitFor(() => {
      expect(screen.getAllByTestId('pin-dots')[0]).toBeTruthy();
    });
  });

  it('does not render when closed', () => {
    const { container } = render(
      <ParentPinDialog open={false} onSuccess={() => {}} onCancel={() => {}} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it('auto-detects setup mode when PIN not set', async () => {
    (tauri.isParentPinSet as ReturnType<typeof vi.fn>).mockResolvedValue(false);
    render(<ParentPinDialog open={true} onSuccess={() => {}} onCancel={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText(/设置 4 位数字 PIN/)).toBeTruthy();
    });
  });

  it('auto-detects verify mode when PIN already set', async () => {
    (tauri.isParentPinSet as ReturnType<typeof vi.fn>).mockResolvedValue(true);
    render(<ParentPinDialog open={true} onSuccess={() => {}} onCancel={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText(/输入 4 位数字 PIN/)).toBeTruthy();
    });
  });

  it('verify: wrong PIN shows error', async () => {
    (tauri.isParentPinSet as ReturnType<typeof vi.fn>).mockResolvedValue(true);
    (tauri.verifyParentPin as ReturnType<typeof vi.fn>).mockResolvedValue(false);
    render(<ParentPinDialog open={true} onSuccess={() => {}} onCancel={() => {}} />);
    await waitFor(() => screen.getByTestId('pin-key-1'));
    fireEvent.click(screen.getByTestId('pin-key-1'));
    fireEvent.click(screen.getByTestId('pin-key-2'));
    fireEvent.click(screen.getByTestId('pin-key-3'));
    fireEvent.click(screen.getByTestId('pin-key-4'));
    fireEvent.click(screen.getByTestId('pin-submit'));
    await waitFor(() => {
      expect(screen.getByTestId('pin-error').textContent).toMatch(/PIN 错误/);
    });
  });

  it('verify: correct PIN calls onSuccess with pin', async () => {
    (tauri.isParentPinSet as ReturnType<typeof vi.fn>).mockResolvedValue(true);
    (tauri.verifyParentPin as ReturnType<typeof vi.fn>).mockResolvedValue(true);
    const onSuccess = vi.fn();
    render(<ParentPinDialog open={true} onSuccess={onSuccess} onCancel={() => {}} />);
    await waitFor(() => screen.getByTestId('pin-key-1'));
    fireEvent.click(screen.getByTestId('pin-key-1'));
    fireEvent.click(screen.getByTestId('pin-key-2'));
    fireEvent.click(screen.getByTestId('pin-key-3'));
    fireEvent.click(screen.getByTestId('pin-key-4'));
    fireEvent.click(screen.getByTestId('pin-submit'));
    await waitFor(() => {
      expect(onSuccess).toHaveBeenCalledWith('1234');
    });
  });

  it('cancel button calls onCancel', async () => {
    (tauri.isParentPinSet as ReturnType<typeof vi.fn>).mockResolvedValue(true);
    const onCancel = vi.fn();
    render(<ParentPinDialog open={true} onSuccess={() => {}} onCancel={onCancel} />);
    await waitFor(() => screen.getByText('取消'));
    fireEvent.click(screen.getByText('取消'));
    expect(onCancel).toHaveBeenCalled();
  });

  it('locks after 3 wrong attempts', async () => {
    (tauri.isParentPinSet as ReturnType<typeof vi.fn>).mockResolvedValue(true);
    (tauri.verifyParentPin as ReturnType<typeof vi.fn>).mockResolvedValue(false);
    render(<ParentPinDialog open={true} onSuccess={() => {}} onCancel={() => {}} />);
    await waitFor(() => screen.getByTestId('pin-key-1'));
    for (let attempt = 0; attempt < 3; attempt++) {
      for (const d of [1, 2, 3, 4]) {
        fireEvent.click(screen.getByTestId(`pin-key-${d}`));
      }
      fireEvent.click(screen.getByTestId('pin-submit'));
      await waitFor(() => {
        expect(tauri.verifyParentPin).toHaveBeenCalledTimes(attempt + 1);
      });
    }
    // 第 3 次失败后锁定
    await waitFor(() => {
      expect(screen.getByTestId('lockout')).toBeTruthy();
    });
  });
});