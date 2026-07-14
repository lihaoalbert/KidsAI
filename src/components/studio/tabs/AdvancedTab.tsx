// W9: 高级 tab — JSON 视图 + 多版本切换 + 导出 (B3 部分)

import { useState } from 'react';
import { useDirectorStore } from '../../../stores/directorStore';

export default function AdvancedTab() {
  const state = useDirectorStore((s) => ({
    cursor: s.cursor,
    idea: s.idea,
    story: s.story,
    character: s.character,
    characterMetas: s.characterMetas,
    style: s.style,
    shots: s.shots,
    finalVideoUrl: s.finalVideoUrl,
    locked_props: s.locked_props,
  }));
  const versions = useDirectorStore((s) => s.versions);
  const activeVersionId = useDirectorStore((s) => s.activeVersionId);
  const saveVersion = useDirectorStore((s) => s.saveVersion);
  const switchVersion = useDirectorStore((s) => s.switchVersion);
  const reFinalize = useDirectorStore((s) => s.reFinalize);

  const [copyState, setCopyState] = useState<'idle' | 'copied'>('idle');

  const exportJSON = () => {
    const json = JSON.stringify(state, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `story-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const exportScript = () => {
    const lines = [
      `# ${state.idea || '未命名故事'}`,
      '',
      `## 主角`,
      state.character ? `${state.character.name}: ${state.character.description}` : '（无）',
      '',
      `## 故事骨架`,
      `- 内核: ${state.story.spine.core || '—'}`,
      `- 冲突: ${state.story.spine.conflict || '—'}`,
      `- 世界观: ${state.story.spine.world || '—'}`,
      `- 调性: ${state.story.spine.tone}`,
      `- 主色: ${state.story.spine.theme_color || '—'}`,
      `- 寓意: ${state.story.spine.ending_moral || '—'}`,
      '',
      `## 剧本`,
      ...(state.story.narrative.paragraphs.length > 0
        ? state.story.narrative.paragraphs.map((p, i) => `### 第 ${i + 1} 段\n\n${p}`)
        : ['（无）']),
      '',
      `## 分镜 (${state.shots.length} 镜)`,
      ...state.shots.map((s, i) =>
        `### 第 ${i + 1} 镜 [${s.beat}/${s.mood}/${s.camera}]\n\n${s.description}\n\n动: ${s.motion}`
      ),
    ];
    const md = lines.join('\n\n');
    const blob = new Blob([md], { type: 'text/markdown' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `script-${Date.now()}.md`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-line bg-surface px-4 py-3">
        <h2 className="text-sm font-semibold text-ink-2">⚙️ 高级</h2>
      </div>

      <div className="flex-1 space-y-4 overflow-auto px-4 py-4">
        {/* 导出 */}
        <section className="rounded-xl border border-line bg-surface p-4 shadow-sm">
          <h3 className="mb-2 text-xs font-semibold text-ink-2">📤 导出</h3>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={exportJSON}
              className="rounded-md border border-line px-3 py-1.5 text-xs font-semibold text-ink-2 hover:bg-surface-2"
            >
              📄 JSON 原始数据
            </button>
            <button
              type="button"
              onClick={exportScript}
              className="rounded-md border border-line px-3 py-1.5 text-xs font-semibold text-ink-2 hover:bg-surface-2"
            >
              📖 剧本 Markdown
            </button>
            {state.finalVideoUrl && (
              <a
                href={state.finalVideoUrl}
                download={`final-${Date.now()}.mp4`}
                className="rounded-md border border-line px-3 py-1.5 text-xs font-semibold text-ink-2 hover:bg-surface-2"
              >
                🎞️ 下载成片视频
              </a>
            )}
            <button
              type="button"
              onClick={() => void reFinalize()}
              className="rounded-md bg-accent px-3 py-1.5 text-xs font-semibold text-bg hover:bg-accent-hover"
              disabled={state.shots.length === 0}
            >
              🔄 重新合成
            </button>
          </div>
        </section>

        {/* 多版本 */}
        <section className="rounded-xl border border-line bg-surface p-4 shadow-sm">
          <div className="mb-2 flex items-center justify-between">
            <h3 className="text-xs font-semibold text-ink-2">🗂️ 版本 ({Object.keys(versions).length})</h3>
            <button
              type="button"
              onClick={() => {
                const name = prompt('版本名', `版本 ${Object.keys(versions).length + 1}`);
                if (name !== null) saveVersion(name);
              }}
              className="rounded-md bg-accent px-2 py-1 text-xs font-semibold text-bg hover:bg-accent-hover"
            >
              ＋ 保存当前
            </button>
          </div>
          <ul className="space-y-1">
            {Object.values(versions).map((v) => (
              <li key={v.id}>
                <button
                  type="button"
                  onClick={() => switchVersion(v.id)}
                  className={[
                    'flex w-full items-center justify-between rounded-md px-3 py-2 text-left text-xs',
                    v.id === activeVersionId
                      ? 'bg-accent-soft-2 text-accent-ink'
                      : 'bg-surface-2 text-ink-2 hover:bg-surface-2',
                  ].join(' ')}
                >
                  <span>
                    <span className="font-semibold">{v.name}</span>
                    <span className="ml-2 text-[10px] text-ink-2">
                      {new Date(v.createdAt).toLocaleString('zh-CN')}
                    </span>
                  </span>
                  {v.id === activeVersionId && <span className="text-accent-ink">● 当前</span>}
                </button>
              </li>
            ))}
          </ul>
        </section>

        {/* JSON 视图 */}
        <section className="rounded-xl border border-line bg-surface p-4 shadow-sm">
          <div className="mb-2 flex items-center justify-between">
            <h3 className="text-xs font-semibold text-ink-2">🔍 原始数据 (只读)</h3>
            <button
              type="button"
              onClick={() => {
                navigator.clipboard.writeText(JSON.stringify(state, null, 2));
                setCopyState('copied');
                setTimeout(() => setCopyState('idle'), 1500);
              }}
              className="rounded-md border border-line px-2 py-1 text-xs text-ink-2 hover:bg-surface-2"
            >
              {copyState === 'copied' ? '✅ 已复制' : '📋 复制'}
            </button>
          </div>
          <pre className="max-h-96 overflow-auto rounded-md bg-code p-3 text-[10px] text-ink-3">
{JSON.stringify(state, null, 2)}
          </pre>
        </section>
      </div>
    </div>
  );
}