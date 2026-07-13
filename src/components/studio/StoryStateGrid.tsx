// W9: 故事状态网格 — 把 7 维 spine + narrative + 资产状态显示成"数据已填多少"的网格
// 不门控任何字段；任何格子可点 → 调对应 action

import { useDirectorStore } from '../../stores/directorStore';
import type { StoryTone } from '../../stores/directorStore';

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

export default function StoryStateGrid() {
  const story = useDirectorStore((s) => s.story);
  const character = useDirectorStore((s) => s.character);
  const style = useDirectorStore((s) => s.style);
  const shots = useDirectorStore((s) => s.shots);
  const finalVideoUrl = useDirectorStore((s) => s.finalVideoUrl);
  const setSpineField = useDirectorStore((s) => s.setSpineField);
  const setNarrative = useDirectorStore((s) => s.setNarrative);

  const cells: GridCell[] = [
    {
      label: '内核',
      emoji: '✨',
      value: story.spine.core || '—',
      filled: Boolean(story.spine.core),
      onClick: () => {
        const v = prompt('故事内核（勇气 / 友情 / 成长…）', story.spine.core);
        if (v !== null) setSpineField('core', v.trim());
      },
    },
    {
      label: '冲突',
      emoji: '⚡',
      value: story.spine.conflict || '—',
      filled: Boolean(story.spine.conflict),
      onClick: () => {
        const v = prompt('冲突（主角 vs 什么）', story.spine.conflict);
        if (v !== null) setSpineField('conflict', v.trim());
      },
    },
    {
      label: '世界观',
      emoji: '🌍',
      value: story.spine.world || '—',
      filled: Boolean(story.spine.world),
      onClick: () => {
        const v = prompt('世界观（火山世界 / 现代都市 / 太空站…）', story.spine.world);
        if (v !== null) setSpineField('world', v.trim());
      },
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
      onClick: () => {
        const v = prompt('适龄（6-9 / 10-13 / 14+）', story.spine.audience);
        if (v !== null) setSpineField('audience', v.trim());
      },
    },
    {
      label: '主色',
      emoji: '🎨',
      value: story.spine.theme_color || '—',
      filled: Boolean(story.spine.theme_color),
      onClick: () => {
        const v = prompt('主色调（暖橙 / 冷蓝 / 莫兰迪…）', story.spine.theme_color);
        if (v !== null) setSpineField('theme_color', v.trim());
      },
    },
    {
      label: '寓意',
      emoji: '🌟',
      value: story.spine.ending_moral || '—',
      filled: Boolean(story.spine.ending_moral),
      onClick: () => {
        const v = prompt('结尾寓意', story.spine.ending_moral);
        if (v !== null) setSpineField('ending_moral', v.trim());
      },
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
                ? 'border-brand-200 bg-white text-gray-700 hover:border-brand-400 hover:bg-brand-50'
                : 'border-dashed border-gray-200 bg-gray-50 text-gray-400',
              cell.onClick ? 'cursor-pointer' : 'cursor-default',
            ].join(' ')}
          >
            <div className="flex items-center gap-1 text-[10px] text-gray-500">
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
        onClick={() => {
          const current = story.narrative.paragraphs.join('\n\n');
          const v = prompt('剧本段落（用空行分段）', current);
          if (v !== null) {
            const paragraphs = v.split(/\n\s*\n/).map((p) => p.trim()).filter(Boolean);
            setNarrative(paragraphs);
          }
        }}
        className="block w-full rounded-lg border border-gray-100 bg-white px-3 py-2 text-left text-xs text-gray-600 hover:border-brand-200 hover:bg-brand-50"
      >
        <div className="mb-0.5 text-[10px] font-semibold text-gray-500">📖 剧本</div>
        <div className="line-clamp-3 leading-relaxed">{narrativeSummary}</div>
      </button>
    </div>
  );
}