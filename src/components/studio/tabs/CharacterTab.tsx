// W9: 角色 tab — 主角 + 形态 + 微表情 + 配音

import { useDirectorStore } from '../../../stores/directorStore';
import type { CharacterForm, CharacterExpression } from '../../../stores/directorStore';

export default function CharacterTab() {
  const character = useDirectorStore((s) => s.character);
  const characterMetas = useDirectorStore((s) => s.characterMetas);
  const addCharacterForm = useDirectorStore((s) => s.addCharacterForm);
  const addCharacterExpression = useDirectorStore((s) => s.addCharacterExpression);
  const setVoiceId = useDirectorStore((s) => s.setVoiceId);

  if (!character) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-center text-sm text-gray-500">
        还没选主角 — 先在「对话」tab 里和 Agent 聊聊
      </div>
    );
  }

  const meta = characterMetas[character.id] ?? { forms: [], expressions: [], voiceId: undefined };

  const handleAddForm = () => {
    const name = prompt('形态名 (默认 / 战斗 / 受伤 / 胜利 …)', '');
    if (!name) return;
    const promptText = prompt('该形态在视频生成中的 prompt 修饰 (例: 身上冒出火焰、双眼发光)', '') ?? '';
    const form: CharacterForm = {
      id: name.toLowerCase().replace(/\s+/g, '-'),
      name,
      prompt: promptText,
    };
    addCharacterForm(character.id, form);
  };

  const handleAddExpression = () => {
    const name = prompt('微表情名 (开心 / 愤怒 / 疑惑 …)', '');
    if (!name) return;
    const expr: CharacterExpression = {
      id: name.toLowerCase().replace(/\s+/g, '-'),
      name,
    };
    addCharacterExpression(character.id, expr);
  };

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-gray-100 bg-white px-4 py-3">
        <h2 className="text-sm font-semibold text-gray-700">🎭 角色 ({character.name})</h2>
      </div>

      <div className="flex-1 space-y-4 overflow-auto px-4 py-4">
        {/* 标准照 */}
        <section className="rounded-xl border border-gray-100 bg-white p-4 shadow-sm">
          <h3 className="mb-2 text-xs font-semibold text-gray-600">🖼️ 标准照</h3>
          <div className="flex h-32 w-32 items-center justify-center rounded-lg bg-gray-100">
            {character.standardImageUrl ? (
              <img src={character.standardImageUrl} alt={character.name} className="h-full w-full rounded-lg object-cover" />
            ) : (
              <span className="text-xs text-gray-400">无图</span>
            )}
          </div>
        </section>

        {/* 形态 */}
        <section className="rounded-xl border border-gray-100 bg-white p-4 shadow-sm">
          <div className="mb-2 flex items-center justify-between">
            <h3 className="text-xs font-semibold text-gray-600">🧬 形态 ({meta.forms.length})</h3>
            <button
              type="button"
              onClick={handleAddForm}
              className="rounded-md bg-brand-600 px-2 py-1 text-xs font-semibold text-white hover:bg-brand-700"
            >
              ＋ 加形态
            </button>
          </div>
          {meta.forms.length === 0 ? (
            <p className="text-xs text-gray-400">（默认形态 — 没有额外形态）</p>
          ) : (
            <ul className="grid grid-cols-2 gap-2">
              {meta.forms.map((f) => (
                <li key={f.id} className="rounded-lg border border-gray-200 bg-gray-50 p-2 text-xs">
                  <div className="font-semibold text-gray-700">{f.name}</div>
                  <div className="mt-0.5 line-clamp-2 text-gray-500">{f.prompt}</div>
                </li>
              ))}
            </ul>
          )}
        </section>

        {/* 微表情 */}
        <section className="rounded-xl border border-gray-100 bg-white p-4 shadow-sm">
          <div className="mb-2 flex items-center justify-between">
            <h3 className="text-xs font-semibold text-gray-600">😀 微表情 ({meta.expressions.length})</h3>
            <button
              type="button"
              onClick={handleAddExpression}
              className="rounded-md bg-brand-600 px-2 py-1 text-xs font-semibold text-white hover:bg-brand-700"
            >
              ＋ 加微表情
            </button>
          </div>
          {meta.expressions.length === 0 ? (
            <p className="text-xs text-gray-400">（无微表情）</p>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {meta.expressions.map((e) => (
                <span key={e.id} className="rounded-full bg-gray-100 px-2 py-0.5 text-xs text-gray-700">
                  {e.name}
                </span>
              ))}
            </div>
          )}
        </section>

        {/* 配音 */}
        <section className="rounded-xl border border-gray-100 bg-white p-4 shadow-sm">
          <h3 className="mb-2 text-xs font-semibold text-gray-600">🎤 配音</h3>
          <div className="flex items-center gap-2">
            <input
              type="text"
              defaultValue={meta.voiceId ?? ''}
              onBlur={(e) => setVoiceId(e.target.value || null)}
              placeholder="voice_id (voice_clone 后填)"
              className="flex-1 rounded-md border border-gray-200 px-2 py-1 text-xs focus:border-brand-400 focus:outline-none"
            />
          </div>
          <p className="mt-1 text-[10px] text-gray-400">使用 VoiceClonePicker 录音后会自动填入</p>
        </section>
      </div>
    </div>
  );
}