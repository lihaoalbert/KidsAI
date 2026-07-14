import { useEffect } from 'react';
import StudioLayout from '../components/studio/StudioLayout';
import StudioCenter from '../components/studio/StudioCenter';
import ResultPane from '../components/studio/ResultPane';
import StoryWorkspace from '../components/studio/StoryWorkspace';
import ProjectsPane from '../components/studio/ProjectsPane';
import PendingConfirmationBanner from '../components/studio/PendingConfirmationBanner';
import AppHeader from '../components/layout/AppHeader';
import { useProjectStore } from '../stores/projectStore';
import { useStudioStore } from '../stores/studioStore';
import { useUserModeStore } from '../stores/userModeStore';

interface StudioPageProps {
  onBackHome?: () => void;
}

export default function StudioPage({ onBackHome }: StudioPageProps) {
  const started = useStudioStore((s) => s.started);
  const start = useStudioStore((s) => s.start);
  const mode = useUserModeStore((s) => s.mode);
  const isAdult = mode === 'adult';

  useEffect(() => {
    if (started) return;
    let cancelled = false;
    const begin = async () => {
      if ('__TAURI_INTERNALS__' in window) {
        try {
          await useProjectStore.getState().ensureCurrent();
        } catch {
          // Start remains available even if local persistence is temporarily unavailable.
        }
      }
      if (!cancelled) start();
    };
    void begin();
    return () => {
      cancelled = true;
    };
  }, [started, start]);

  return (
    <div className="flex flex-col h-full bg-bg">
      <AppHeader
        title={isAdult ? 'Studio' : '视频创作'}
        breadcrumb={[isAdult ? 'Home' : '课程中心', isAdult ? 'Studio' : '视频创作']}
        actions={
          <button
            type="button"
            onClick={() => onBackHome?.()}
            className="text-meta text-ink-2 hover:text-ink transition-colors"
          >
            {isAdult ? '← Home' : '← 返回首页'}
          </button>
        }
      />
      {/* P0 fix: ProjectsPane 移到 studio 页面内部, Sidebar 永远显示完整导航 */}
      <div className="flex flex-1 min-h-0 w-full">
        <aside className="w-56 shrink-0 border-r border-line bg-surface">
          <ProjectsPane onBackHome={() => onBackHome?.()} />
        </aside>
        <div className="flex-1 min-w-0">
          <StudioLayout
            left={<StoryWorkspace />}
            center={<StudioCenter />}
            right={<ResultPane />}
          />
        </div>
      </div>
      <PendingConfirmationBanner />
    </div>
  );
}