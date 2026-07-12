// W3.7+ 拉片复刻 — 视频文件选择 + 浏览器抽帧 UI
// 关键点:
// - 文件选择 + 大小/时长校验(< 100MB / 120s);超限弹 toast 不进入抽帧
// - 用 URL.createObjectURL 喂给隐藏 <video>;元数据加载完后抽帧
// - 帧数滑块 3-8;变化触发重新抽(useMemo via (file,count))
// - 必须清理:URL.revokeObjectURL + canvas.width=0
// - 全前端 — 无后端 ffmpeg 依赖

import { useEffect, useMemo, useRef, useState } from 'react';
import {
  extractFramesAtTimestamps,
  evenlySpacedTimes,
  type ExtractedFrame,
} from '../utils/frameExtractor';

export interface ReferenceVideoPickerProps {
  /// 建议抽多少帧(3-8,默认 5)
  defaultCount?: number;
  /// 上限配置
  maxSizeMb?: number;
  maxDurationSec?: number;
  /// 帧变化时回调(把抽好的帧给上层)
  onChange: (frames: ExtractedFrame[]) => void;
  /// 文件选择错误的回调(toast)
  onError?: (message: string) => void;
}

const DEFAULT_MAX_MB = 100;
const DEFAULT_MAX_DURATION_SEC = 120;
const MIN_COUNT = 3;
const MAX_COUNT = 8;

