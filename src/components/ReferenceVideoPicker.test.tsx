// ReferenceVideoPicker 组件测试（W3.7+）
// 覆盖:超限文件校验 / 帧数变化重抽 / onChange 时机

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import ReferenceVideoPicker from './ReferenceVideoPicker';

// Mock frameExtractor 模块,避免在 jsdom 跑真视频逻辑
const extractMock = vi.fn();
vi.mock('../utils/frameExtractor', async () => {
  const actual = await vi.importActual<typeof import('../utils/frameExtractor')>(
    '../utils/frameExtractor',
  );
  return {
    ...actual,
    extractFramesAtTimestamps: (...args: unknown[]) => extractMock(...args),
  };
});

function makeFile(name: string, sizeBytes: number, type: string = 'video/mp4'): File {
  const blob = new Blob([new ArrayBuffer(sizeBytes)], { type });
  return new File([blob], name, { type });
}

function makeDataUrl(n: number) {
  return `data:image/jpeg;base64,FRAME_${n}`;
}

describe('ReferenceVideoPicker', () => {
  beforeEach(() => {
    extractMock.mockReset();
  });

  it('基础渲染:显示文件选择 + 提示文字', () => {
    render(<ReferenceVideoPicker onChange={() => {}} />);

    expect(screen.getByText(/🎞️/)).toBeDefined();
    expect(screen.getByText(/上传参考视频/)).toBeDefined();
    expect(screen.getByText(/mp4 \/ webm/)).toBeDefined();
  });

  it('拒超大文件:不创建 blob URL,不进入抽帧,onError 回调', async () => {
    const onChange = vi.fn();
    const onError = vi.fn();

    render(<ReferenceVideoPicker onChange={onChange} onError={onError} maxSizeMb={50} />);

    // 200MB 大文件,超过 50MB 上限
    const huge = makeFile('huge.mp4', 200 * 1024 * 1024);
    const input = screen.getByTestId('reference-video-file-input') as HTMLInputElement;

    fireEvent.change(input, { target: { files: [huge] } });

    // onError 被调,带 size 信息
    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError.mock.calls[0][0]).toMatch(/太大/);
    // 抽帧 mock 没被调
    expect(extractMock).not.toHaveBeenCalled();
  });

  it('拒绝不支持的格式:onError 回调', () => {
    const onError = vi.fn();
    render(<ReferenceVideoPicker onChange={() => {}} onError={onError} />);

    const weird = makeFile('weird.mov', 5 * 1024 * 1024, 'video/quicktime');
    const input = screen.getByTestId('reference-video-file-input') as HTMLInputElement;

    fireEvent.change(input, { target: { files: [weird] } });

    expect(onError).toHaveBeenCalled();
    expect(onError.mock.calls[0][0]).toMatch(/格式不支持/);
  });

  it('帧数变化触发新的抽帧调用(参数随 count 调整)', async () => {
    extractMock.mockResolvedValue([
      { id: 'f1', dataUrl: makeDataUrl(1), timestampMs: 1000 },
      { id: 'f2', dataUrl: makeDataUrl(2), timestampMs: 2000 },
      { id: 'f3', dataUrl: makeDataUrl(3), timestampMs: 3000 },
    ]);

    render(<ReferenceVideoPicker onChange={() => {}} defaultCount={5} />);

    const input = screen.getByTestId('reference-video-file-input') as HTMLInputElement;
    const ok = makeFile('ok.mp4', 10 * 1024 * 1024);
    fireEvent.change(input, { target: { files: [ok] } });

    // 触发 loadedmetadata
    const video = screen.getByTestId('reference-video-element') as HTMLVideoElement;
    Object.defineProperty(video, 'videoWidth', { value: 1280, configurable: true });
    Object.defineProperty(video, 'videoHeight', { value: 720, configurable: true });
    Object.defineProperty(video, 'duration', { value: 10, configurable: true });
    fireEvent.loadedMetadata(video);

    // 等 useEffect + microtask
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(extractMock).toHaveBeenCalledTimes(1);
    const firstTimes = extractMock.mock.calls[0][2] as number[];
    expect(firstTimes).toHaveLength(5); // defaultCount

    // 改 count → 重抽
    const slider = screen.getByTestId('frame-count-slider') as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '7' } });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(extractMock).toHaveBeenCalledTimes(2);
    const secondTimes = extractMock.mock.calls[1][2] as number[];
    expect(secondTimes).toHaveLength(7);

    // 第二个 snapshot 用同一个 component instance,无卸载,extracTimes 不同
    expect(secondTimes).not.toEqual(firstTimes);
  });

  it('抽帧完成后通过 onChange 把帧数推给上层', async () => {
    const onChange = vi.fn();
    const extractedFrames = [
      { id: 'f1', dataUrl: makeDataUrl(1), timestampMs: 1000 },
      { id: 'f2', dataUrl: makeDataUrl(2), timestampMs: 2000 },
      { id: 'f3', dataUrl: makeDataUrl(3), timestampMs: 3000 },
    ];
    extractMock.mockResolvedValue(extractedFrames);

    render(<ReferenceVideoPicker onChange={onChange} defaultCount={3} />);

    const input = screen.getByTestId('reference-video-file-input') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [makeFile('a.mp4', 10 * 1024 * 1024)] } });

    const video = screen.getByTestId('reference-video-element') as HTMLVideoElement;
    Object.defineProperty(video, 'videoWidth', { value: 1280, configurable: true });
    Object.defineProperty(video, 'videoHeight', { value: 720, configurable: true });
    Object.defineProperty(video, 'duration', { value: 5, configurable: true });
    fireEvent.loadedMetadata(video);

    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    // 最后一次 onChange 调用应传出完整的 extractedFrames
    expect(onChange).toHaveBeenCalled();
    const lastCall = onChange.mock.calls[onChange.mock.calls.length - 1][0];
    expect(lastCall).toEqual(extractedFrames);
  });
});
