// W3.7+ frameExtractor 单元测试
// 关键场景:5 帧顺序 / AbortSignal 中断 / 空时间戳
// 实现策略:jsdom 里 HTMLVideoElement/HTMLCanvasElement 不行为完整,自己 stub 最小接口

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  extractFramesAtTimestamps,
  evenlySpacedTimes,
  type ExtractOptions,
} from './frameExtractor';

// ------------- stub HTMLVideoElement -------------
// 只需要 currentTime (setter 触发 'seeked' 事件) + videoWidth / videoHeight
function makeStubVideo(width = 640, height = 360) {
  const listeners: Record<string, Array<(e?: Event) => void>> = {};
  const video: any = {
    videoWidth: width,
    videoHeight: height,
    _currentTime: 0,
    get currentTime() {
      return this._currentTime;
    },
    set currentTime(t: number) {
      this._currentTime = t;
      // 下一个 microtask 触发 seeked (同步触发会让 test 不易控制)
      queueMicrotask(() => {
        const handlers = (listeners['seeked'] ?? []).slice();
        for (const h of handlers) h();
      });
    },
    addEventListener(name: string, handler: any) {
      if (!listeners[name]) listeners[name] = [];
      listeners[name].push(handler);
    },
    removeEventListener(name: string, handler: any) {
      if (!listeners[name]) return;
      listeners[name] = listeners[name].filter((h) => h !== handler);
    },
  };
  return video;
}

// ------------- stub HTMLCanvasElement -------------
function makeStubCanvas() {
  const canvas: any = {
    width: 640,
    height: 360,
    _calls: [] as Array<{ w: number; h: number }>,
    getContext() {
      return {
        drawImage: (_video: any, _x: number, _y: number, w: number, h: number) => {
          canvas._calls.push({ w, h });
        },
      };
    },
    toDataURL(_format: string, quality: number) {
      return `data:image/jpeg;base64,STUB_${canvas._calls.length}_q${quality}`;
    },
  };
  return canvas;
}

// Convenience wrapper
async function call(
  video: any,
  canvas: any,
  times: number[],
  opts: ExtractOptions = {},
) {
  return extractFramesAtTimestamps(video as HTMLVideoElement, canvas as HTMLCanvasElement, times, opts);
}

beforeEach(() => {
  vi.useRealTimers();
});

describe('extractFramesAtTimestamps', () => {
  it('5 个时间戳按顺序抽帧;每帧的 dataUrl 不同且递增', async () => {
    const video = makeStubVideo();
    const canvas = makeStubCanvas();
    const times = [100, 200, 300, 400, 500];

    const frames = await call(video, canvas, times);

    expect(frames).toHaveLength(5);
    expect(frames[0].timestampMs).toBe(100);
    expect(frames[4].timestampMs).toBe(500);

    // dataUrls are deterministic-by-call-number (STUB_1_, STUB_2_, ...)
    expect(frames[0].dataUrl).toBe('data:image/jpeg;base64,STUB_1_q0.7');
    expect(frames[4].dataUrl).toBe('data:image/jpeg;base64,STUB_5_q0.7');

    // video.currentTime 在最后一次 await 之后是 times[4]/1000
    expect(video._currentTime).toBe(500 / 1000);

    // 抽完所有帧后 canvas 被释放(最终 width=0,无下一轮要重置)
    expect(canvas.width).toBe(0);
    expect(canvas._calls.length).toBe(5);
  });

  it('空时间戳:resolve 空数组,canvas 无副作用', async () => {
    const video = makeStubVideo();
    const canvas = makeStubCanvas();

    const frames = await call(video, canvas, []);

    expect(frames).toEqual([]);
    expect(canvas._calls.length).toBe(0);
  });

  it('AbortSignal:预先 abort 立即抛 AbortError,不进入 loop', async () => {
    const video = makeStubVideo();
    const canvas = makeStubCanvas();
    const controller = new AbortController();
    controller.abort(); // 调用前就 abort

    await expect(
      call(video, canvas, [100, 200, 300], { signal: controller.signal }),
    ).rejects.toMatchObject({ name: 'AbortError' });

    // 一帧都没抽 — canvas 无副作用
    expect(canvas._calls.length).toBe(0);
  });

  it('video 没 metadata (videoWidth=0) 立即抛错,不进入 loop', async () => {
    const video = makeStubVideo(0, 0); // 0x0 = 没加载
    const canvas = makeStubCanvas();

    await expect(call(video, canvas, [100])).rejects.toThrow(
      /videoWidth/i,
    );
  });

  it('quality 选项透传到 toDataURL', async () => {
    const video = makeStubVideo();
    const canvas = makeStubCanvas();

    const frames = await call(video, canvas, [100], { quality: 0.3 });

    expect(frames[0].dataUrl).toContain('_q0.3');
  });
});

describe('evenlySpacedTimes', () => {
  it('count=5, duration=10000ms → [0, 2500, 5000, 7500, 10000]', () => {
    expect(evenlySpacedTimes(5, 10000)).toEqual([0, 2500, 5000, 7500, 10000]);
  });

  it('count=1 → [0] (单点)', () => {
    expect(evenlySpacedTimes(1, 5000)).toEqual([0]);
  });

  it('count=0 → []', () => {
    expect(evenlySpacedTimes(0, 5000)).toEqual([]);
  });

  it('timeRange=[1000, 3000] + count=3 → [1000, 2000, 3000]', () => {
    expect(evenlySpacedTimes(3, 99999, [1000, 3000])).toEqual([1000, 2000, 3000]);
  });
});
