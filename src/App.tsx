// SCREENSHOT-DEV: 仅 import.meta.env.DEV 时启用, 生产构建自动 dead-code-eliminate
// ?mockTauri=1 注入 Tauri IPC 假数据 (vite dev 截图专用)
// ?bypassOnboarding=1 在 useEffect 里直跳 HomePage
if (import.meta.env.DEV && typeof window !== 'undefined') {
  const params = new URLSearchParams(window.location.search);
  if (params.get('mockTauri') === '1') {
    const make = (id: string, title: string, description: string, emoji: string, difficulty: 1 | 2 | 3, prereqs: string[] = []) => ({
      id, orderNum: parseInt(id.slice(1)), title, description, coverEmoji: emoji,
      estimatedMinutes: 15, rewardTokens: 10, difficulty, prerequisites: prereqs,
      steps: [], aiName: 'AI', aiAvatar: '🤖', systemPrompt: '', tools: [],
      scoringCriteria: { creativity: 25, completion: 25, expression: 25, interaction: 25, depth: 0 },
    });
    const LEVEL_STUBS = [
      make('L1', '我的 AI 伙伴', '认识你的 AI 伙伴，聊一聊它能帮你做什么。', '🤖', 1),
      make('L2', '想一个故事', '用 AI 帮你写一段有趣的小故事。', '📖', 1, ['L1']),
      make('L3', '画一只小猫', '用提示词生成一张专属小猫插画。', '🎨', 2, ['L2']),
      make('L4', '小猫动起来', '把静态图片变成 5 秒动画。', '🎬', 2, ['L3']),
      make('L5', '故事配图', '用 AI 给故事自动配上 3 张插图。', '📚', 3, ['L4']),
      make('L6', '做一段预告片', '把视频片段 + 文字组合成预告片。', '🎞️', 3, ['L5']),
      make('L7', '发布作品集', '整理你的作品，准备分享给朋友。', '🏆', 2, ['L6']),
    ];
    // @ts-expect-error 仅截图 dev 用
    window.__TAURI_INTERNALS__ = {
      invoke: (cmd: string, args?: Record<string, unknown>) => {
        const stubs: Record<string, unknown> = {
          get_license_info: { deviceId: 'dev-screenshot-1', nickname: '小明', ageTier: 1, isDemo: false, llmApiKey: 'sk-stub', videoApiKey: 'sk-stub', lastBalance: 100 },
          get_balance: { deviceId: 'dev-screenshot-1', balance: 100, dailyConsumed: 11, dailyQuota: 30, dailyRemaining: 19 },
          list_levels: LEVEL_STUBS,
          list_progress: [],
          completed_level_ids: [],
          list_creations: [],
          list_projects: [],
          // W8: 项目 CRUD 在 vite 截图模式返回空骨架, 避免 useProjectStore 触发 null.id
          create_project: {
            id: 'p-screenshot-1',
            title: (args?.title as string) ?? '我的小电影',
            levelId: null,
            cursor: 0,
            thumbPath: null,
            totalCredits: 0,
            createdAt: Date.now(),
            updatedAt: Date.now(),
          },
          load_project: {
            meta: {
              id: 'p-screenshot-1',
              title: '我的小电影',
              levelId: null,
              cursor: 0,
              thumbPath: null,
              totalCredits: 0,
              createdAt: Date.now(),
              updatedAt: Date.now(),
            },
            plan: {},
            transcript: { items: [], started: false },
          },
          save_project_state: {
            id: 'p-screenshot-1',
            title: '我的小电影',
            levelId: null,
            cursor: 0,
            thumbPath: null,
            totalCredits: 0,
            createdAt: Date.now(),
            updatedAt: Date.now(),
          },
        };
        return Promise.resolve(stubs[cmd] ?? null);
      },
    };
  }
}

import { useEffect, useState } from 'react';
import Sidebar from './components/Sidebar';
import HomePage from './pages/HomePage';
import WorkshopPage from './pages/WorkshopPage';
import LibraryPage from './pages/LibraryPage';
import MyAgentPage from './pages/MyAgentPage';
import LevelDetailPage from './pages/LevelDetailPage';
import AgentRunnerPage from './pages/AgentRunnerPage';
import StudioPage from './pages/StudioPage';
import MarketplacePage from './pages/MarketplacePage';
import SettingsPage from './pages/SettingsPage';
import OnboardingPage from './pages/OnboardingPage';
import PetCorner from './components/pet/PetCorner';
import { checkAlreadyActivated } from './pages/OnboardingPage';
import type { ActivateResponse } from './api/tauri';
import { useAssetStore } from './stores/assetStore';
import { initializeProjectPersistence, useProjectStore } from './stores/projectStore';

