// EditPanel 组件测试（W3.5）
// 覆盖：渲染坐标 / 点击 submit 调 onSubmit / 点击 cancel 调 onCancel

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import EditPanel from './EditPanel';
import type { AgentAsset } from '../api/tauri';

const sampleAsset: AgentAsset = {
  type: 'image',
  url: 'https://example.com/cat.jpg',
  prompt: '一只小猫',
  tool: 'generate_image',
  tokensCost: 10,
};

describe('EditPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('渲染：显示点击位置（归一化百分比）+ 原图', () => {
    render(
      <EditPanel
        asset={sampleAsset}
        clickX={0.45}
        clickY={0.3}
        onSubmit={() => {}}
        onCancel={() => {}}
      />,
    );

    // 坐标显示为百分比
    expect(screen.getByText(/45%/)).toBeDefined();
    expect(screen.getByText(/30%/)).toBeDefined();
    // 原 prompt 提示
    expect(screen.getByText(/一只小猫/)).toBeDefined();
    // 输入框 placeholder
    expect(screen.getByPlaceholderText(/改成蓝色/)).toBeDefined();
  });

  it('Submit: 点击按钮触发 onSubmit，传入 trimmed 输入', () => {
    const onSubmit = vi.fn();
    render(
      <EditPanel
        asset={sampleAsset}
        clickX={0.5}
        clickY={0.5}
        onSubmit={onSubmit}
        onCancel={() => {}}
      />,
    );

    const textarea = screen.getByPlaceholderText(/改成蓝色/) as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: '  把裙子改成蓝色  ' } });

    const submitBtn = screen.getByRole('button', { name: /生成/ });
    fireEvent.click(submitBtn);

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith('把裙子改成蓝色');
  });

  it('Submit: 空输入时不触发 onSubmit', () => {
    const onSubmit = vi.fn();
    render(
      <EditPanel
        asset={sampleAsset}
        clickX={0.5}
        clickY={0.5}
        onSubmit={onSubmit}
        onCancel={() => {}}
      />,
    );

    const submitBtn = screen.getByRole('button', { name: /生成/ });
    fireEvent.click(submitBtn);

    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('Cancel: 点击取消按钮触发 onCancel', () => {
    const onCancel = vi.fn();
    render(
      <EditPanel
        asset={sampleAsset}
        clickX={0.5}
        clickY={0.5}
        onSubmit={() => {}}
        onCancel={onCancel}
      />,
    );

    const cancelBtn = screen.getByRole('button', { name: /取消/ });
    fireEvent.click(cancelBtn);

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('Cancel: 点击 backdrop 也触发 onCancel', () => {
    const onCancel = vi.fn();
    const { container } = render(
      <EditPanel
        asset={sampleAsset}
        clickX={0.5}
        clickY={0.5}
        onSubmit={() => {}}
        onCancel={onCancel}
      />,
    );

    // backdrop 是第一个 fixed inset-0 的 div
    const backdrop = container.querySelector('[aria-label="关闭编辑面板"]');
    expect(backdrop).toBeTruthy();
    fireEvent.click(backdrop!);

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('Cancel: 按 ESC 键触发 onCancel', () => {
    const onCancel = vi.fn();
    render(
      <EditPanel
        asset={sampleAsset}
        clickX={0.5}
        clickY={0.5}
        onSubmit={() => {}}
        onCancel={onCancel}
      />,
    );

    fireEvent.keyDown(window, { key: 'Escape' });

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('disabled 状态下 submit 按钮不响应', () => {
    const onSubmit = vi.fn();
    render(
      <EditPanel
        asset={sampleAsset}
        clickX={0.5}
        clickY={0.5}
        disabled={true}
        onSubmit={onSubmit}
        onCancel={() => {}}
      />,
    );

    const textarea = screen.getByPlaceholderText(/改成蓝色/) as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: '改色' } });

    const submitBtn = screen.getByRole('button', { name: /生成中/ });
    expect(submitBtn).toBeDefined();
    fireEvent.click(submitBtn);

    expect(onSubmit).not.toHaveBeenCalled();
  });
});