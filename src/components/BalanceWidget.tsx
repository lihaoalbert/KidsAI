// 学币余额展示 (W4.5 B2)
//
// 优先读后端真实余额, 失败/未激活时退到 tokenStore 兜底.
// 余额 < 10 时显示"找管理员加学币"提示.

import { useEffect, useState } from 'react';
import { getBalance, getLicenseInfo, type BalanceResponse } from '../api/tauri';
import { useTokenStore } from '../stores/tokenStore';

interface BalanceWidgetProps {
  /// 测试钩: 禁用自动 fetch (用 prop 传值)
  initialBalance?: number;
}

export default function BalanceWidget({
  initialBalance,
}: BalanceWidgetProps = {}) {
  const [info, setInfo] = useState<BalanceResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [demo, setDemo] = useState(false);
  const localBalance = useTokenStore((s) => s.balance);
  const setBalance = useTokenStore((s) => s.setBalance);

  useEffect(() => {
    if (initialBalance !== undefined) return;
    let cancelled = false;
    (async () => {
      try {
        const li = await getLicenseInfo();
        if (cancelled) return;
        setDemo(li?.isDemo ?? true);
        if (!li) {
          // 未激活: 保留 store 的 fallback
          return;
        }
        const b = await getBalance();
        if (cancelled) return;
        setInfo(b);
        setBalance(b.balance);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [initialBalance, setBalance]);

  const balance = info?.balance ?? initialBalance ?? localBalance;
  const lowBalance = balance < 10;

  return (
    <div className="inline-flex items-center gap-2 px-3 py-1.5 bg-amber-50 border border-amber-200 rounded-full text-sm">
      <span className="text-base">💎</span>
      <span className="font-semibold text-amber-900">{balance}</span>
      <span className="text-amber-700 text-xs">学币</span>
      {demo && (
        <span
          className="ml-1 text-[10px] text-amber-600 bg-amber-100 px-1.5 py-0.5 rounded"
          title="本地演示模式，未连接学币服务器"
        >
          演示
        </span>
      )}
      {info && (
        <span className="text-amber-700 text-xs hidden sm:inline">
          · 今日剩 {info.dailyRemaining}
        </span>
      )}
      {error && (
        <span className="text-red-500 text-xs hidden sm:inline">⚠ {error}</span>
      )}
      {lowBalance && (
        <span className="ml-1 text-[10px] text-red-600 bg-red-50 px-1.5 py-0.5 rounded">
          学币不足，找管理员加
        </span>
      )}
    </div>
  );
}