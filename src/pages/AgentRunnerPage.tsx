// 关卡运行页（W2.8 - L1 全流程）
// 步骤：填表 -> 调 agentStore.send() -> 实时显示事件流 -> 展示资产
// W3.4: 增加角色选择 — 左侧加 "🎭 选一个角色" 卡片，选中的角色会随 send() 传给 backend
// W3.5: 指哪打哪 — 生成的图片可点击 → 右侧抽屉 → 输入修改意图 → 触发新一轮生成
// W3.6: 增加风格选择 — 左侧加 "🎨 选一种风格" 卡片，与角色独立可叠加
// W3.7+: 拉片复刻 — 按 step.type 分支渲染（reference_setup / reference_recreate）
//         抽帧 + 顺序复刻全部由 store 内的 recreateFrames 处理

import { useEffect, useMemo, useRef, useState } from 'react';
import Button from '../components/Button';
import Card from '../components/Card';
import EditPanel from '../components/EditPanel';
import ReferenceVideoPicker from '../components/ReferenceVideoPicker';
import FrameSelector from '../components/FrameSelector';
import AlertDialog from '../components/ui/AlertDialog';
import { useAgentStore } from '../stores/agentStore';
import { useLevelStore } from '../stores/levelStore';
import { checkSafety, saveCreation, type AgentAsset } from '../api/tauri';
import type { Character, StylePreset } from '../api/tauri';

interface AgentRunnerPageProps {
  levelId?: string;
  onBack?: () => void;
}

