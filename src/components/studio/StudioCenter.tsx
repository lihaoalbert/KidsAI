// W9: 中栏 tab 容器 — 对话 / 剧本 / 分镜 / 角色 / 高级
// 同一份数据 (directorStore), 按用户深度渐进暴露功能

import { useState } from 'react';
import ConversationTab from './tabs/ConversationTab';
import NarrativeTab from './tabs/NarrativeTab';
import StoryboardTab from './tabs/StoryboardTab';
import CharacterTab from './tabs/CharacterTab';
import AdvancedTab from './tabs/AdvancedTab';

export type StudioTab = 'conversation' | 'narrative' | 'storyboard' | 'character' | 'advanced';

const TABS: Array<{ id: StudioTab; label: string; emoji: string; desc: string }> = [
  { id: 'conversation', label: '对话', emoji: '💬', desc: '和 Agent 聊天' },
  { id: 'narrative', label: '剧本', emoji: '📖', desc: 'LLM 充实的剧本' },
  { id: 'storyboard', label: '分镜', emoji: '🎬', desc: '镜头列表 + 镜头语言 + 声音设计' },
  { id: 'character', label: '角色', emoji: '🎭', desc: '主角 + 形态 + 微表情 + 配音' },
  { id: 'advanced', label: '高级', emoji: '⚙️', desc: 'JSON / 版本 / 导出' },
];

interface StudioCenterProps {
  initialTab?: StudioTab;
}

export default function StudioCenter({ initialTab = 'conversation' }: StudioCenterProps) {
  const [tab, setTab] = useState<StudioTab>(initialTab);

  return (
    <div className="flex h-full w-full flex-col bg-warm-50/40">
      {/* Tab bar */}
      <div className="flex items-center gap-1 border-b border-gray-100 bg-white/80 px-2 py-1.5 backdrop-blur">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTab(t.id)}
            title={t.desc}
            className={[
              'flex items-center gap-1 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors',
              tab === t.id
                ? 'bg-brand-100 text-brand-800'
                : 'text-gray-500 hover:bg-gray-100 hover:text-gray-700',
            ].join(' ')}
          >
            <span>{t.emoji}</span>
            <span>{t.label}</span>
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="min-h-0 flex-1">
        {tab === 'conversation' && <ConversationTab />}
        {tab === 'narrative' && <NarrativeTab />}
        {tab === 'storyboard' && <StoryboardTab />}
        {tab === 'character' && <CharacterTab />}
        {tab === 'advanced' && <AdvancedTab />}
      </div>
    </div>
  );
}