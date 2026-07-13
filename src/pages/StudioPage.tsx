import { useEffect } from 'react';
import StudioLayout from '../components/studio/StudioLayout';
import StudioCenter from '../components/studio/StudioCenter';
import ResultPane from '../components/studio/ResultPane';
import StoryWorkspace from '../components/studio/StoryWorkspace';
import PendingConfirmationBanner from '../components/studio/PendingConfirmationBanner';
import { useProjectStore } from '../stores/projectStore';
import { useStudioStore } from '../stores/studioStore';

export default function StudioPage() {
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
      <StudioLayout
        left={<StoryWorkspace />}
        center={<StudioCenter />}
        right={<ResultPane />}
      />
      <PendingConfirmationBanner />
    </>
  );
}