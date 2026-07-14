// W10 Day 4 — 家长 PIN 弹层
//
// 复用组件, 用于:
//   1. 首次设置 PIN (set 流程): 输入 + 确认
//   2. 验证 PIN (verify 流程): 单次输入
//   3. 重设 PIN (reset + set 流程): 先 verify 当前 PIN 才能重设
//
// 4 位数字 PIN, 数字键盘组件, 防止娃乱按字母键.
//
// 设计:
//   - 父组件管理 open 状态; onSuccess 在 PIN 验证/设置成功后触发.
//   - 内部错误用 inline 提示, 失败 3 次锁 30 秒 (前端状态, 防止快速 brute force)

import { useEffect, useRef, useState } from 'react';
import {
  isParentPinSet,
  setParentPin,
  verifyParentPin,
} from '../../api/tauri';

interface ParentPinDialogProps {
  open: boolean;
  /// "setup" 首次设 / "verify" 已设后验证
  mode?: 'setup' | 'verify';
  /// 验证 / 设置成功后回调, 传入用户输入的 PIN (verify 模式)
  onSuccess: (pin: string) => void;
  /// 取消回调
  onCancel: () => void;
  /// 标题 (默认 "家长 PIN")
  title?: string;
  /// 提示文案
  hint?: string;
}

const MAX_ATTEMPTS = 3;
const LOCKOUT_SECONDS = 30;
const PIN_LENGTH = 4;

