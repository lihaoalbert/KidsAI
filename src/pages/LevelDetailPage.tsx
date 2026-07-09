import { useMemo } from 'react';
import Button from '../components/Button';
import Card from '../components/Card';
import { getLevel } from '../data/levels';

interface LevelDetailPageProps {
  levelId?: string;
  onBack?: () => void;
  onStart?: (levelId: string) => void;
}

export default function LevelDetailPage({
  levelId,
  onBack,
  onStart,
}: LevelDetailPageProps) {
  const resolvedId = levelId ?? 'L1';
  const level = useMemo(() => getLevel(resolvedId), [resolvedId]);

  if (!level) {
    return (
      <div className="p-8 max-w-3xl mx-auto">
        <Card>
          <div className="text-center py-12">
            <div className="text-5xl mb-3">🤔</div>
            <p className="text-gray-700">找不到这个关卡：{resolvedId}</p>
            {onBack && (
              <div className="mt-4">
                <Button variant="secondary" onClick={onBack}>
                  返回首页
                </Button>
              </div>
            )}
          </div>
        </Card>
      </div>
    );
  }

  const totalMinutes = level.estimatedMinutes;
  const prereqs = level.prerequisites;

  return (
    <div className="p-8 max-w-4xl mx-auto">
      {/* 顶部导航 */}
      <div className="mb-4 flex items-center text-sm text-gray-500">
        <button
          onClick={onBack}
          className="hover:text-brand-600 transition-colors"
        >
          ← 课程中心
        </button>
        <span className="mx-2">/</span>
        <span>第 {level.orderNum} 关</span>
      </div>

      {/* 关卡头部 */}
      <Card variant="elevated" className="mb-6">
        <div className="flex items-start gap-6">
          <div className="w-24 h-24 rounded-xl bg-gradient-to-br from-brand-100 to-warm-100 flex items-center justify-center text-5xl flex-shrink-0">
            {level.coverEmoji}
          </div>
          <div className="flex-1">
            <div className="flex items-center gap-2 mb-1">
              <span className="text-xs font-mono text-brand-600 bg-brand-50 px-2 py-0.5 rounded">
                {level.id}
              </span>
              <span className="text-xs text-gray-500">
                {'★'.repeat(level.difficulty)}
                <span className="text-gray-300">
                  {'★'.repeat(5 - level.difficulty)}
                </span>
              </span>
            </div>
            <h1 className="text-2xl font-bold text-gray-900 mb-2">
              {level.title}
            </h1>
            <p className="text-sm text-gray-600 mb-3">{level.description}</p>
            <div className="flex items-center gap-4 text-xs text-gray-600">
              <span>⏱ 约 {totalMinutes} 分钟</span>
              <span>💎 完成奖励 {level.rewardTokens} 学币</span>
              <span>
                🤖 AI 老师：{level.aiName} {level.aiAvatar}
              </span>
            </div>
          </div>
        </div>
      </Card>

      {/* 前置关卡提示 */}
      {prereqs.length > 0 && (
        <Card className="mb-6 bg-amber-50 border-amber-200">
          <div className="flex items-start gap-3">
            <div className="text-xl">🔒</div>
            <div className="text-sm text-amber-900">
              <div className="font-medium mb-1">需要先完成：</div>
              <div className="flex flex-wrap gap-2">
                {prereqs.map((p) => (
                  <span
                    key={p}
                    className="px-2 py-0.5 bg-white border border-amber-300 rounded text-xs"
                  >
                    {p}
                  </span>
                ))}
              </div>
            </div>
          </div>
        </Card>
      )}

      {/* 步骤列表 */}
      <div className="mb-6">
        <h2 className="text-lg font-semibold text-gray-900 mb-3">
          📋 闯关步骤（{level.steps.length} 步）
        </h2>
        <div className="space-y-3">
          {level.steps.map((step) => (
            <Card key={step.id} variant="bordered">
              <div className="flex items-start gap-4">
                <div className="w-9 h-9 rounded-full bg-brand-500 text-white flex items-center justify-center font-semibold text-sm flex-shrink-0">
                  {step.orderNum}
                </div>
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-1">
                    <h3 className="font-semibold text-gray-900">
                      {step.title}
                    </h3>
                    <span className="text-xs px-2 py-0.5 rounded bg-gray-100 text-gray-600">
                      {step.type === 'input' && '✏️ 填写'}
                      {step.type === 'choice' && '🔘 选择'}
                      {step.type === 'action' && '👀 观看'}
                      {step.type === 'free' && '🎨 自由'}
                    </span>
                  </div>
                  <p className="text-sm text-gray-600">{step.instruction}</p>
                  {step.placeholder && (
                    <div className="mt-2 text-xs text-gray-500 italic">
                      提示：{step.placeholder}
                    </div>
                  )}
                </div>
              </div>
            </Card>
          ))}
        </div>
      </div>

      {/* 评分标准 */}
      <Card className="mb-6">
        <h2 className="text-base font-semibold text-gray-900 mb-3">
          ⭐ 评分维度
        </h2>
        <div className="grid grid-cols-5 gap-2">
          {Object.entries(level.scoringCriteria).map(([key, value]) => {
            const labels: Record<string, string> = {
              creativity: '创意',
              technical: '技术',
              narrative: '叙事',
              aesthetic: '美感',
              compliance: '合规',
            };
            return (
              <div
                key={key}
                className="text-center p-2 bg-gray-50 rounded-md"
              >
                <div className="text-lg font-bold text-brand-600">
                  {value}
                </div>
                <div className="text-xs text-gray-600">
                  {labels[key] ?? key}
                </div>
              </div>
            );
          })}
        </div>
      </Card>

      {/* 开始按钮 */}
      <div className="sticky bottom-0 bg-warm-50 -mx-8 px-8 py-4 border-t border-gray-200">
        <div className="flex items-center justify-between max-w-4xl mx-auto">
          <div className="text-sm text-gray-600">
            准备好开始挑战了吗？完成后可获得{' '}
            <span className="font-semibold text-brand-600">
              {level.rewardTokens} 学币
            </span>
          </div>
          <div className="flex gap-3">
            {onBack && (
              <Button variant="secondary" onClick={onBack}>
                再看看
              </Button>
            )}
            <Button
              variant="primary"
              size="lg"
              onClick={() => onStart?.(level.id)}
            >
              🚀 开始挑战
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