export default function AgentRunnerPage({ levelId, onBack }: AgentRunnerPageProps) {
  const resolvedId = levelId ?? 'L1';
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
    loadCharacters,
    setCharacter,
    characters,
    character,
    loadStyles,
    setStyle,
    styles,
    style,
    // W3.5: 编辑抽屉状态 + actions
    editing,
    setEditing,
    clearEditing,
    editImageAsset,
    // W3.7+: 抽好的帧 + 整段复刻进度 + actions
    extractedFrames,
    setExtractedFrames,
    recreateProgress,
    recreateFrames,
  } = useAgentStore();

  const level = useMemo(
    () => levels.find((l) => l.id === resolvedId),
    [levels, resolvedId],
  );

  const [userInput, setUserInput] = useState('');
  const [safetyWarning, setSafetyWarning] = useState<string | null>(null);
  const [submitMessage, setSubmitMessage] = useState<string | null>(null);
  const [pickerMessage, setPickerMessage] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    reset();
  }, [resolvedId, reset]);

  // W3.4 + W3.6: 进入页面时拉一次角色 + 风格清单（内部有缓存）
  useEffect(() => {
    loadCharacters();
    loadStyles();
  }, [loadCharacters, loadStyles]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  if (!level) {
    return (
      <div className="p-8 max-w-3xl mx-auto">
        <Card>
          <div className="text-center py-12">
            <p className="text-ink-2">找不到关卡：{resolvedId}</p>
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
  // W3.7+: 拉片复刻两种 step type
  const setupStep = level.steps.find((s) => s.type === 'reference_setup');
  const recreateStep = level.steps.find((s) => s.type === 'reference_recreate');
  const isReferenceLevel = !!setupStep && !!recreateStep;

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

    setSubmitMessage(`🎉 提交成功！得分 ${total}/100`);
    // 用户点 "好" 后由 AlertDialog onClose 回调触发 onBack?.();
  };

  const handleCancel = async () => {
    await cancel();
  };

  return (
    // W3.7+: key={level.id} 强制在跨关时卸载整个子树,picker 状态、extractedFrames 全部清空
    <div key={level.id} className="p-6 max-w-5xl mx-auto h-full flex flex-col">
      {/* Header */}
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-3 text-sm text-ink-2">
          <button onClick={onBack} className="hover:text-accent-ink">← 返回</button>
          <span>/</span>
          <span className="font-mono text-accent-ink">{level.id}</span>
          <span className="font-semibold text-ink">{level.title}</span>
        </div>
        <div className="text-xs text-ink-2 flex items-center gap-2">
          {level.aiAvatar} <span>AI 老师：{level.aiName}</span>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-4 flex-1 min-h-0">
        {/* 左：关卡指令 + 角色选择 + 评分维度 */}
        <div className="col-span-1 space-y-3 overflow-auto">
          <Card variant="bordered">
            <h3 className="font-semibold text-ink mb-2">📋 任务</h3>
            <p className="text-sm text-ink-2 mb-2">{level.description}</p>
            {inputStep && (
              <div className="text-xs text-ink-2 bg-accent-soft rounded p-2">
                💡 {inputStep.hint ?? inputStep.instruction}
              </div>
            )}
          </Card>

          {/* W3.4: 角色一致性 — 让小朋友选一个角色，同一会话生图会保持形象一致 */}
          <Card variant="bordered">
            <h3 className="font-semibold text-ink mb-2">🎭 选一个角色</h3>
            <p className="text-xs text-ink-2 mb-3">
              选好后，本关多次生成的图片都会是这位主角～
            </p>
            <div className="space-y-1.5">
              {/* "不绑定" 选项 — 兼容旧行为 */}
              <button
                type="button"
                onClick={() => setCharacter(null)}
                disabled={isRunning}
                className={[
                  'w-full text-left text-xs px-2.5 py-2 rounded-md border transition-colors',
                  character === null
                    ? 'bg-accent-soft border-accent text-accent-ink font-medium'
                    : 'border-line hover:border-line text-ink-2',
                  isRunning ? 'opacity-50 cursor-not-allowed' : '',
                ].join(' ')}
              >
                🚫 不绑定角色
              </button>
              {characters.map((c: Character) => {
                const selected = character?.id === c.id;
                return (
                  <button
                    key={c.id}
                    type="button"
                    onClick={() => setCharacter(c)}
                    disabled={isRunning}
                    className={[
                      'w-full text-left text-xs px-2.5 py-2 rounded-md border transition-colors',
                      selected
                        ? 'bg-accent-soft border-accent text-accent-ink font-medium'
                        : 'border-line hover:border-line text-ink-2',
                      isRunning ? 'opacity-50 cursor-not-allowed' : '',
                    ].join(' ')}
                  >
                    <div className="font-semibold">{c.name}</div>
                    <div className="text-[11px] text-ink-2 mt-0.5 line-clamp-2">
                      {c.description}
                    </div>
                  </button>
                );
              })}
              {characters.length === 0 && (
                <div className="text-[11px] text-ink-3 italic px-1 py-1">
                  （暂无角色，模型默认按关卡指令生成）
                </div>
              )}
            </div>
          </Card>

          {/* W3.6: 风格模板切换 — 选一种视觉风格，与角色独立可叠加 */}
          <Card variant="bordered">
            <h3 className="font-semibold text-ink mb-2">🎨 选一种风格</h3>
            <p className="text-xs text-ink-2 mb-3">
              比如把「水墨」加到「小启」上，会得到水墨风的小启～
            </p>
            <div className="space-y-1.5">
              <button
                type="button"
                onClick={() => setStyle(null)}
                disabled={isRunning}
                className={[
                  'w-full text-left text-xs px-2.5 py-2 rounded-md border transition-colors',
                  style === null
                    ? 'bg-accent-soft border-accent text-accent-ink font-medium'
                    : 'border-line hover:border-line text-ink-2',
                  isRunning ? 'opacity-50 cursor-not-allowed' : '',
                ].join(' ')}
              >
                🚫 不绑定风格
              </button>
              {styles.map((s: StylePreset) => {
                const selected = style?.id === s.id;
                return (
                  <button
                    key={s.id}
                    type="button"
                    onClick={() => setStyle(s)}
                    disabled={isRunning}
                    className={[
                      'w-full text-left text-xs px-2.5 py-2 rounded-md border transition-colors',
                      selected
                        ? 'bg-accent-soft border-accent text-accent-ink font-medium'
                        : 'border-line hover:border-line text-ink-2',
                      isRunning ? 'opacity-50 cursor-not-allowed' : '',
                    ].join(' ')}
                  >
                    <div className="font-semibold">{s.name}</div>
                    <div className="text-[11px] text-ink-2 mt-0.5 line-clamp-2">
                      {s.description}
                    </div>
                  </button>
                );
              })}
              {styles.length === 0 && (
                <div className="text-[11px] text-ink-3 italic px-1 py-1">
                  （暂无风格，模型默认按关卡指令生成）
                </div>
              )}
            </div>
          </Card>

          <Card>
            <h3 className="font-semibold text-ink mb-2">🎯 评分维度</h3>
            <div className="space-y-1 text-xs">
              {Object.entries(level.scoringCriteria).map(([k, v]) => (
                <div key={k} className="flex justify-between">
                  <span className="text-ink-2">
                    {k === 'creativity' ? '创意' :
                     k === 'technical' ? '技术' :
                     k === 'narrative' ? '叙事' :
                     k === 'aesthetic' ? '美感' : '合规'}
                  </span>
                  <span className="text-accent-ink font-mono">{v} 分</span>
                </div>
              ))}
            </div>
          </Card>
        </div>

        {/* 中：对话流 */}
        <div className="col-span-2 flex flex-col min-h-0">
          <Card className="flex-1 flex flex-col min-h-0">
            {/* W3.4 + W3.6: 当前角色 + 风格 banner — 一眼能看到是不是自己选的那个 */}
            {(character || style) && (
              <div className="flex items-center justify-between mb-2 pb-2 border-b border-line gap-3 flex-wrap">
                <div className="flex items-center gap-4 text-xs text-ink-2 flex-wrap">
                  {character && (
                    <span>
                      🎭 当前角色：
                      <span className="font-semibold text-accent-ink ml-1">
                        {character.name}
                      </span>
                    </span>
                  )}
                  {style && (
                    <span>
                      🎨 当前风格：
                      <span className="font-semibold text-accent-ink ml-1">
                        {style.name}
                      </span>
                    </span>
                  )}
                  <span className="text-ink-3 text-[11px]">
                    （本次生成的图片都会保持这个形象 + 风格）
                  </span>
                </div>
                <div className="flex gap-2">
                  {character && (
                    <button
                      type="button"
                      onClick={() => setCharacter(null)}
                      disabled={isRunning}
                      className="text-[11px] text-ink-3 hover:text-ink-2 disabled:opacity-50"
                    >
                      清除角色
                    </button>
                  )}
                  {style && (
                    <button
                      type="button"
                      onClick={() => setStyle(null)}
                      disabled={isRunning}
                      className="text-[11px] text-ink-3 hover:text-ink-2 disabled:opacity-50"
                    >
                      清除风格
                    </button>
                  )}
                </div>
              </div>
            )}
            <div className="flex-1 overflow-auto space-y-3 pr-1">
              {messages.length === 0 && (
                <div className="text-sm text-ink-3 text-center py-8">
                  {level.aiAvatar} {level.aiName} 准备好了，输入你的想法开始吧～
                </div>
              )}
              {messages.map((m) => (
                <div
                  key={m.id}
                  className={[
                    'rounded-lg p-3 text-sm',
                    m.role === 'user'
                      ? 'bg-accent-soft ml-8'
                      : m.role === 'assistant'
                      ? 'bg-bg mr-8'
                      : m.role === 'thought'
                      ? 'bg-surface-2 text-ink-2 text-xs italic'
                      : m.role === 'tool'
                      ? 'bg-accent-soft text-accent-ink text-xs'
                      : 'bg-yellow-50 text-yellow-900 text-xs',
                  ].join(' ')}
                >
                  <div className="text-xs text-ink-2 mb-1">
                    {m.role === 'user' ? '🧒 你' :
                     m.role === 'assistant' ? `${level.aiAvatar} ${level.aiName}` :
                     m.role === 'thought' ? `💭 思考 ${m.meta?.step ? `#${m.meta.step}` : ''}` :
                     m.role === 'tool' ? `🔧 ${m.meta?.toolName ?? 'tool'}` : '⚙️ 系统'}
                  </div>
                  <div className="whitespace-pre-wrap">{m.content}</div>
                </div>
              ))}
              {isRunning && (
                <div className="text-xs text-ink-2 italic px-3">
                  {level.aiAvatar} {level.aiName} 正在思考…
                </div>
              )}
              {error && !isRunning && (
                <div className="text-xs px-3 py-1 rounded bg-danger-soft text-danger border border-danger-soft">
                  {error}
                </div>
              )}
              <div ref={messagesEndRef} />
            </div>

            {/* 输入区 — 按 step type 分支(L1-L5 文本输入,L6/L7 抽帧 + 复刻) */}
            <div className="mt-3 pt-3 border-t border-line">
              {safetyWarning && (
                <div className="mb-2 text-xs px-2 py-1 rounded bg-warning-soft text-warning border border-warning-soft">
                  {safetyWarning}
                </div>
              )}

              {setupStep ? (
                // ---------- W3.7+ 拉片复刻 ----------
                <ReferenceVideoPicker
                  onChange={setExtractedFrames}
                  onError={(msg) => setPickerMessage(msg)}
                />
              ) : (
                // ---------- L1-L5 文本输入 ----------
                <textarea
                  value={userInput}
                  onChange={(e) => setUserInput(e.target.value)}
                  placeholder={inputStep?.placeholder ?? '在这里输入你的想法…'}
                  className="w-full text-sm border border-line rounded-md p-2 resize-none focus:outline-none focus:border-accent"
                  rows={3}
                  disabled={isRunning}
                />
              )}

              {/* L6/L7 第二步:选帧复刻 — 仅在 extractedFrames 准备好之后渲染 */}
              {recreateStep && extractedFrames.length > 0 && (
                <div className="mt-3 pt-3 border-t border-line">
                  <FrameSelector
                    levelId={level.id}
                    frames={extractedFrames}
                    mode={recreateStep.mode ?? 'single'}
                    progress={recreateProgress ?? undefined}
                    isRunning={isRunning}
                    systemPrompt={level.systemPrompt}
                    tools={level.tools ?? []}
                    characterId={character?.id}
                    styleId={style?.id}
                    onRun={(params) =>
                      void recreateFrames({
                        levelId: params.systemPrompt ? level.id : level.id,
                        systemPrompt: params.systemPrompt,
                        frames: params.frames,
                        characterId: params.characterId,
                        styleId: params.styleId,
                        tools: params.tools,
                      })
                    }
                  />
                </div>
              )}

              <div className="mt-2 flex items-center justify-between">
                <div className="text-xs text-ink-2">
                  {error && <span className="text-danger">⚠️ {error}</span>}
                </div>
                <div className="flex gap-2">
                  {!isReferenceLevel && lastResponse && !isRunning && (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={handleSubmitForScore}
                    >
                      提交并查看评分
                    </Button>
                  )}
                  {/* L1-L5 文本输入 + 取消;L6/L7 复刻由 FrameSelector 自己管按钮 */}
                  {!isReferenceLevel && (
                    isRunning ? (
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
                    )
                  )}
                </div>
              </div>
            </div>
          </Card>

          {/* 资产展示 */}
          {assets.length > 0 && (
            <Card className="mt-3">
              <h3 className="font-semibold text-ink mb-2 text-sm">🎁 生成的资产</h3>
              <p className="text-[11px] text-ink-2 mb-2">
                ✏️ 点击图片任意位置可"指哪打哪"精修
              </p>
              <div className="grid grid-cols-2 gap-2">
                {assets.map((a, i) => (
                  <AssetTile
                    key={i}
                    asset={a}
                    isRunning={isRunning}
                    onClick={(clickX, clickY) => setEditing(a, clickX, clickY)}
                  />
                ))}
              </div>
            </Card>
          )}
        </div>
      </div>

      {/* W3.5: 编辑抽屉（独立组件） */}
      {editing && (
        <EditPanel
          asset={editing.asset}
          clickX={editing.clickX}
          clickY={editing.clickY}
          disabled={isRunning}
          onCancel={clearEditing}
          onSubmit={(prompt) => {
            if (!level) return;
            // 把关卡允许的工具都带上，edit_image 会被 store 自动追加
            const tools = level.tools ?? [];
            void editImageAsset({
              levelId: level.id,
              systemPrompt: level.systemPrompt,
              prompt,
              tools,
            });
          }}
        />
      )}

      <AlertDialog
        open={submitMessage !== null}
        title="提交成功"
        message={submitMessage ?? ''}
        onClose={() => {
          setSubmitMessage(null);
          onBack?.();
        }}
      />
      <AlertDialog
        open={pickerMessage !== null}
        title="参考视频出错"
        message={pickerMessage ?? ''}
        onClose={() => setPickerMessage(null)}
      />
    </div>
  );
}

/// 资产单格 — 单独抽出便于测试 + 点击坐标捕获
function AssetTile({
  asset,
  isRunning,
  onClick,
}: {
  asset: AgentAsset;
  isRunning: boolean;
  onClick: (clickX: number, clickY: number) => void;
}) {
  const handleImgClick = (e: React.MouseEvent<HTMLImageElement>) => {
    if (isRunning) return;
    if (asset.type !== 'image') return; // 视频 / 音频暂不支持指哪打哪
    const rect = e.currentTarget.getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width;
    const y = (e.clientY - rect.top) / rect.height;
    onClick(x, y);
  };

  // 编辑过的资产有 sourceAssetUrl → 加蓝色边框 + 底部缩略
  const hasSource = !!asset.sourceAssetUrl;

  return (
    <div
      className={[
        'border rounded-md overflow-hidden transition-colors',
        hasSource
          ? 'border-accent ring-1 ring-accent-line'
          : 'border-line hover:border-accent',
      ].join(' ')}
    >
      {/* Main image (clickable for image type) */}
      <div className="relative">
        {asset.type === 'image' && (
          <img
            src={asset.url}
            alt={asset.prompt}
            className={[
              'w-full aspect-video object-cover bg-surface-2',
              !isRunning ? 'cursor-crosshair' : 'cursor-not-allowed',
            ].join(' ')}
            loading="lazy"
            onClick={handleImgClick}
          />
        )}
        {asset.type === 'video' && (
          <video
            src={asset.url}
            poster={asset.thumbnailUrl}
            controls
            className="w-full aspect-video bg-ink"
          />
        )}
        {asset.type === 'audio' && (
          <div className="p-3 bg-surface-2">
            <audio src={asset.url} controls className="w-full" />
          </div>
        )}
        {/* 编辑产物角标 */}
        {hasSource && (
          <div className="absolute top-1 right-1 bg-accent text-bg text-[10px] px-1.5 py-0.5 rounded">
            ✏️ 精修
          </div>
        )}
      </div>

      {/* 编辑历史缩略（仅 edit 后的图） */}
      {hasSource && asset.sourceAssetUrl && (
        <div className="px-2 py-1.5 bg-accent-soft flex items-center gap-2">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            src={asset.sourceAssetUrl}
            alt="原图"
            className="w-10 h-6 object-cover rounded border border-line"
          />
          <span className="text-[10px] text-accent-ink">← 原图</span>
        </div>
      )}

      <div className="p-2 text-xs text-ink-2 truncate" title={asset.prompt}>
        {asset.tool} · {asset.prompt}
      </div>
    </div>
  );
}
