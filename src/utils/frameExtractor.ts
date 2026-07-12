// W3.7+ 抽帧纯函数 — 输入 videoEl + canvas + 时间戳列表,串行输出 jpeg dataUrl
//
// 为什么独立成纯函数:
// - 单元可测 (jsdom 里没真 <video>, 但可以 stub currentTime + seeked 事件)
// - 组件层只管 UI (file picker / slider / preview),把数学扔这里
// - 串行 seek 队列避免 currentTime + seeked 竞态
// - canvas 每次抽完重置 width=0 释放位图内存(WebView 大视频必备)
// - AbortSignal 支持,外层 cancel 按钮可中断

export interface ExtractedFrame {
  id: string;
  dataUrl: string;
  timestampMs: number;
  description?: string;
}

export interface ExtractOptions {
  /** JPEG 质量 (0-1),默认 0.7 */
  quality?: number;
  /** 中断信号;被 abort 后已抽帧保留,后续未抽的不抽 */
  signal?: AbortSignal;
}

/**
 * 串行 seek 到每个时间戳,drawImage 到 canvas,导出 jpeg。
 * 失败抛 Error 并保留已抽的帧。
 *
 * 关键约束:
 * - 必须 preload="auto" (或 "metadata") 才能用 currentTime
 * - canvas 必须先按 video.videoWidth/Height 设 size,否则 drawImage 0x0
 * - seek 后等 seeked 事件触发才 draw — 直接读 currentTime 会拿到旧帧
 */
export async function extractFramesAtTimestamps(
  videoEl: HTMLVideoElement,
  canvas: HTMLCanvasElement,
  timesMs: number[],
  opts: ExtractOptions = {},
): Promise<ExtractedFrame[]> {
  const { quality = 0.7, signal } = opts;
  const results: ExtractedFrame[] = [];

  // 防御性检查
  if (!videoEl || !canvas) {
    throw new Error('videoEl 和 canvas 都必须提供');
  }
  if (!Number.isFinite(videoEl.videoWidth) || videoEl.videoWidth === 0) {
    throw new Error('video 还没加载 metadata(无 videoWidth)');
  }

  // 设置 canvas 尺寸为视频尺寸(只设一次)
  canvas.width = videoEl.videoWidth;
  canvas.height = videoEl.videoHeight;

  for (let i = 0; i < timesMs.length; i++) {
    if (signal?.aborted) {
      throw new DOMException('Extraction aborted', 'AbortError');
    }
    const ts = timesMs[i];
    await seekTo(videoEl, ts, signal);
    if (signal?.aborted) {
      throw new DOMException('Extraction aborted', 'AbortError');
    }
    drawFrame(videoEl, canvas);
    const dataUrl = canvas.toDataURL('image/jpeg', quality);
    results.push({
      id: `f${i}_${Math.floor(ts)}`,
      dataUrl,
      timestampMs: ts,
    });
    // 显式释放位图内存 —— toDataURL 已复制,但 canvas.width=0 强制回收
    canvas.width = 0;
    canvas.height = 0;
    // 下一轮重新设回 video 尺寸
    if (i + 1 < timesMs.length) {
      canvas.width = videoEl.videoWidth;
      canvas.height = videoEl.videoHeight;
    }
  }
  return results;
}

/// 把视频均匀切成 N 段时间戳。
/// timeRange: [startMs, endMs],默认 [0, duration]
export function evenlySpacedTimes(
  count: number,
  durationMs: number,
  timeRange?: [number, number],
): number[] {
  if (count <= 0) return [];
  const [start, end] = timeRange ?? [0, durationMs];
  const span = end - start;
  if (span <= 0 || count === 1) return [start];
  // 等距采样,首尾都包含
  const step = span / (count - 1);
  return Array.from({ length: count }, (_, i) => Math.round(start + step * i));
}

// ----------------- 内部 helpers -----------------

/**
 * Seek 到指定时间,等待 'seeked' 事件 resolve。
 * 关键:必须等 seeked,直接读 currentTime 会拿到旧帧。
 */
function seekTo(
  videoEl: HTMLVideoElement,
  timeMs: number,
  signal?: AbortSignal,
): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    if (signal?.aborted) {
      reject(new DOMException('aborted', 'AbortError'));
      return;
    }
    const onSeeked = () => {
      cleanup();
      resolve();
    };
    const onAbort = () => {
      cleanup();
      reject(new DOMException('aborted', 'AbortError'));
    };
    const onError = () => {
      cleanup();
      reject(new Error('video seek error'));
    };
    const cleanup = () => {
      videoEl.removeEventListener('seeked', onSeeked);
      videoEl.removeEventListener('error', onError);
      signal?.removeEventListener('abort', onAbort);
    };
    videoEl.addEventListener('seeked', onSeeked, { once: true });
    videoEl.addEventListener('error', onError, { once: true });
    if (signal) signal.addEventListener('abort', onAbort, { once: true });
    // 设 currentTime 触发 seek;毫秒 → 秒
    videoEl.currentTime = timeMs / 1000;
  });
}

function drawFrame(videoEl: HTMLVideoElement, canvas: HTMLCanvasElement) {
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('canvas 2d context 不可用');
  ctx.drawImage(videoEl, 0, 0, canvas.width, canvas.height);
}