export function ParentPinDialog({
  open,
  mode: initialMode,
  onSuccess,
  onCancel,
  title = '家长 PIN',
  hint,
}: ParentPinDialogProps) {
  const [mode, setMode] = useState<'setup' | 'verify'>(initialMode ?? 'verify');
  const [pin, setPin] = useState('');
  const [confirmPin, setConfirmPin] = useState('');
  const [stage, setStage] = useState<'enter' | 'confirm'>('enter');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [attempts, setAttempts] = useState(0);
  const [lockedUntil, setLockedUntil] = useState<number>(0);
  const [now, setNow] = useState<number>(Date.now());
  const tickRef = useRef<number | null>(null);

  // 打开时: 检查 PIN 是否已设, 决定 mode
  useEffect(() => {
    if (!open) return;
    setPin('');
    setConfirmPin('');
    setStage('enter');
    setError(null);
    setAttempts(0);
    setLockedUntil(0);
    if (initialMode) {
      setMode(initialMode);
    } else {
      isParentPinSet()
        .then((set) => setMode(set ? 'verify' : 'setup'))
        .catch(() => setMode('verify'));
    }
  }, [open, initialMode]);

  // 锁定倒计时
  useEffect(() => {
    if (lockedUntil > 0) {
      tickRef.current = window.setInterval(() => {
        setNow(Date.now());
        if (Date.now() >= lockedUntil) {
          setLockedUntil(0);
          setAttempts(0);
        }
      }, 500);
      return () => {
        if (tickRef.current !== null) window.clearInterval(tickRef.current);
      };
    }
    return undefined;
  }, [lockedUntil]);

  if (!open) return null;

  const isLocked = lockedUntil > 0 && now < lockedUntil;
  const lockSecondsLeft = isLocked
    ? Math.ceil((lockedUntil - now) / 1000)
    : 0;

  const handleDigit = (d: string) => {
    if (busy || isLocked) return;
    setError(null);
    if (stage === 'enter') {
      if (pin.length >= PIN_LENGTH) return;
      setPin((p) => p + d);
    } else {
      if (confirmPin.length >= PIN_LENGTH) return;
      setConfirmPin((p) => p + d);
    }
  };

  const handleBackspace = () => {
    if (busy || isLocked) return;
    setError(null);
    if (stage === 'enter') {
      setPin((p) => p.slice(0, -1));
    } else {
      setConfirmPin((p) => p.slice(0, -1));
    }
  };

  const handleClear = () => {
    if (busy || isLocked) return;
    setError(null);
    if (stage === 'enter') setPin('');
    else setConfirmPin('');
  };

  const submit = async (p: string) => {
    setBusy(true);
    setError(null);
    try {
      if (mode === 'setup') {
        await setParentPin(p);
        onSuccess(p);
      } else {
        const ok = await verifyParentPin(p);
        if (ok) {
          onSuccess(p);
        } else {
          const newAttempts = attempts + 1;
          setAttempts(newAttempts);
          setPin('');
          if (newAttempts >= MAX_ATTEMPTS) {
            setLockedUntil(Date.now() + LOCKOUT_SECONDS * 1000);
            setError(`错误次数过多, 已锁定 ${LOCKOUT_SECONDS} 秒`);
          } else {
            setError(`PIN 错误 (剩 ${MAX_ATTEMPTS - newAttempts} 次)`);
          }
        }
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleContinue = async () => {
    if (busy || isLocked) return;
    if (mode === 'setup' && stage === 'enter') {
      // 进入确认阶段
      setStage('confirm');
      return;
    }
    if (mode === 'setup' && stage === 'confirm') {
      if (pin !== confirmPin) {
        setError('两次输入不一致, 请重新输入');
        setStage('enter');
        setPin('');
        setConfirmPin('');
        return;
      }
      await submit(pin);
      return;
    }
    // verify
    if (pin.length !== PIN_LENGTH) {
      setError(`PIN 须 ${PIN_LENGTH} 位`);
      return;
    }
    await submit(pin);
  };

  const activePin = stage === 'enter' ? pin : confirmPin;
  const canContinue = activePin.length === PIN_LENGTH && !busy && !isLocked;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      data-testid="parent-pin-dialog"
    >
      <div className="bg-white rounded-2xl shadow-2xl p-6 w-[360px] max-w-[92vw]">
        <h2 className="text-xl font-semibold text-gray-800 mb-2">{title}</h2>
        <p className="text-sm text-gray-600 mb-4">
          {hint ??
            (mode === 'setup'
              ? '设置 4 位数字 PIN, 用于安装 skill 和切换模式'
              : '输入 4 位数字 PIN 解锁家长操作')}
        </p>

        {/* PIN dots */}
        <div className="flex justify-center gap-3 mb-4" data-testid="pin-dots">
          {Array.from({ length: PIN_LENGTH }).map((_, i) => (
            <div
              key={i}
              className={`w-12 h-14 rounded-lg border-2 flex items-center justify-center text-2xl font-bold ${
                activePin.length > i
                  ? 'border-blue-500 bg-blue-50 text-blue-700'
                  : 'border-gray-300 bg-gray-50 text-gray-400'
              }`}
            >
              {activePin.length > i ? '•' : ''}
            </div>
          ))}
        </div>

        {stage === 'confirm' && (
          <p className="text-xs text-blue-600 text-center mb-2">
            再输入一次确认
          </p>
        )}

        {isLocked ? (
          <p className="text-sm text-red-600 text-center mb-4" data-testid="lockout">
            已锁定, 请 {lockSecondsLeft} 秒后再试
          </p>
        ) : error ? (
          <p className="text-sm text-red-600 text-center mb-4" data-testid="pin-error">
            {error}
          </p>
        ) : (
          <div className="h-9 mb-4" />
        )}

        {/* Number pad */}
        <div className="grid grid-cols-3 gap-2 mb-4">
          {[1, 2, 3, 4, 5, 6, 7, 8, 9].map((d) => (
            <button
              key={d}
              type="button"
              className="h-12 rounded-lg bg-gray-100 hover:bg-gray-200 active:bg-gray-300 text-lg font-medium disabled:opacity-50"
              onClick={() => handleDigit(d.toString())}
              disabled={busy || isLocked}
              data-testid={`pin-key-${d}`}
            >
              {d}
            </button>
          ))}
          <button
            type="button"
            className="h-12 rounded-lg bg-gray-100 hover:bg-gray-200 text-sm"
            onClick={handleClear}
            disabled={busy || isLocked}
            data-testid="pin-clear"
          >
            清空
          </button>
          <button
            type="button"
            className="h-12 rounded-lg bg-gray-100 hover:bg-gray-200 text-lg font-medium disabled:opacity-50"
            onClick={() => handleDigit('0')}
            disabled={busy || isLocked}
            data-testid="pin-key-0"
          >
            0
          </button>
          <button
            type="button"
            className="h-12 rounded-lg bg-gray-100 hover:bg-gray-200 text-sm"
            onClick={handleBackspace}
            disabled={busy || isLocked}
            data-testid="pin-back"
          >
            ←
          </button>
        </div>

        <div className="flex gap-2">
          <button
            type="button"
            className="flex-1 h-10 rounded-lg bg-gray-200 hover:bg-gray-300 text-gray-700"
            onClick={onCancel}
            disabled={busy}
          >
            取消
          </button>
          <button
            type="button"
            className="flex-1 h-10 rounded-lg bg-blue-600 hover:bg-blue-700 text-white disabled:opacity-50"
            onClick={handleContinue}
            disabled={!canContinue}
            data-testid="pin-submit"
          >
            {busy ? '验证中…' : mode === 'setup' && stage === 'enter' ? '下一步' : '确认'}
          </button>
        </div>
      </div>
    </div>
  );
}