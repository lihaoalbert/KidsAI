// 首次启动激活页 (W4.5 B2) — 阶段3 重做 hero 版
//
// DESIGN.md §5.4 Onboarding hero:
//   Forest: 60% mascot + "跟 🦉 一起 拍出 你的故事" (inline image typography) | 40% form
//   Coast:  40% form | 60% sample preview (mock Studio shot)
//   CTA restraint: 1 primary CTA. Forest: "开始创作"; Coast: "Launch Studio"
//   Step indicator: Forest adds 3-dot progress (1/3 · 2/3 · 3/3) to reassure 8-year-olds
//
// 流程: 1. 收集昵称 + 年级 (3 选 1: 8-10 / 11-13 / 14-16)
//       2. 调 backend activate → 写本地 license.json → 跳 HomePage
//       3. 失败 → 内联错误 + 重试; demo 模式 → 直接跳

import { useState } from 'react';
import {
  activateDevice,
  getFingerprintHash,
  getLicenseInfo,
  saveIdentity,
  type ActivateResponse,
} from '../api/tauri';
import { useUserModeStore } from '../stores/userModeStore';

const AGE_TIERS = [
  { value: 1, label: '8-10 岁', emoji: '🐣' },
  { value: 2, label: '11-13 岁', emoji: '🐥' },
  { value: 3, label: '14-16 岁', emoji: '🦅' },
] as const;

/// ageTier 数字 (W4.5) → kernel 字符串
const AGE_TIER_TO_KERNEL: Record<number, string> = {
  1: '8-10',
  2: '11-13',
  3: '14-16',
};

interface OnboardingPageProps {
  onActivated: (resp: ActivateResponse) => void;
  onSkip?: () => void;
}

