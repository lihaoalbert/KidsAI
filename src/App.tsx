import { useEffect, useState } from 'react';
import Sidebar from './components/Sidebar';
import HomePage from './pages/HomePage';
import WorkshopPage from './pages/WorkshopPage';
import LibraryPage from './pages/LibraryPage';
import MyAgentPage from './pages/MyAgentPage';
import LevelDetailPage from './pages/LevelDetailPage';
import AgentRunnerPage from './pages/AgentRunnerPage';
import StudioPage from './pages/StudioPage';
import OnboardingPage from './pages/OnboardingPage';
import { checkAlreadyActivated } from './pages/OnboardingPage';
import type { ActivateResponse } from './api/tauri';

export type PageKey =
  | 'home'
  | 'workshop'
  | 'library'
  | 'agent'
  | 'level'
  | 'runner'
  | 'studio';

function App() {
  const [activated, setActivated] = useState<boolean | null>(null); // null = 探测中
  const [currentPage, setCurrentPage] = useState<PageKey>('home');
  const [selectedLevelId, setSelectedLevelId] = useState<string>('L1');

  useEffect(() => {
    let cancelled = false;
    checkAlreadyActivated().then((ok) => {
      if (!cancelled) setActivated(ok);
    });
    return () => {
      cancelled = true;
    };
  }, []);

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
        return <StudioPage />;
      case 'agent':
        return <MyAgentPage />;
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
    </div>
  );
}

export default App;