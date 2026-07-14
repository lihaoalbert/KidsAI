// W9: 故事状态网格 — 把 7 维 spine + narrative + 资产状态显示成"数据已填多少"的网格
// 不门控任何字段；任何格子可点 → 调对应 action
// P0 fix: 用内联 PromptDialog 替代 window.prompt()

import { useState } from 'react';
import { useDirectorStore } from '../../stores/directorStore';
import type { StoryTone } from '../../stores/directorStore';
import PromptDialog from '../ui/PromptDialog';

const TONE_LABEL: Record<StoryTone, string> = {
  playful: '🎈 欢乐',
  epic: '⚔️ 史诗',
  healing: '🌸 治愈',
  comedy: '😄 喜剧',
  mystery: '🔮 神秘',
  serious: '🎯 严肃',
  romantic: '💕 浪漫',
};

interface GridCell {
  label: string;
  emoji: string;
  value: string;
  filled: boolean;
  onClick?: () => void;
}

type EditTarget =
  | { kind: 'spine'; field: 'core' | 'conflict' | 'world' | 'audience' | 'theme_color' | 'ending_moral'; title: string; placeholder?: string; hint?: string }
  | { kind: 'narrative' }
  | null;

export default function StoryStateGrid() {
  const story = useDirectorStore((s) => s.story);
  const character = useDirectorStore((s) => s.character);
  const style = useDirectorStore((s) => s.style);
  const shots = useDirectorStore((s) => s.shots);
  const finalVideoUrl = useDirectorStore((s) => s.finalVideoUrl);
  const setSpineField = useDirectorStore((s) => s.setSpineField);
  const setNarrative = useDirectorStore((s) => s.setNarrative);
  const [editTarget, setEditTarget] = useState<EditTarget>(null);

  const openSpinePrompt = (
    field: 'core' | 'conflict' | 'world' | 'audience' | 'theme_color' | 'ending_moral',
    title: string,
    placeholder?: string,
    hint?: string,
  ) => setEditTarget({ kind: 'spine', field, title, placeholder, hint });

  const cells: GridCell[] = [
    {
      label: '内核',
      emoji: '✨',
      value: story.spine.core || '—',
      filled: Boolean(story.spine.core),
      onClick: () => openSpinePrompt('core', '故事内核（勇气 / 友情 / 成长…）', '例如：勇气 / 友情 / 成长'),
    },
    {
      label: '冲突',
      emoji: '⚡',
      value: story.spine.conflict || '—',
      filled: Boolean(story.spine.conflict),
      onClick: () => openSpinePrompt('conflict', '冲突（主角 vs 什么）', '例如：小狮子 vs 内心的恐惧'),
    },
    {
      label: '世界观',
      emoji: '🌍',
      value: story.spine.world || '—',
      filled: Boolean(story.spine.world),
      onClick: () => openSpinePrompt('world', '世界观（火山世界 / 现代都市 / 太空站…）'),
    },
    {
      label: '调性',
      emoji: '🎭',
      value: TONE_LABEL[story.spine.tone],
      filled: true, // 永远有值（默认 playful）
      onClick: () => {
        const tones: StoryTone[] = ['playful', 'epic', 'healing', 'comedy', 'mystery', 'serious', 'romantic'];
        const idx = tones.indexOf(story.spine.tone);
        const next = tones[(idx + 1) % tones.length];
        setSpineField('tone', next);
      },
    },
    {
      label: '适龄',
      emoji: '👶',
      value: story.spine.audience || '—',
      filled: Boolean(story.spine.audience),
      onClick: () => openSpinePrompt('audience', '适龄（6-9 / 10-13 / 14+）'),
    },
    {
      label: '主色',
      emoji: '🎨',
      value: story.spine.theme_color || '—',
      filled: Boolean(story.spine.theme_color),
      onClick: () => openSpinePrompt('theme_color', '主色调（暖橙 / 冷蓝 / 莫兰迪…）'),
    },
    {
      label: '寓意',
      emoji: '🌟',
      value: story.spine.ending_moral || '—',
      filled: Boolean(story.spine.ending_moral),
      onClick: () => openSpinePrompt('ending_moral', '结尾寓意'),
    },
    // 资产状态
    {
      label: '主角',
      emoji: '🎭',
      value: character ? character.name : '—',
      filled: Boolean(character),
    },
    {
      label: '风格',
      emoji: '🎨',
      value: style ? style.name : '—',
      filled: Boolean(style),
    },
    {
      label: '分镜',
      emoji: '🎬',
      value: shots.length > 0 ? `${shots.length} 镜` : '—',
      filled: shots.length > 0,
    },
    {
      label: '视频',
      emoji: '🎞️',
      value: finalVideoUrl ? '已合成' : '未合成',
      filled: Boolean(finalVideoUrl),
    },
  ];

  const narrativeSummary = story.narrative.paragraphs.length > 0
    ? story.narrative.paragraphs.slice(0, 2).join(' / ') + (story.narrative.paragraphs.length > 2 ? '…' : '')
    : '（点空白处让 AI 充实剧本）';

  const promptDefault = (): string => {
    if (!editTarget) return '';
    if (editTarget.kind === 'spine') {
      return (story.spine[editTarget.field] as string) ?? '';
    }
    return story.narrative.paragraphs.join('\n\n');
  };

  const promptTitle = (): string => {
    if (!editTarget) return '';
    if (editTarget.kind === 'spine') return editTarget.title;
    return '剧本段落（用空行分段）';
  };

  const promptHint = (): string | undefined => {
    if (editTarget?.kind === 'spine') return editTarget.hint;
    return '用空行分段，每段一段剧情';
  };

  return (
    <div className="space-y-3">
      {/* 7 维骨架 + 资产状态 — 4 列网格 */}
      <div className="grid grid-cols-2 gap-1.5 lg:grid-cols-4">
        {cells.map((cell) => (
          <button
            key={cell.label}
            type="button"
            onClick={cell.onClick}
            disabled={!cell.onClick}
            title={cell.value}
            className={[
              'rounded-lg border px-2 py-1.5 text-left text-xs transition-all',
              cell.filled
                ? 'border-accent-line bg-surface text-ink-2 hover:border-accent hover:bg-accent-soft'
                : 'border-dashed border-line bg-surface-2 text-ink-3',
              cell.onClick ? 'cursor-pointer' : 'cursor-default',
            ].join(' ')}
          >
            <div className="flex items-center gap-1 text-[10px] text-ink-2">
              <span>{cell.emoji}</span>
              <span>{cell.label}</span>
            </div>
            <div className="mt-0.5 truncate font-semibold">{cell.value}</div>
          </button>
        ))}
      </div>

      {/* narrative 摘要 — 可点编辑 */}
      <button
        type="button"
        onClick={() => setEditTarget({ kind: 'narrative' })}
        className="block w-full rounded-lg border border-line bg-surface px-3 py-2 text-left text-xs text-ink-2 hover:border-accent-line hover:bg-accent-soft"
      >
        <div className="mb-0.5 text-[10px] font-semibold text-ink-2">📖 剧本</div>
        <div className="line-clamp-3 leading-relaxed">{narrativeSummary}</div>
      </button>

      <PromptDialog
        open={editTarget !== null}
        title={promptTitle()}
        defaultValue={promptDefault()}
        placeholder={editTarget?.kind === 'spine' ? editTarget.placeholder : undefined}
        hint={promptHint()}
        onCancel={() => setEditTarget(null)}
        onConfirm={(v) => {
          if (!editTarget) return;
          if (editTarget.kind === 'spine') {
            if (v) setSpineField(editTarget.field, v);
          } else {
            const paragraphs = v
              .split(/\n\s*\n/)
              .map((p) => p.trim())
              .filter(Boolean);
            setNarrative(paragraphs);
          }
          setEditTarget(null);
        }}
      />
    </div>
  );
}
