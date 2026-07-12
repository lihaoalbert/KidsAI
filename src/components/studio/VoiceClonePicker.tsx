// W6 C2 + E2: 声音复刻入口 (阶段2 主角拍板后).
//
// 设计原则:
// - 显式 10 学币/次 cost, 让家长看到支出
// - 录 10 秒 / 上传音频 / 跳过 三选 — 不让孩面对"必须录"的强制
// - 默认 MiniMax; 后端无 MiniMax key 时显示"暂不可用", 引导跳过
//
// 接线 (W6.5+):
// - audio 录音用 MediaRecorder API (浏览器原生, 无需 tauri plugin)
// - 录完拿到 Blob → 转 base64 → 通过 tauri command 上传到 MiniMax
// - 当前 UI 仅渲染 CTA + 状态; 后端接线留 W6.5+

import { useState } from 'react';
import { useDirectorStore } from '../../stores/directorStore';

interface VoiceClonePickerProps {
  onClose: () => void;
}

type Status = 'idle' | 'recording' | 'uploading' | 'success' | 'error' | 'unavailable';

export default function VoiceClonePicker({ onClose }: VoiceClonePickerProps) {
  const character = useDirectorStore((s) => s.character);
  const voiceId = useDirectorStore((s) => s.voiceId);
  const setVoiceId = useDirectorStore((s) => s.setVoiceId);
  const [status, setStatus] = useState<Status>(voiceId ? 'success' : 'idle');
  const [error, setError] = useState<string | null>(null);

  const handleStartRecord = async () => {
    setError(null);
    setStatus('recording');
    try {
      // 检查浏览器支持
      if (!navigator.mediaDevices?.getUserMedia) {
        throw new Error('浏览器不支持录音, 请改用上传音频');
      }
      // MediaRecorder 拿 10s 流 (W6.5+ 接 tauri command 上传)
      // 当前 stub: 模拟 2s 后 mock 返 voice_id
      await new Promise((r) => setTimeout(r, 1500));
      setStatus('uploading');
      await new Promise((r) => setTimeout(r, 1200));
      const mockVoiceId = `mock_${character?.id ?? 'kid'}_${Date.now().toString(36)}`;
      setVoiceId(mockVoiceId);
      setStatus('success');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setStatus('error');
    }
  };

  const handleSkip = () => {
    setVoiceId(null);
    onClose();
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-sm rounded-2xl bg-white p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-base font-semibold text-gray-800">
            🎤 给主角录个声音
          </h3>
          <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[10px] font-medium text-amber-700">
            10 学币 / 次
          </span>
        </div>

        {character && (
          <div className="mb-4 rounded-xl bg-gray-50 p-3 text-xs text-gray-600">
            <span className="font-medium text-gray-800">{character.name}</span> 的声音会变成主角的"专属配音",
            后续每个分镜的旁白都用这个声音读.
          </div>
        )}

        {voiceId ? (
          <div className="mb-4 rounded-xl border border-emerald-200 bg-emerald-50 p-3">
            <div className="text-sm font-medium text-emerald-800">✓ 主角有了专属声音</div>
            <div className="mt-1 text-[10px] text-emerald-600">voice_id: {voiceId}</div>
          </div>
        ) : status === 'success' ? null : (
          <div className="space-y-2">
            <button
              type="button"
              onClick={handleStartRecord}
              disabled={status === 'recording' || status === 'uploading'}
              className="flex w-full items-center justify-center gap-2 rounded-xl bg-brand-500 px-4 py-3 text-sm font-medium text-white hover:bg-brand-600 disabled:opacity-50"
            >
              {status === 'recording' && <span>🎙️ 录音中...</span>}
              {status === 'uploading' && <span>📤 训练中...</span>}
              {(status === 'idle' || status === 'error') && (
                <>
                  <span>🎙️</span>
                  <span>录 10 秒</span>
                </>
              )}
            </button>
            <button
              type="button"
              disabled
              className="flex w-full items-center justify-center gap-2 rounded-xl border border-gray-200 px-4 py-2 text-sm text-gray-500 opacity-60"
              title="W6.5+ 上线"
            >
              📁 上传音频文件 (即将开放)
            </button>
          </div>
        )}

        {status === 'error' && error && (
          <div className="mt-2 text-[11px] text-rose-600">⚠️ {error}</div>
        )}

        <div className="mt-4 flex justify-between gap-2">
          <button
            type="button"
            onClick={handleSkip}
            className="rounded-xl bg-gray-100 px-3 py-1.5 text-xs text-gray-600 hover:bg-gray-200"
          >
            {voiceId ? '完成 →' : '跳过, 用系统声音'}
          </button>
          {voiceId && (
            <button
              type="button"
              onClick={() => {
                setVoiceId(null);
                setStatus('idle');
              }}
              className="rounded-xl bg-gray-100 px-3 py-1.5 text-xs text-gray-600 hover:bg-gray-200"
            >
              ↺ 重新录
            </button>
          )}
        </div>
      </div>
    </div>
  );
}