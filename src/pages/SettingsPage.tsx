// P0 fix: SettingsPage 替代 App.tsx 中 settings 路由跳 Marketplace 的隐藏 bug
// 提供: 家长 PIN 管理 / 学币详情 / 模式切换入口 / Secret 版本查看 / 关于

import { useEffect, useState } from 'react';
import Card from '../components/Card';
import Button from '../components/Button';
import { useTokenStore } from '../stores/tokenStore';
import { useUserModeStore } from '../stores/userModeStore';
import {
  getAppVersion,
  getCurrentSecretVersion,
  getLicenseInfo,
} from '../api/tauri';
import { ParentPinDialog } from '../components/system/ParentPinDialog';
import { ModeSwitchDialog } from '../components/system/ModeSwitchDialog';
import SecretsHistoryPanel from '../components/system/SecretsHistoryPanel';
import type { UserMode } from '../api/tauri';

export default function SettingsPage() {
  const balance = useTokenStore((s) => s.balance);
  const mode = useUserModeStore((s) => s.mode);
  const [version, setVersion] = useState<string>('');
  const [secretVersions, setSecretVersions] = useState<Record<string, string>>({});
  const [deviceId, setDeviceId] = useState<string>('');
  const [pinDialog, setPinDialog] = useState<null | 'setup' | 'reset'>(null);
  const [modeDialog, setModeDialog] = useState<UserMode | null>(null);
  const [pinSet, setPinSet] = useState<boolean | null>(null);

  useEffect(() => {
    void getAppVersion().then(setVersion).catch(() => setVersion('unknown'));
    void getCurrentSecretVersion()
      .then(setSecretVersions)
      .catch(() => setSecretVersions({}));
    void getLicenseInfo()
      .then((info) => setDeviceId(info?.deviceId ?? ''))
      .catch(() => setDeviceId(''));
  }, []);

  const handleModeSwitchClose = async () => {
    setModeDialog(null);
  };

  return (
    <div className="p-8 max-w-4xl mx-auto">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-gray-900">⚙️ 家长设置</h1>
        <p className="text-base text-gray-600 mt-1">
          家长专属管理面板 — 学币 / 模式 / 安全 / 关于
        </p>
      </div>

      {/* 学币详情 */}
      <section className="mb-6">
        <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider mb-3">
          学币
        </h2>
        <Card>
          <div className="flex items-center justify-between">
            <div>
              <div className="text-xs text-gray-500">当前余额</div>
              <div className="text-3xl font-bold text-brand-700 mt-1">💎 {balance}</div>
              <div className="text-xs text-gray-400 mt-1">
                ≈ 可生成 {Math.floor(balance / 5)} 个 5 秒视频
              </div>
            </div>
          </div>
        </Card>
      </section>

      {/* 模式切换 */}
      <section className="mb-6">
        <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider mb-3">
          模式
        </h2>
        <Card>
          <div className="flex items-center justify-between mb-3">
            <div>
              <div className="text-sm font-semibold text-gray-900">
                当前模式：{mode === 'child' ? '🧒 儿童模式' : '🧑 成人模式'}
              </div>
              <div className="text-xs text-gray-500 mt-1">
                {mode === 'child'
                  ? '强安全过滤 + 学币配额 + 年龄分级'
                  : '安全词放宽 + 不限题材 + 商用素材'}
              </div>
            </div>
          </div>
          <Button
            variant={mode === 'child' ? 'primary' : 'secondary'}
            size="md"
            onClick={() => setModeDialog(mode === 'child' ? 'adult' : 'child')}
          >
            {mode === 'child' ? '切到成人模式' : '切回儿童模式'}
          </Button>
          <p className="text-[11px] text-gray-400 mt-2">
            切换需要家长 PIN 验证, 双向都需要
          </p>
        </Card>
      </section>

      {/* 家长 PIN */}
      <section className="mb-6">
        <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider mb-3">
          家长 PIN
        </h2>
        <Card>
          <div className="text-sm text-gray-700 mb-3">
            {pinSet === null
              ? '检测中…'
              : pinSet
              ? '✅ 已设置 PIN'
              : '⚠️ 还未设置 PIN — 切模式 / 装 skill 都需要 PIN'}
          </div>
          <div className="flex gap-2">
            {!pinSet && (
              <Button
                variant="primary"
                size="sm"
                onClick={() => setPinDialog('setup')}
              >
                设置 PIN
              </Button>
            )}
            {pinSet && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setPinDialog('reset')}
              >
                重置 PIN
              </Button>
            )}
          </div>
        </Card>
      </section>

      {/* Secret 版本 + P1-5 多版本回滚 */}
      <section className="mb-6">
        <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider mb-3">
          系统状态
        </h2>
        <Card>
          <div className="space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-gray-600">App 版本</span>
              <span className="font-mono text-gray-900">{version || '…'}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-600">设备 ID</span>
              <span className="font-mono text-gray-900 text-xs">
                {deviceId ? deviceId.slice(0, 12) + '…' : '…'}
              </span>
            </div>
            <div>
              <div className="flex justify-between mb-1">
                <span className="text-gray-600">Secret 版本</span>
                <span className="font-mono text-gray-900 text-xs">
                  {Object.keys(secretVersions).length === 0
                    ? '未安装 / 走 fallback'
                    : Object.entries(secretVersions)
                        .map(([p, v]) => `${p}:${v}`)
                        .join(' / ')}
                </span>
              </div>
              {/* P1-5: 列出 child / adult profile 的历史版本, 提供回滚入口.
                  只在至少有一个 profile 已装 secret 时才展开, 避免空白. */}
              {Object.keys(secretVersions).length > 0 && (
                <div className="border-t pt-3 mt-2 space-y-3">
                  <SecretsHistoryPanel profile="child" />
                  {secretVersions.adult && (
                    <SecretsHistoryPanel profile="adult" />
                  )}
                </div>
              )}
            </div>
          </div>
        </Card>
      </section>

      {/* Mode Switch Dialog */}
      {modeDialog && (
        <ModeSwitchDialog
          open
          targetMode={modeDialog}
          onClose={handleModeSwitchClose}
        />
      )}

      {/* PIN Dialog */}
      {pinDialog && (
        <ParentPinDialog
          open
          mode={pinDialog === 'setup' ? 'setup' : undefined}
          title={pinDialog === 'setup' ? '设置家长 PIN' : '重置家长 PIN'}
          hint="PIN 用于: 切模式、装 skill、更新 Secret"
          onCancel={() => setPinDialog(null)}
          onSuccess={() => {
            setPinDialog(null);
            setPinSet(true);
          }}
        />
      )}
    </div>
  );
}