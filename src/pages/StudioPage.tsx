import { useEffect } from 'react';
import StudioLayout from '../components/studio/StudioLayout';
import ProjectsPane from '../components/studio/ProjectsPane';
import ConversationPane from '../components/studio/ConversationPane';
import ResultPane from '../components/studio/ResultPane';
import { useStudioStore } from '../stores/studioStore';

export default function StudioPage() {
  const started = useStudioStore((s) => s.started);
  const start = useStudioStore((s) => s.start);

  useEffect(() => {
    if (!started) start();
  }, [started, start]);

  return (
    <StudioLayout
      left={<ProjectsPane onNewVideo={start} />}
      center={<ConversationPane />}
      right={<ResultPane />}
    />
  );
}