export function ReferenceVideoPicker({
  defaultCount = 5,
  maxSizeMb = DEFAULT_MAX_MB,
  maxDurationSec = DEFAULT_MAX_DURATION_SEC,
  onChange,
  onError,
}: ReferenceVideoPickerProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const [videoFile, setVideoFile] = useState<File | null>(null);
  const [videoUrl, setVideoUrl] = useState<string | null>(null);
  const [videoMeta, setVideoMeta] = useState<{
    width: number;
    height: number;
    duration: number;
  } | null>(null);
  const [count, setCount] = useState(defaultCount);
  const [frames, setFrames] = useState<ExtractedFrame[]>([]);
  const [isExtracting, setIsExtracting] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // 选新文件:校验 + 建 blob URL + 加载 metadata
  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0];
    if (!f) return;
    setErrorMsg(null);

    // 大小校验
    const sizeMb = f.size / (1024 * 1024);
    if (sizeMb > maxSizeMb) {
      const msg = `视频太大(${sizeMb.toFixed(0)}MB),请上传 ${maxSizeMb}MB 以内`;
      setErrorMsg(msg);
      onError?.(msg);
      e.target.value = '';
      return;
    }
    // 格式校验
    if (!/^(video\/mp4|video\/webm)/.test(f.type)) {
      const msg = `格式不支持(${f.type}),请用 mp4 或 webm`;
      setErrorMsg(msg);
      onError?.(msg);
      e.target.value = '';
      return;
    }

    // 撤销旧 url + frames
    if (videoUrl) URL.revokeObjectURL(videoUrl);
    setFrames([]);
    setVideoMeta(null);
    setVideoFile(f);
    const url = URL.createObjectURL(f);
    setVideoUrl(url);
  };

  // video metadata 加载完成 → 可抽帧
  const handleLoadedMetadata = () => {
    const v = videoRef.current;
    if (!v) return;
    const duration = v.duration;
    if (!Number.isFinite(duration) || duration <= 0) {
      const msg = '读取视频时长失败,请重试';
      setErrorMsg(msg);
      onError?.(msg);
      return;
    }
    if (duration > maxDurationSec) {
      const msg = `视频太长(${duration.toFixed(0)}s),请用 ${maxDurationSec}s 以内`;
      setErrorMsg(msg);
      onError?.(msg);
      // 清掉
      if (videoUrl) URL.revokeObjectURL(videoUrl);
      setVideoUrl(null);
      setVideoFile(null);
      if (fileInputRef.current) fileInputRef.current.value = '';
      return;
    }
    setVideoMeta({
      width: v.videoWidth,
      height: v.videoHeight,
      duration: duration * 1000,
    });
  };

  // 元数据 + count 都就绪 → 自动抽
  useEffect(() => {
    if (!videoMeta || !videoRef.current || !canvasRef.current) return;
    const v = videoRef.current;
    const canvas = canvasRef.current;

    // 跳过前 5% 和后 5%(避免黑屏/loading),中间均匀
    const startMs = videoMeta.duration * 0.05;
    const endMs = videoMeta.duration * 0.95;
    const times = evenlySpacedTimes(count, videoMeta.duration, [startMs, endMs]);

    const controller = new AbortController();
    abortRef.current?.abort();
    abortRef.current = controller;

    setIsExtracting(true);
    setFrames([]);
    extractFramesAtTimestamps(v, canvas, times, { signal: controller.signal })
      .then((result) => {
        if (controller.signal.aborted) return;
        setFrames(result);
        setIsExtracting(false);
      })
      .catch((err) => {
        if (err?.name === 'AbortError') return;
        const msg = `抽帧失败: ${err?.message ?? err}`;
        setErrorMsg(msg);
        onError?.(msg);
        setIsExtracting(false);
      });

    return () => controller.abort();
  }, [videoMeta, count, onError]);

  // 帧变化 → 上抛
  useEffect(() => {
    onChange(frames);
  }, [frames, onChange]);

  // 卸载 / 切换 level 时清理
  useEffect(() => {
    return () => {
      abortRef.current?.abort();
      if (videoUrl) URL.revokeObjectURL(videoUrl);
      if (canvasRef.current) {
        canvasRef.current.width = 0;
        canvasRef.current.height = 0;
      }
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const summary = useMemo(() => {
    if (!videoFile) return null;
    const sizeMb = (videoFile.size / (1024 * 1024)).toFixed(1);
    const dur = videoMeta ? `${(videoMeta.duration / 1000).toFixed(1)}s` : '加载中…';
    const w = videoMeta ? `${videoMeta.width}×${videoMeta.height}` : '';
    return `${sizeMb} MB · ${dur} ${w ? `· ${w}` : ''}`;
  }, [videoFile, videoMeta]);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold text-gray-900">🎞️ 上传参考视频</h4>
        <span className="text-[11px] text-gray-500">
          支持 mp4 / webm,≤ {maxSizeMb}MB,≤ {maxDurationSec}s
        </span>
      </div>

      {/* 文件选择 */}
      <div className="border-2 border-dashed border-gray-200 rounded-lg p-4 bg-gray-50">
        <input
          ref={fileInputRef}
          type="file"
          accept="video/mp4,video/webm"
          onChange={handleFileChange}
          className="block w-full text-xs text-gray-700 file:mr-3 file:py-1.5 file:px-3 file:rounded-md file:border-0 file:text-xs file:font-medium file:bg-brand-100 file:text-brand-700 hover:file:bg-brand-200"
          data-testid="reference-video-file-input"
        />
        {summary && (
          <div className="mt-2 text-[11px] text-gray-600 font-mono">{summary}</div>
        )}
      </div>

      {/* 帧数滑块 + 重抽 */}
      {videoMeta && (
        <div className="flex items-center gap-3">
          <label className="text-xs text-gray-600">抽帧数</label>
          <input
            type="range"
            min={MIN_COUNT}
            max={MAX_COUNT}
            value={count}
            onChange={(e) => setCount(Number(e.target.value))}
            className="flex-1"
            data-testid="frame-count-slider"
          />
          <span className="text-xs font-mono text-gray-700 w-6 text-right">{count}</span>
        </div>
      )}

      {errorMsg && (
        <div className="text-xs px-3 py-2 rounded bg-red-50 text-red-700 border border-red-200">
          {errorMsg}
        </div>
      )}

      {/* 帧预览 */}
      {frames.length > 0 && (
        <div>
          <div className="text-xs text-gray-600 mb-1">
            {isExtracting ? '⏳ 抽帧中…' : `✅ 已抽 ${frames.length} 帧`}
          </div>
          <div className="grid grid-cols-5 gap-2">
            {frames.map((f, i) => (
              <div key={f.id} className="relative">
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img
                  src={f.dataUrl}
                  alt={`frame ${i + 1}`}
                  className="w-full aspect-video object-cover rounded border border-gray-200"
                />
                <div className="absolute bottom-0 left-0 right-0 bg-black/60 text-white text-[10px] px-1 py-0.5 text-center font-mono">
                  {((f.timestampMs ?? 0) / 1000).toFixed(1)}s
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 隐藏元素:video 用于读取帧,canvas 用于 drawImage */}
      <video
        ref={(el) => {
          videoRef.current = el;
        }}
        src={videoUrl ?? undefined}
        preload="auto"
        muted
        onLoadedMetadata={handleLoadedMetadata}
        className="hidden"
        data-testid="reference-video-element"
      />
      <canvas ref={canvasRef} className="hidden" />
    </div>
  );
}

export default ReferenceVideoPicker;
