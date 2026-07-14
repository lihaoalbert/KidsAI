// Home — 阶段3 重建 bento 版
//
// DESIGN.md §5.1 Home / 课程中心:
//   Forest: 8-cell bento grid, 大 hero + 关卡网格 + 学币/今日/项目 三联
//   Coast: 12-cell dense, instrument-first
//   Hero card (主): 主推关卡 + 进度 + 主 CTA "继续创作"
//   Today/Project/Balance 三联: 信息密度高

import { useEffect, useMemo } from 'react';
import Card from '../components/Card';
import BalanceWidget from '../components/BalanceWidget';
import FeedbackButton from '../components/FeedbackButton';
import AppHeader from '../components/layout/AppHeader';
import { useLevelStore } from '../stores/levelStore';
import { useUserModeStore } from '../stores/userModeStore';
import { useProjectStore } from '../stores/projectStore';

interface HomePageProps {
  onOpenLevel?: (levelId: string) => void;
  onOpenStudio?: () => void;
  onOpenLibrary?: () => void;
}

export default function HomePage({
  onOpenLevel,
  onOpenStudio,
  onOpenLibrary,
}: HomePageProps) {
  const { levels, isUnlocked, refresh, isLoading, error } = useLevelStore();
  const projects = useProjectStore((s) => s.list);
  const mode = useUserModeStore((s) => s.mode);
  const isAdult = mode === 'adult';

  useEffect(() => {
    refresh();
  }, [refresh]);

  const stats = useMemo(() => {
    const total = levels.length;
    const unlocked = levels.filter((l) => isUnlocked(l.id)).length;
    const nextLevel = levels.find((l) => isUnlocked(l.id));
    return { total, unlocked, nextLevel };
  }, [levels, isUnlocked]);

  // 顶部右侧 actions
  const headerActions = (
    <>
      <FeedbackButton />
    </>
  );

  // Hero 主卡 (Studio CTA)
  const heroCard = (
    <Card
      variant="filled"
      className={
        onOpenStudio
          ? 'cursor-pointer hover:shadow-lg transition-shadow'
          : ''
      }
      onClick={onOpenStudio}
    >
      <div className="p-6 flex flex-col h-full min-h-[200px]">
        <div className="text-5xl mb-3">{isAdult ? '🎬' : '🎬'}</div>
        <div className="text-xl font-bold text-ink mb-1">
          {isAdult ? 'Launch Studio' : '开始你的故事'}
        </div>
        <div className="text-sm text-ink-2 mb-4 flex-1">
          {isAdult
            ? '6-step pipeline from brief to final cut. Pro tools, calm palette.'
            : '跟 🦉 一起，分 6 步把脑子里的画面变成真正的视频。'}
        </div>
        <div className="inline-flex items-center gap-1.5 text-accent-ink text-sm font-semibold">
          {isAdult ? 'Open Studio →' : '开始创作 →'}
        </div>
      </div>
    </Card>
  );

  // 学币 / 今日 / 项目 三联 — 父 bento 列 (右 1/3) 内的子网格
  const statsCards = (
    <div className="grid grid-cols-1 gap-3 h-full">
      <Card variant="default">
        <div className="p-4">
          <div className="text-2xs uppercase tracking-wider text-ink-3 mb-1">
            {isAdult ? 'Credits' : '学币'}
          </div>
          <BalanceWidget compact />
        </div>
      </Card>
      <Card variant="default">
        <div className="p-4">
          <div className="text-2xs uppercase tracking-wider text-ink-3 mb-1">
            {isAdult ? 'Progress' : '今日进度'}
          </div>
          <div className="text-2xl font-bold text-ink">
            {stats.unlocked}
            <span className="text-ink-3 text-sm font-normal"> / {stats.total}</span>
          </div>
          <div className="text-xs text-ink-2 mt-1">
            {isAdult ? 'Levels unlocked' : '已解锁关卡'}
          </div>
        </div>
      </Card>
      <Card
        variant="default"
        className={projects.length > 0 && onOpenLibrary ? 'cursor-pointer' : ''}
        onClick={() => projects.length > 0 && onOpenLibrary?.()}
      >
        <div className="p-4">
          <div className="text-2xs uppercase tracking-wider text-ink-3 mb-1">
            {isAdult ? 'Projects' : '我的项目'}
          </div>
          <div className="text-2xl font-bold text-ink">{projects.length}</div>
          <div className="text-xs text-ink-2 mt-1">
            {projects.length > 0
              ? isAdult
                ? 'Open Library →'
                : '查看作品库 →'
              : isAdult
                ? 'None yet'
                : '还没有，去创作吧 →'}
          </div>
        </div>
      </Card>
    </div>
  );

  return (
    <div className="flex flex-col h-full bg-bg">
      <AppHeader
        title={isAdult ? 'Home' : '课程中心'}
        actions={headerActions}
      />

      <div className="flex-1 overflow-y-auto p-8 max-w-6xl mx-auto w-full">
        {/* 欢迎区 */}
        <div className="mb-6">
          <h1 className="text-2xl font-bold text-ink">
            {isAdult ? '👋 Welcome back' : '👋 你好，小创作者！'}
          </h1>
          <p className="text-base text-ink-2 mt-1">
            {isAdult
              ? 'Pick up where you left off, or start a new project.'
              : '欢迎来到 AI 创作世界。今天想做点什么？'}
          </p>
        </div>

        {/* Bento: Hero + 三联 stats */}
        <div
          className={
            isAdult
              ? 'grid grid-cols-1 lg:grid-cols-3 gap-3 mb-8'
              : 'grid grid-cols-1 lg:grid-cols-3 gap-4 mb-8'
          }
        >
          <div className={isAdult ? 'lg:col-span-1' : 'lg:col-span-2'}>
            {heroCard}
          </div>
          <div className={isAdult ? 'lg:col-span-2' : 'lg:col-span-1'}>
            {statsCards}
          </div>
        </div>

        {/* 推荐关卡 */}
        <div className="mb-6">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-lg font-semibold text-ink">
              {isAdult
                ? `Recommended levels (${stats.unlocked} / ${stats.total})`
                : `推荐关卡（已解锁 ${stats.unlocked} / 共 ${stats.total}）`}
            </h2>
            {isLoading && (
              <span className="text-xs text-ink-2">加载中…</span>
            )}
          </div>

          {error && (
            <div className="mb-3 p-3 rounded-md bg-danger-soft border border-danger-soft text-sm text-danger">
              ⚠️ 加载关卡失败：{error}
            </div>
          )}

          <div className="grid grid-cols-3 gap-4">
            {levels.map((level) => {
              const available = isUnlocked(level.id);
              return (
                <Card
                  key={level.id}
                  variant={available ? 'bordered' : 'default'}
                  className={
                    onOpenLevel && available
                      ? 'cursor-pointer hover:shadow-lg transition-shadow'
                      : ''
                  }
                  onClick={() => {
                    if (onOpenLevel && available) onOpenLevel(level.id);
                  }}
                  footer={
                    <div className="flex items-center justify-between text-xs text-ink-2">
                      <span>
                        💎 {level.rewardTokens}{' '}
                        {isAdult ? 'credits' : '学币'}
                      </span>
                      <span>⏱ {level.estimatedMinutes} 分钟</span>
                    </div>
                  }
                >
                  <div
                    className={`aspect-video rounded-md flex items-center justify-center text-5xl mb-2 ${
                      available
                        ? 'bg-gradient-to-br from-accent-soft to-highlight/30'
                        : 'bg-surface-2 grayscale'
                    }`}
                  >
                    {available ? level.coverEmoji : '🔒'}
                  </div>
                  <div className="text-xs font-mono text-accent-ink mb-1">
                    {level.id} · 难度{' '}
                    {'★'.repeat(level.difficulty)}
                  </div>
                  <div className="text-sm font-semibold text-ink line-clamp-1">
                    {level.title}
                  </div>
                  <div className="text-xs text-ink-2 line-clamp-2 mt-1">
                    {level.description}
                  </div>
                </Card>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}