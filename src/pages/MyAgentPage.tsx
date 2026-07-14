// P0 fix: MyAgentPage 从 placeholder 变成真实可用的"高级功能聚合页"
// 提供 14-16 岁专属功能入口 + 高级 skill 入口 (为 M3 自习室 / 项目管理铺路)

import { useNavigate } from 'react-router-dom';
import Card from '../components/Card';

interface ShortcutCard {
  emoji: string;
  title: string;
  desc: string;
  badge?: string;
  comingSoon?: boolean;
  to?: string;
}

const shortcuts: ShortcutCard[] = [
  {
    emoji: '🛋️',
    title: '自习室模式',
    desc: '写作业时让 AI 安静陪着, 计时 + 答疑, 不打扰你',
    badge: '即将上线',
    comingSoon: true,
  },
  {
    emoji: '📂',
    title: '项目管理',
    desc: '把多个作品归到同一个频道, 像 B 站 UP 主一样连载',
    badge: '即将上线',
    comingSoon: true,
  },
  {
    emoji: '🔍',
    title: '知识检索',
    desc: '让 agent 帮你查物理公式 / 历史事件 / 英文单词',
    badge: '即将上线',
    comingSoon: true,
  },
  {
    emoji: '🎬',
    title: '高级导演',
    desc: '跳过引导, 直接给 AI 提要求, 做更复杂的视频',
    to: 'studio',
  },
  {
    emoji: '📦',
    title: 'Skill 市场',
    desc: '官方 + 第三方 skill, 解锁更多创作能力',
    to: 'marketplace',
  },
  {
    emoji: '⚙️',
    title: '家长设置',
    desc: '学币详情, 模式切换, PIN 管理',
    to: 'settings',
  },
];

export default function MyAgentPage() {
  const navigate = useNavigate();

  return (
    <div className="p-8 max-w-6xl mx-auto">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-gray-900">🤖 我的 Agent</h1>
        <p className="text-base text-gray-600 mt-1">
          14+ 专属功能：自习、项目、检索、创作增强
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {shortcuts.map((s) => (
          <Card
            key={s.title}
            variant={s.comingSoon ? 'default' : 'bordered'}
            className={
              s.comingSoon
                ? 'opacity-60'
                : 'cursor-pointer hover:shadow-lg transition-shadow'
            }
            onClick={() => {
              if (s.to === 'studio') navigate('/studio');
              else if (s.to === 'marketplace') navigate('/marketplace');
              else if (s.to === 'settings') navigate('/settings');
            }}
          >
            <div className="flex items-start gap-3">
              <div className="text-3xl">{s.emoji}</div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <div className="font-semibold text-gray-900">{s.title}</div>
                  {s.badge && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-100 text-amber-700">
                      {s.badge}
                    </span>
                  )}
                </div>
                <div className="text-xs text-gray-500 leading-relaxed">
                  {s.desc}
                </div>
              </div>
            </div>
          </Card>
        ))}
      </div>

      <div className="mt-8 text-xs text-gray-400 text-center">
        💡 部分功能还在打磨中, 期待你的反馈
      </div>
    </div>
  );
}
