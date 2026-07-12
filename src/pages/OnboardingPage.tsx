// 首次启动激活页 (W4.5 B2)
//
// 流程:
//   1. 收集昵称 + 年级 (3 选 1: 8-10 / 11-13 / 14-16)
//   2. 调 backend activate → 写本地 license.json → 跳 HomePage
//   3. 失败 → 内联错误 + 重试; demo 模式 → 直接跳 (无 server 时)
//
// 设计要点:
//   - 单屏表单, 不要分步骤(孩子没耐心)
//   - 失败信息具体 (服务器连不上 / 昵称太长), 不要泛泛"出错了"
//   - 激活前 disable 主页; 激活后自动跳

import { useState } from 'react';
import {
  activateDevice,
  getFingerprintHash,
  getLicenseInfo,
  type ActivateResponse,
} from '../api/tauri';

const AGE_TIERS = [
  { value: 1, label: '8-10 岁', emoji: '🐣' },
  { value: 2, label: '11-13 岁', emoji: '🐥' },
  { value: 3, label: '14-16 岁', emoji: '🦅' },
] as const;

interface OnboardingPageProps {
  onActivated: (resp: ActivateResponse) => void;
  /// 测试钩: 已激活时跳过 Onboarding
  onSkip?: () => void;
}

export default function OnboardingPage({
  onActivated,
  onSkip,
}: OnboardingPageProps) {
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
      onActivated(resp);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // server 不可达时, demo 模式仍会给一个 envelope (KIDSAI_SERVER_URL 未设)
      // 但如果是连不上 + 有 server 模式, 给出具体建议
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
    // 仅 dev 调试用: 清掉旧 license, 跳过激活直接进 Home
    // (生产路径不显示此按钮)
    if (onSkip) onSkip();
  };

  return (
    <div className="min-h-full flex items-center justify-center px-6 py-12 bg-gradient-to-b from-warm-50 to-warm-100">
      <div className="max-w-md w-full bg-white rounded-2xl shadow-lg p-8">
        <div className="text-center mb-6">
          <div className="text-5xl mb-2">🦉</div>
          <h1 className="text-2xl font-bold text-gray-900">欢迎来到 KidsAI</h1>
          <p className="text-sm text-gray-600 mt-1">
            先认识一下你，我们就可以开始创作啦～
          </p>
        </div>

        <label className="block mb-5">
          <span className="text-sm font-medium text-gray-700 mb-1 block">
            你的昵称
          </span>
          <input
            type="text"
            value={nickname}
            onChange={(e) => setNickname(e.target.value)}
            placeholder="比如 小明"
            maxLength={16}
            className="w-full px-4 py-2 border border-gray-300 rounded-lg focus:outline-none focus:border-brand-400 text-base"
            disabled={submitting}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !submitting) handleSubmit();
            }}
          />
        </label>

        <div className="mb-6">
          <span className="text-sm font-medium text-gray-700 mb-2 block">
            你多大了？
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
                    ? 'border-brand-500 bg-brand-50'
                    : 'border-gray-200 hover:border-gray-300'
                }`}
              >
                <div className="text-2xl">{tier.emoji}</div>
                <div className="text-xs mt-1 font-medium">{tier.label}</div>
              </button>
            ))}
          </div>
        </div>

        {error && (
          <div className="mb-4 p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700">
            {error}
          </div>
        )}

        <button
          type="button"
          onClick={handleSubmit}
          disabled={submitting || !nickname.trim()}
          className="w-full py-3 bg-brand-500 hover:bg-brand-600 text-white font-semibold rounded-lg disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {submitting ? '正在准备你的工作室...' : '开始创作 🎨'}
        </button>

        {onSkip && (
          <button
            type="button"
            onClick={handleResetAndSkip}
            className="mt-3 text-xs text-gray-400 hover:text-gray-600 underline block w-full text-center"
          >
            调试: 跳过激活
          </button>
        )}

        <p className="text-xs text-gray-400 text-center mt-4">
          首次激活会获得 <span className="font-semibold">100 学币</span>，
          每天可生成 30 学币的 AI 作品
        </p>
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