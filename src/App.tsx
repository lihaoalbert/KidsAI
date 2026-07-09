import { useState } from 'react';
import Sidebar from './components/Sidebar';
import HomePage from './pages/HomePage';
import WorkshopPage from './pages/WorkshopPage';
import LibraryPage from './pages/LibraryPage';
import MyAgentPage from './pages/MyAgentPage';
import LevelDetailPage from './pages/LevelDetailPage';

export type PageKey = 'home' | 'workshop' | 'library' | 'agent' | 'level';

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
    // Week 2.8 端到端 demo 阶段会接入关卡运行页（runner）
    // 本周只完成"数据模型 + 详情页"，先给一个友好提示
    alert(
      `🚧 关卡 ${levelId} 的运行页（runner）将在 Week 2.8 接入！\n\n` +
        `目前已就绪：\n` +
        `• 关卡数据模型（W2.1）\n` +
        `• 关卡详情页（W2.1）\n\n` +
        `接下来：\n` +
        `• Tauri 命令框架 + Zustand stores（W2.2）\n` +
        `• 本地 SQLite 进度缓存（W2.3）\n` +
        `• Agent Loop 核心（W2.4）\n` +
        `• 端到端 L1 demo（W2.8）`,
    );
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
