// 关卡运行页（W2.8 - L1 全流程）
// 步骤：填表 -> 调 agentStore.send() -> 实时显示事件流 -> 展示资产

import { useEffect, useMemo, useRef, useState } from 'react';
import { useParams } from 'react-router-dom';
import Button from '../components/Button';
import Card from '../components/Card';
import { useAgentStore } from '../stores/agentStore';
import { useLevelStore } from '../stores/levelStore';
import { checkSafety, saveCreation } from '../api/tauri';

interface AgentRunnerPageProps {
  levelId?: string;
  onBack?: () => void;
}

export default function AgentRunnerPage({ levelId, onBack }: AgentRunnerPageProps) {
  const params = useParams<{ id: string }>();
  const resolvedId = levelId ?? params.id ?? 'L1';
  const { levels, submitLevel } = useLevelStore();
  const {
    messages,
    assets,
    isRunning,
    error,
    lastResponse,
    send,
    cancel,
    reset,
  } = useAgentStore();

  const level = useMemo(
    () => levels.find((l) => l.id === resolvedId),
    [levels, resolvedId],
  );

  const [userInput, setUserInput] = useState('');
  const [safetyWarning, setSafetyWarning] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    reset();
  }, [resolvedId, reset]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  if (!level) {
    return (
      <div className="p-8 max-w-3xl mx-auto">
        <Card>
          <div className="text-center py-12">
            <p className="text-gray-700">找不到关卡：{resolvedId}</p>
            {onBack && (
              <div className="mt-4">
                <Button variant="secondary" onClick={onBack}>返回</Button>
              </div>
            )}
          </div>
        </Card>
      </div>
    );
  }

  const inputStep = level.steps.find((s) => s.type === 'input');

  const handleSubmit = async () => {
    if (!userInput.trim() || !inputStep) return;
    // 入口审核
    const verdict = await checkSafety(userInput);
    if (typeof verdict === 'object' && 'block' in verdict) {
      setSafetyWarning(`🚫 ${verdict.block.reason}`);
      return;
    }
    if (typeof verdict === 'object' && 'warn' in verdict) {
      setSafetyWarning(`⚠️ ${verdict.warn.reason}`);
    } else {
      setSafetyWarning(null);
    }
    await send(level.id, userInput, level.systemPrompt);
  };

  const handleSubmitForScore = async () => {
    if (!lastResponse) return;
    // MVP：mock 评分 = 60 + 字符数因子（纯前端逻辑，便于演示）
    const creativity = Math.min(30, 10 + Math.floor(userInput.length / 3));
    const technical = 20;
    const narrative = 15;
    const aesthetic = 12;
    const compliance = 10;
    const total = creativity + technical + narrative + aesthetic + compliance;
    const rubric = { creativity, technical, narrative, aesthetic, compliance };
    const feedback = `你在「${level.title}」做出了一个 5 秒小视频！\n\n小启觉得你写的描述里有 ${userInput.length} 个字，下次试试 5 要素（颜色+品种+装饰+场景+动作）会让视频更具体哦～`;

    await submitLevel(level.id, total, rubric, feedback);

    // 保存作品（W2.3）
    try {
      await saveCreation({
        id: lastResponse.sessionId,
        levelId: level.id,
        userInput,
        agentOutput: lastResponse as unknown as Record<string, unknown>,
        score: total,
        rubric,
        feedback,
        assets,
      });
    } catch (e) {
      // 静默失败，不影响主流程
      console.warn('save_creation failed:', e);
    }

    alert(`🎉 提交成功！得分 ${total}/100`);
    onBack?.();
  };

  const handleCancel = async () => {
    await cancel();
  };

  return (
    <div className="p-6 max-w-5xl mx-auto h-full flex flex-col">
      {/* Header */}
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-3 text-sm text-gray-500">
          <button onClick={onBack} className="hover:text-brand-600">← 返回</button>
          <span>/</span>
          <span className="font-mono text-brand-600">{level.id}</span>
          <span className="font-semibold text-gray-900">{level.title}</span>
        </div>
        <div className="text-xs text-gray-500 flex items-center gap-2">
          {level.aiAvatar} <span>AI 老师：{level.aiName}</span>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-4 flex-1 min-h-0">
        {/* 左：关卡指令 */}
        <div className="col-span-1 space-y-3 overflow-auto">
          <Card variant="bordered">
            <h3 className="font-semibold text-gray-900 mb-2">📋 任务</h3>
            <p className="text-sm text-gray-700 mb-2">{level.description}</p>
            {inputStep && (
              <div className="text-xs text-gray-600 bg-brand-50 rounded p-2">
                💡 {inputStep.hint ?? inputStep.instruction}
              </div>
            )}
          </Card>

          <Card>
            <h3 className="font-semibold text-gray-900 mb-2">🎯 评分维度</h3>
            <div className="space-y-1 text-xs">
              {Object.entries(level.scoringCriteria).map(([k, v]) => (
                <div key={k} className="flex justify-between">
                  <span className="text-gray-600">
                    {k === 'creativity' ? '创意' :
                     k === 'technical' ? '技术' :
                     k === 'narrative' ? '叙事' :
                     k === 'aesthetic' ? '美感' : '合规'}
                  </span>
                  <span className="text-brand-600 font-mono">{v} 分</span>
                </div>
              ))}
            </div>
          </Card>
        </div>

        {/* 中：对话流 */}
        <div className="col-span-2 flex flex-col min-h-0">
          <Card className="flex-1 flex flex-col min-h-0">
            <div className="flex-1 overflow-auto space-y-3 pr-1">
              {messages.length === 0 && (
                <div className="text-sm text-gray-400 text-center py-8">
                  {level.aiAvatar} {level.aiName} 准备好了，输入你的想法开始吧～
                </div>
              )}
              {messages.map((m) => (
                <div
                  key={m.id}
                  className={[
                    'rounded-lg p-3 text-sm',
                    m.role === 'user'
                      ? 'bg-brand-50 ml-8'
                      : m.role === 'assistant'
                      ? 'bg-warm-50 mr-8'
                      : m.role === 'thought'
                      ? 'bg-gray-50 text-gray-600 text-xs italic'
                      : m.role === 'tool'
                      ? 'bg-blue-50 text-blue-900 text-xs'
                      : 'bg-yellow-50 text-yellow-900 text-xs',
                  ].join(' ')}
                >
                  <div className="text-xs text-gray-500 mb-1">
                    {m.role === 'user' ? '🧒 你' :
                     m.role === 'assistant' ? `${level.aiAvatar} ${level.aiName}` :
                     m.role === 'thought' ? `💭 思考 ${m.meta?.step ? `#${m.meta.step}` : ''}` :
                     m.role === 'tool' ? `🔧 ${m.meta?.toolName ?? 'tool'}` : '⚙️ 系统'}
                  </div>
                  <div className="whitespace-pre-wrap">{m.content}</div>
                </div>
              ))}
              {isRunning && (
                <div className="text-xs text-gray-500 italic px-3">
                  {level.aiAvatar} {level.aiName} 正在思考…
                </div>
              )}
              {error && !isRunning && (
                <div className="text-xs px-3 py-1 rounded bg-red-50 text-red-700 border border-red-200">
                  {error}
                </div>
              )}
              <div ref={messagesEndRef} />
            </div>

            {/* 输入区 */}
            <div className="mt-3 pt-3 border-t border-gray-100">
              {safetyWarning && (
                <div className="mb-2 text-xs px-2 py-1 rounded bg-amber-50 text-amber-900 border border-amber-200">
                  {safetyWarning}
                </div>
              )}
              <textarea
                value={userInput}
                onChange={(e) => setUserInput(e.target.value)}
                placeholder={inputStep?.placeholder ?? '在这里输入你的想法…'}
                className="w-full text-sm border border-gray-300 rounded-md p-2 resize-none focus:outline-none focus:border-brand-500"
                rows={3}
                disabled={isRunning}
              />
              <div className="mt-2 flex items-center justify-between">
                <div className="text-xs text-gray-500">
                  {error && <span className="text-red-600">⚠️ {error}</span>}
                </div>
                <div className="flex gap-2">
                  {lastResponse && !isRunning && (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={handleSubmitForScore}
                    >
                      提交并查看评分
                    </Button>
                  )}
                  {isRunning ? (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={handleCancel}
                    >
                      取消
                    </Button>
                  ) : (
                    <Button
                      variant="primary"
                      size="sm"
                      onClick={handleSubmit}
                      disabled={!userInput.trim()}
                    >
                      🚀 开始生成
                    </Button>
                  )}
                </div>
              </div>
            </div>
          </Card>

          {/* 资产展示 */}
          {assets.length > 0 && (
            <Card className="mt-3">
              <h3 className="font-semibold text-gray-900 mb-2 text-sm">🎁 生成的资产</h3>
              <div className="grid grid-cols-2 gap-2">
                {assets.map((a, i) => (
                  <div key={i} className="border border-gray-200 rounded-md overflow-hidden">
                    {a.type === 'image' && (
                      <img
                        src={a.url}
                        alt={a.prompt}
                        className="w-full aspect-video object-cover bg-gray-100"
                        loading="lazy"
                      />
                    )}
                    {a.type === 'video' && (
                      <video
                        src={a.url}
                        poster={a.thumbnailUrl}
                        controls
                        className="w-full aspect-video bg-black"
                      />
                    )}
                    {a.type === 'audio' && (
                      <div className="p-3 bg-gray-50">
                        <audio src={a.url} controls className="w-full" />
                      </div>
                    )}
                    <div className="p-2 text-xs text-gray-600 truncate">
                      {a.tool} · {a.prompt}
                    </div>
                  </div>
                ))}
              </div>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}