export type PageKey =
  | 'home'
  | 'workshop'
  | 'library'
  | 'agent'
  | 'level'
  | 'runner'
  | 'studio'
  | 'marketplace'
  | 'settings';

function App() {
  const [activated, setActivated] = useState<boolean | null>(null); // null = 探测中
  const [currentPage, setCurrentPage] = useState<PageKey>('home');
  const [selectedLevelId, setSelectedLevelId] = useState<string>('L1');

  useEffect(() => {
    let cancelled = false;
    // SCREENSHOT-DEV: 仅 vite dev 启用, 生产构建 dead-code-eliminate
    if (import.meta.env.DEV && new URLSearchParams(window.location.search).get('bypassOnboarding') === '1') {
      setActivated(true);
      return;
    }
    checkAlreadyActivated().then((ok) => {
      if (!cancelled) setActivated(ok);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // W6 B3: 启动时拉 asset manifest (如果 license info 有 server url, 跟着走).
  // fallback: 默认 https://api.kids.ibi.ren (production ECS).
  useEffect(() => {
    if (!('__TAURI_INTERNALS__' in window)) return;
    const serverUrl =
      import.meta.env.VITE_KIDSAI_SERVER_URL ?? 'https://api.kids.ibi.ren';
    void useAssetStore.getState().fetch(serverUrl);
  }, []);

  useEffect(() => {
    if (activated !== true || !('__TAURI_INTERNALS__' in window)) return;
    let cancelled = false;
    const disposePersistence = initializeProjectPersistence();
    void useProjectStore.getState().refresh().then(async () => {
      if (cancelled) return;
      const recent = useProjectStore
        .getState()
        .list.find((project) => project.cursor < 6);
      if (!recent) return;
      try {
        await useProjectStore.getState().open(recent.id);
        if (!cancelled) setCurrentPage('studio');
      } catch {
        return;
      }
    });
    return () => {
      cancelled = true;
      disposePersistence();
    };
  }, [activated]);

  const handleActivated = (_resp: ActivateResponse) => {
    setActivated(true);
    setCurrentPage('home');
  };

  if (activated === null) {
    // 探测中, 简单 loader (生产可以加 spinner)
    return (
      <div className="flex h-full items-center justify-center bg-warm-50 text-gray-500">
        加载中...
      </div>
    );
  }

  if (!activated) {
    return <OnboardingPage onActivated={handleActivated} />;
  }

  const handleOpenLevel = (levelId: string) => {
    setSelectedLevelId(levelId);
    setCurrentPage('level');
  };

  const handleBackToHome = () => {
    setCurrentPage('home');
  };

  const handleStart = (levelId: string) => {
    setSelectedLevelId(levelId);
    setCurrentPage('runner');
  };

  const renderPage = () => {
    switch (currentPage) {
      case 'home':
        return (
          <HomePage
            onOpenLevel={handleOpenLevel}
            onOpenStudio={() => setCurrentPage('studio')}
          />
        );
      case 'workshop':
        return <WorkshopPage onOpenStudio={() => setCurrentPage('studio')} />;
      case 'library':
        return <LibraryPage />;
      case 'studio':
        return <StudioPage onBackHome={handleBackToHome} />;
      case 'agent':
        return <MyAgentPage />;
      case 'marketplace':
        return <MarketplacePage />;
      case 'settings':
        return <SettingsPage />;
      case 'level':
        return (
          <LevelDetailPage
            levelId={selectedLevelId}
            onBack={handleBackToHome}
            onStart={handleStart}
          />
        );
      case 'runner':
        return (
          <AgentRunnerPage
            levelId={selectedLevelId}
            onBack={handleBackToHome}
          />
        );
    }
  };

  return (
    <div className="flex h-full bg-warm-50">
      <Sidebar currentPage={currentPage} onNavigate={setCurrentPage} />
      <main className="flex-1 overflow-auto">{renderPage()}</main>
      <PetCorner onNavigate={(p) => setCurrentPage(p === 'home' ? 'home' : 'library')} />
    </div>
  );
}

export default App;