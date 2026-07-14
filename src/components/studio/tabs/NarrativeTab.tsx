// W9: 剧本 tab — LLM 充实的 narrative（3-5 段）+ 用户可编辑

import { useState } from 'react';
import { useDirectorStore } from '../../../stores/directorStore';
import { runAgent } from '../../../api/tauri';

export default function NarrativeTab() {
  const story = useDirectorStore((s) => s.story);
  const setNarrative = useDirectorStore((s) => s.setNarrative);
  const chat = useDirectorStore((s) => s.chat);
  const [editingIdx, setEditingIdx] = useState<number | null>(null);
  const [draft, setDraft] = useState('');
  const [enriching, setEnriching] = useState(false);

  const paragraphs = story.narrative.paragraphs;

  const startEdit = (idx: number) => {
    setEditingIdx(idx);
    setDraft(paragraphs[idx] ?? '');
  };

  const saveEdit = () => {
    if (editingIdx === null) return;
    const next = [...paragraphs];
    next[editingIdx] = draft.trim();
    setNarrative(next.filter((p) => p.length > 0));
    setEditingIdx(null);
    setDraft('');
  };

  const deleteParagraph = (idx: number) => {
    const next = paragraphs.filter((_, i) => i !== idx);
    setNarrative(next);
  };

  const addParagraph = () => {
    const v = prompt('新段落内容');
    if (v && v.trim()) setNarrative([...paragraphs, v.trim()]);
  };

  const enrichWithLLM = async () => {
    const idea = useDirectorStore.getState().idea || story.who;
    if (!idea.trim() && !story.who) {
      alert('先告诉小启一些故事想法 (在「对话」tab)');
      return;
    }
    setEnriching(true);
    chat('🤔 我来把你的想法扩展成 3-5 段详细剧本...');
    try {
      const systemPrompt = `你是 KidsAI 的故事编剧助手。基于用户的故事梗概，写 3-5 段 (200-400 字 / 段) 适合 6-12 岁孩子的剧本。
- 段落之间用空行分隔
- 每段聚焦一段情节, 包含动作 / 情绪 / 关键对话 / 视觉重点
- 主角特点、画风、冲突已锁定, 必须遵循
- 输出纯中文剧本文本, 不要 JSON 包装, 不要 "以下是剧本" 之类前缀`;
      const ctx = `梗概: ${idea}
主角: ${story.who}
想要: ${story.wants}
但是: ${story.but}
结尾: ${story.ending}
故事内核: ${story.spine.core}
冲突: ${story.spine.conflict}
世界观: ${story.spine.world}`;
      const resp = await runAgent({
        levelId: 'narrative_enrich',
        userInput: ctx,
        systemPrompt,
        tools: [],
      });
      const text = resp.finalAnswer?.trim() ?? '';
      const newParas = text.split(/\n\s*\n/).map((p) => p.trim()).filter((p) => p.length > 30);
      if (newParas.length > 0) {
        setNarrative(newParas);
        chat(`✨ 写了 ${newParas.length} 段剧本, 你可以点任意段落改`);
      } else {
        chat('😅 没写出合适的内容, 你可以点 + 段落 自己加');
      }
    } catch (e) {
      chat(`😅 充实剧本走神了: ${e}`);
    } finally {
      setEnriching(false);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-line bg-surface px-4 py-3">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-ink-2">📖 剧本 ({paragraphs.length} 段)</h2>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={enrichWithLLM}
              disabled={enriching}
              className="rounded-md bg-accent px-3 py-1 text-xs font-semibold text-bg hover:bg-accent-hover disabled:opacity-50"
            >
              {enriching ? '🧠 充实中...' : '✨ 让 AI 充实'}
            </button>
            <button
              type="button"
              onClick={addParagraph}
              className="rounded-md border border-line px-3 py-1 text-xs font-semibold text-ink-2 hover:bg-surface-2"
            >
              ＋ 段落
            </button>
          </div>
        </div>
      </div>

      <div className="flex-1 space-y-3 overflow-auto px-4 py-4">
        {paragraphs.length === 0 && (
          <div className="rounded-xl border-2 border-dashed border-line bg-surface px-4 py-8 text-center text-sm text-ink-2">
            <p className="mb-2">还没有剧本内容</p>
            <p>点 ✨ 让 AI 充实, 或 ＋ 段落 自己写</p>
          </div>
        )}
        {paragraphs.map((p, idx) => (
          <div key={idx} className="rounded-xl border border-line bg-surface p-4 shadow-sm">
            {editingIdx === idx ? (
              <div>
                <textarea
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  className="h-32 w-full rounded-lg border border-line p-2 text-sm focus:border-accent focus:outline-none"
                />
                <div className="mt-2 flex justify-end gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      setEditingIdx(null);
                      setDraft('');
                    }}
                    className="rounded-md px-3 py-1 text-xs text-ink-2 hover:bg-surface-2"
                  >
                    取消
                  </button>
                  <button
                    type="button"
                    onClick={saveEdit}
                    className="rounded-md bg-accent px-3 py-1 text-xs font-semibold text-bg hover:bg-accent-hover"
                  >
                    保存
                  </button>
                </div>
              </div>
            ) : (
              <div>
                <div className="mb-2 flex items-start justify-between gap-2">
                  <span className="text-xs font-semibold text-ink-3">第 {idx + 1} 段</span>
                  <div className="flex gap-1">
                    <button
                      type="button"
                      onClick={() => startEdit(idx)}
                      className="rounded px-2 py-0.5 text-xs text-ink-2 hover:bg-surface-2"
                    >
                      ✏️
                    </button>
                    <button
                      type="button"
                      onClick={() => deleteParagraph(idx)}
                      className="rounded px-2 py-0.5 text-xs text-ink-2 hover:bg-surface-2"
                    >
                      🗑️
                    </button>
                  </div>
                </div>
                <p className="whitespace-pre-wrap text-sm leading-relaxed text-ink-2">{p}</p>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}