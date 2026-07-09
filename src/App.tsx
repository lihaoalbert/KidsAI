import { useState } from 'react';
import Sidebar from './components/Sidebar';
import HomePage from './pages/HomePage';
import WorkshopPage from './pages/WorkshopPage';
import LibraryPage from './pages/LibraryPage';
import MyAgentPage from './pages/MyAgentPage';
import LevelDetailPage from './pages/LevelDetailPage';
import AgentRunnerPage from './pages/AgentRunnerPage';

export type PageKey = 'home' | 'workshop' | 'library' | 'agent' | 'level' | 'runner';

function App() {
  const [currentPage, setCurrentPage] = useState<PageKey>('home');
  const [selectedLevelId, setSelectedLevelId] = useState<string>('L1');

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
        return <HomePage onOpenLevel={handleOpenLevel} />;
      case 'workshop':
        return <WorkshopPage />;
      case 'library':
        return <LibraryPage />;
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
