import { useEffect } from 'react';
import StudioLayout from '../components/studio/StudioLayout';
import ConversationPane from '../components/studio/ConversationPane';
import ResultPane from '../components/studio/ResultPane';
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

  return <StudioLayout center={<ConversationPane />} right={<ResultPane />} />;
}