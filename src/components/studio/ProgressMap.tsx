import { useDirectorStore } from '../../stores/directorStore';

const STAGES = ['点子', '主角', '画风', '分镜', '试拍', '定稿'] as const;
const EMOJI = ['💡', '🌟', '🎨', '🎞️', '🎬', '🏆'] as const;

export default function ProgressMap() {
  const stage = useDirectorStore((s) => s.stage);
  return (
    <div className="flex items-center gap-1 px-3 py-2">
      {STAGES.map((label, i) => {
        const step = i + 1;
        const active = step === stage;
        const done = step < stage;
        return (
          <div key={label} className="flex items-center gap-1">
            <div
              className={[
                'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-semibold transition-colors',
                active
                  ? 'bg-brand-500 text-white shadow-sm'
                  : done
                    ? 'bg-green-100 text-green-700'
                    : 'bg-gray-100 text-gray-400',
              ].join(' ')}
            >
              <span>{done ? '✅' : EMOJI[i]}</span>
              <span className="hidden sm:inline">{label}</span>
            </div>
            {i < STAGES.length - 1 && (
              <span className={done ? 'text-green-300' : 'text-gray-200'}>–</span>
            )}
          </div>
        );
      })}
    </div>
  );
}