export default function OnboardingPage({
  onActivated,
  onSkip,
}: OnboardingPageProps) {
  const mode = useUserModeStore((s) => s.mode);
  const isAdult = mode === 'adult';
  const [nickname, setNickname] = useState('');
  const [ageTier, setAgeTier] = useState<number>(1);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    const name = nickname.trim();
    if (!name) {
      setError('请先填一个昵称');
      return;
    }
    if (name.length > 16) {
      setError('昵称最多 16 个字');
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const fp = await getFingerprintHash();
      const resp = await activateDevice(fp, name, ageTier);
      try {
        await saveIdentity({
          userId: resp.deviceId,
          nickname: name,
          petId: 'huomiao',
          ageTier: AGE_TIER_TO_KERNEL[ageTier] ?? '8-10',
        });
      } catch (idErr) {
        console.warn('[onboarding] saveIdentity failed (non-blocking):', idErr);
      }
      onActivated(resp);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes('activate http')) {
        setError(
          '连不上学币服务器，请检查网络后重试；如果问题持续请联系管理员。',
        );
      } else {
        setError(msg);
      }
    } finally {
      setSubmitting(false);
    }
  };

  const handleResetAndSkip = async () => {
    if (onSkip) onSkip();
  };

  // Forest 3-dot progress — 安慰 8 岁娃
  const ProgressDots = () => (
    <div className="flex items-center gap-1.5 text-2xs font-medium text-ink-3">
      <span className="text-accent-ink">1/3</span>
      <span className="w-1.5 h-1.5 rounded-full bg-accent" />
      <span className="w-1.5 h-1.5 rounded-full bg-accent-line" />
      <span className="w-1.5 h-1.5 rounded-full bg-accent-line" />
      <span>先认识一下</span>
    </div>
  );

  // Form panel — 双模式共用
  const FormPanel = (
    <div className="bg-surface rounded-2xl shadow-lg p-8 border border-line">
      <div className="text-center mb-6">
        <div className="text-5xl mb-2">{isAdult ? '✨' : '🦉'}</div>
        <h1 className="text-2xl font-bold text-ink">
          {isAdult ? 'Welcome to KidsAI Studio' : '欢迎来到 KidsAI'}
        </h1>
        <p className="text-sm text-ink-2 mt-1">
          {isAdult
            ? 'Set up your workspace to start creating.'
            : '先认识一下你，我们就可以开始创作啦～'}
        </p>
      </div>

      <label className="block mb-5">
        <span className="text-sm font-medium text-ink-2 mb-1 block">
          {isAdult ? 'Display name' : '你的昵称'}
        </span>
        <input
          type="text"
          value={nickname}
          onChange={(e) => setNickname(e.target.value)}
          placeholder={isAdult ? 'e.g. Jordan' : '比如 小明'}
          maxLength={16}
          className="w-full px-4 py-2 border border-line rounded-lg focus:outline-none focus:border-accent text-base bg-bg text-ink placeholder:text-ink-3"
          disabled={submitting}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !submitting) handleSubmit();
          }}
        />
      </label>

      <div className="mb-6">
        <span className="text-sm font-medium text-ink-2 mb-2 block">
          {isAdult ? 'Age range' : '你多大了？'}
        </span>
        <div className="grid grid-cols-3 gap-2">
          {AGE_TIERS.map((tier) => (
            <button
              key={tier.value}
              type="button"
              onClick={() => setAgeTier(tier.value)}
              disabled={submitting}
              className={`py-3 px-2 rounded-lg border-2 text-center transition-all ${
                ageTier === tier.value
                  ? 'border-accent bg-accent-soft'
                  : 'border-line hover:border-accent-line'
              }`}
            >
              <div className="text-2xl">{tier.emoji}</div>
              <div className="text-xs mt-1 font-medium text-ink-2">
                {tier.label}
              </div>
            </button>
          ))}
        </div>
      </div>

      {error && (
        <div className="mb-4 p-3 bg-danger-soft border border-danger-soft rounded-lg text-sm text-danger">
          {error}
        </div>
      )}

      <button
        type="button"
        onClick={handleSubmit}
        disabled={submitting || !nickname.trim()}
        className="w-full py-3 bg-accent hover:bg-accent-hover active:bg-accent-active text-bg font-semibold rounded-lg disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {submitting
          ? isAdult
            ? 'Preparing workspace…'
            : '正在准备你的工作室...'
          : isAdult
            ? 'Launch Studio'
            : '开始创作 🎨'}
      </button>

      {onSkip && (
        <button
          type="button"
          onClick={handleResetAndSkip}
          className="mt-3 text-xs text-ink-3 hover:text-ink-2 underline block w-full text-center"
        >
          调试: 跳过激活
        </button>
      )}

      <p className="text-xs text-ink-3 text-center mt-4">
        {isAdult
          ? '100 credits on activation. Generous daily limits in Adult mode.'
          : '首次激活会获得 100 学币，每天可生成 30 学币的 AI 作品'}
      </p>
    </div>
  );

  // Forest mascot panel — 60% left, inline image typography
  const ForestHero = (
    <div className="flex flex-col justify-center px-12 py-16 bg-gradient-to-br from-accent-soft/40 to-highlight/30 rounded-3xl border border-line min-h-[480px]">
      <div className="text-6xl mb-6">🦉</div>
      <h2 className="text-3xl font-bold text-ink leading-tight mb-4">
        跟 <span className="inline-block text-5xl align-middle">🦉</span>{' '}
        一起 <br />
        拍出 <span className="text-accent-ink">你的故事</span>
      </h2>
      <p className="text-base text-ink-2 leading-relaxed max-w-md">
        AI 陪你从一句话开始，分 6 步把脑子里的画面变成真正的视频。
        <br />
        不用会画画，不用会剪辑 — 告诉 🦉 你想看什么就好。
      </p>
      <div className="mt-8">
        <ProgressDots />
      </div>
    </div>
  );

  // Coast sample panel — 60% right, mock Studio shot
  const CoastHero = (
    <div className="flex flex-col justify-center px-12 py-16 bg-gradient-to-br from-surface-2 to-surface rounded-3xl border border-line min-h-[480px]">
      <div className="flex items-center gap-2 mb-6">
        <div className="w-2 h-2 rounded-full bg-accent" />
        <span className="text-meta uppercase tracking-wider text-ink-3 font-mono">
          Studio · Live
        </span>
      </div>
      <h2 className="text-3xl font-bold text-ink leading-tight mb-4">
        Create <span className="text-accent-ink">cinematic</span> stories
        <br />
        with your AI co-director.
      </h2>
      <p className="text-base text-ink-2 leading-relaxed max-w-md font-mono">
        // 6-step pipeline: brief → character → narrative → storyboard →
        shot → final cut.
        <br />
        // Production-grade output. Pro tools, calm palette.
      </p>
      <div className="mt-8 grid grid-cols-3 gap-2 max-w-md">
        {[1, 2, 3].map((i) => (
          <div
            key={i}
            className="aspect-video rounded-md bg-surface-2 border border-line flex items-center justify-center"
          >
            <div className="w-6 h-6 rounded-full bg-accent/30" />
          </div>
        ))}
      </div>
    </div>
  );

  return (
    <div
      data-mode={isAdult ? 'adult' : 'child'}
      className="min-h-full flex items-center justify-center bg-bg text-ink"
    >
      <div className="w-full max-w-6xl px-6 py-12">
        <div className="grid grid-cols-1 lg:grid-cols-5 gap-8 items-center">
          {/* Forest: 60% mascot left, 40% form right */}
          {!isAdult && (
            <>
              <div className="lg:col-span-3">{ForestHero}</div>
              <div className="lg:col-span-2">{FormPanel}</div>
            </>
          )}
          {/* Coast: 40% form left, 60% sample right */}
          {isAdult && (
            <>
              <div className="lg:col-span-2">{FormPanel}</div>
              <div className="lg:col-span-3">{CoastHero}</div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// helper used by tests / dev: 探测 license 状态 (SSR-safe)
export async function checkAlreadyActivated(): Promise<boolean> {
  try {
    const info = await getLicenseInfo();
    return info !== null;
  } catch {
    return false;
  }
}