import { useState } from 'react';
import Sidebar from './components/Sidebar';
import HomePage from './pages/HomePage';
import WorkshopPage from './pages/WorkshopPage';
import LibraryPage from './pages/LibraryPage';
import MyAgentPage from './pages/MyAgentPage';

export type PageKey = 'home' | 'workshop' | 'library' | 'agent';

function App() {
  const [currentPage, setCurrentPage] = useState<PageKey>('home');

  const renderPage = () => {
    switch (currentPage) {
      case 'home':
        return <HomePage />;
      case 'workshop':
        return <WorkshopPage />;
      case 'library':
        return <LibraryPage />;
      case 'agent':
        return <MyAgentPage />;
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
