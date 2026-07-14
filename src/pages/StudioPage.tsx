import { useEffect } from 'react';
import StudioLayout from '../components/studio/StudioLayout';
import StudioCenter from '../components/studio/StudioCenter';
import ResultPane from '../components/studio/ResultPane';
import StoryWorkspace from '../components/studio/StoryWorkspace';
import ProjectsPane from '../components/studio/ProjectsPane';
import PendingConfirmationBanner from '../components/studio/PendingConfirmationBanner';
import { useProjectStore } from '../stores/projectStore';
import { useStudioStore } from '../stores/studioStore';

interface StudioPageProps {
  onBackHome?: () => void;
}

export default function StudioPage({ onBackHome }: StudioPageProps) {
  const started = useStudioStore((s) => s.started);
  const start = useStudioStore((s) => s.start);

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
    <>
      {/* P0 fix: ProjectsPane 移到 studio 页面内部, Sidebar 永远显示完整导航 */}
      <div className="flex h-full w-full">
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
    </>
  );
}