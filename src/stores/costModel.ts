// W9: costModel — 纯函数估算一次变更的代价 (学分 / 秒 / 下游影响)
// 不调任何 IO；UI 弹层前用此算账
//
// 依赖图 (谁改谁重):
//   who → character.{standard,threeView,forms} → shots[*].character_form + prompt
//       → shots[*].preview → final
//   spine.{core,conflict,world,tone,theme_color,audience,ending_moral}
//       → shots[*].prompt → shots[*].preview → final
//   narrative.paragraphs / scene → shots[*].prompt → shots[*].preview → final
//   character.form → shots[*] 仅该角色形态 (其他镜不动) → shots[*].preview → final
//   character.expression → shots[*].character_form (仅微表情) → shots[*].preview → final
//   shot.cinematography → shots[i].preview → final (仅该镜)
//   shot.sound → shots[i].preview → final (仅该镜的合成)
//   shot.prompt → shots[i].preview → final (仅该镜)
//   shot.reorder → shots[*].prompt (顺序影响衔接) → shots[*].preview → final
//   shot.insert / shot.delete → shots[*] 衔接 → shots[*].preview → final
//   finalize → final only

import type { DirectorShot, Story } from './directorStore';

export type ChangeKind =
  | 'who'
  | 'wants'
  | 'but'
  | 'ending'
  | 'spine.core'
  | 'spine.conflict'
  | 'spine.world'
  | 'spine.tone'
  | 'spine.theme_color'
  | 'spine.audience'
  | 'spine.ending_moral'
  | 'narrative.paragraph'
  | 'narrative.scene'
  | 'character'
  | 'character.form'
  | 'character.expression'
  | 'character.voice'
  | 'style'
  | 'shot.prompt'
  | 'shot.cinematography'
  | 'shot.sound'
  | 'shot.character_form'
  | 'shot.reorder'
  | 'shot.insert'
  | 'shot.delete'
  | 'finalize';

export interface CostEstimate {
  invalidates: string[];     // ['character.threeView', 'shots[*].prompt', 'shots[*].preview', 'final']
  credits: number;           // 学币
  seconds: number;           // 预估耗时
  requiresConfirm: boolean;  // > 5 学币必须确认
  rationale: string;         // 「改主角影响所有引用该角色的镜头 + 合成」
}

/** 一次变更的学分成本估算 */
const CREDIT_PER_SHOT_PREVIEW = 5;  // 单镜试拍
const CREDIT_PER_FINALIZE = 8;      // 整片合成
const CREDIT_PER_CHARACTER_GEN = 3; // 角色标准照/三视图/形态

/** 一次变更的耗时估算 (秒) */
const SEC_PER_SHOT_PREVIEW = 8;
const SEC_PER_FINALIZE = 25;
const SEC_PER_CHARACTER_GEN = 4;

const SECONDS_FOR_NARRATIVE_REGEN = 15; // 改 narrative 还要重新生成 plan
const CREDITS_FOR_NARRATIVE_REGEN = 2;  // AI plan 重生 (不消耗视频学分)

/** 触发 finalize 的所有上游改动 → 影响 final 视频
 *  注意: shot.* 类 (prompt/cinematography/sound/character_form) 影响该镜的合成,
 *  但只有全局性变更 (who/spine/narrative/style/reorder/insert/delete) 才影响 final 整片
 */
const FINAL_INVALIDATES: ChangeKind[] = [
  'who', 'wants', 'but', 'ending',
  'spine.core', 'spine.conflict', 'spine.world', 'spine.tone', 'spine.theme_color', 'spine.audience', 'spine.ending_moral',
  'narrative.paragraph', 'narrative.scene',
  'character', 'character.form', 'character.expression', 'character.voice',
  'style',
  'shot.reorder', 'shot.insert', 'shot.delete',
];

/** 触发 shots[*].prompt 重生成的改动 (除 shot.* 自身外，因为 shot.* 不需要重生 plan) */
const PROMPT_INVALIDATES: ChangeKind[] = [
  'who', 'wants', 'but', 'ending',
  'spine.core', 'spine.conflict', 'spine.world', 'spine.tone', 'spine.theme_color', 'spine.audience', 'spine.ending_moral',
  'narrative.paragraph', 'narrative.scene',
  'character', 'character.form', 'character.expression',
  'style',
  'shot.reorder', 'shot.insert', 'shot.delete',
];

/** 触发 character 资产重生成的改动 */
const CHARACTER_INVALIDATES: ChangeKind[] = [
  'who', 'character', 'character.form', 'character.expression',
];

export function estimateCost(
  kind: ChangeKind,
  ctx: { story: Story; shots: DirectorShot[]; shotIndex?: number },
): CostEstimate {
  const shotCount = ctx.shots.length;
  const idx = ctx.shotIndex ?? -1;
  const invalidates: string[] = [];

  // 1. character 资产
  if (CHARACTER_INVALIDATES.includes(kind)) {
    if (kind === 'who' || kind === 'character') {
      invalidates.push('character.standardImage', 'character.threeView', 'character.forms');
    } else {
      invalidates.push(`character.${kind.split('.')[1] ?? 'meta'}`);
    }
  }

  // 2. shots[*].prompt
  if (PROMPT_INVALIDATES.includes(kind)) {
    invalidates.push('shots[*].prompt');
  }

  // 3. shots[*].preview — 任何 prompt 重生成/角色/形态都触发重拍
  if (PROMPT_INVALIDATES.includes(kind)) {
    invalidates.push('shots[*].preview');
  } else if (
    kind === 'shot.prompt' || kind === 'shot.cinematography' ||
    kind === 'shot.sound' || kind === 'shot.character_form'
  ) {
    if (idx >= 0) invalidates.push(`shots[${idx}].preview`);
    else invalidates.push('shots[*].preview');
  }

  // 4. final — 只有全局变更 或 已有视频 才影响 final
  if (FINAL_INVALIDATES.includes(kind)) {
    // shot.insert 在 shots=0 时不触发 final (没东西可合)
    if (!(kind === 'shot.insert' && shotCount === 0)) {
      invalidates.push('final');
    }
  }
  if (kind === 'finalize') invalidates.push('final');

  // 5. 算学分 + 秒
  let credits = 0;
  let seconds = 0;

  if (kind === 'who' || kind === 'character') {
    credits += CREDIT_PER_CHARACTER_GEN;
    seconds += SEC_PER_CHARACTER_GEN;
  }
  if (kind === 'character.form' || kind === 'character.expression') {
    credits += 1;
    seconds += 2;
  }
  if (kind === 'character.voice') {
    credits += 2;
    seconds += 6;
  }
  // narrative 改动 → 重新生成 plan (LLM)
  if (kind === 'narrative.paragraph' || kind === 'narrative.scene') {
    credits += CREDITS_FOR_NARRATIVE_REGEN;
    seconds += SECONDS_FOR_NARRATIVE_REGEN;
  }
  // shot.* (单镜 edit) — 仅该镜重拍
  if (
    kind === 'shot.prompt' || kind === 'shot.cinematography' ||
    kind === 'shot.sound' || kind === 'shot.character_form'
  ) {
    credits += CREDIT_PER_SHOT_PREVIEW;
    seconds += SEC_PER_SHOT_PREVIEW;
  }
  // shot.insert — 新镜本身要拍; 已有镜衔接可能变 → 全部重拍
  if (kind === 'shot.insert') {
    credits += CREDIT_PER_SHOT_PREVIEW; // 新镜
    if (shotCount > 0) {
      credits += shotCount * CREDIT_PER_SHOT_PREVIEW; // 已有镜重拍
      seconds += shotCount * SEC_PER_SHOT_PREVIEW;
    }
    seconds += SEC_PER_SHOT_PREVIEW;
  }
  // shot.reorder / shot.delete — 全部重拍
  if (kind === 'shot.reorder') {
    credits += shotCount * CREDIT_PER_SHOT_PREVIEW;
    seconds += shotCount * SEC_PER_SHOT_PREVIEW;
  }
  if (kind === 'shot.delete') {
    credits += Math.max(0, shotCount - 1) * CREDIT_PER_SHOT_PREVIEW;
    seconds += Math.max(0, shotCount - 1) * SEC_PER_SHOT_PREVIEW;
  }
  // shots[*].prompt 重生成 + 试拍 (story 全局变更)
  if (PROMPT_INVALIDATES.includes(kind)) {
    credits += shotCount * CREDIT_PER_SHOT_PREVIEW;
    seconds += shotCount * SEC_PER_SHOT_PREVIEW;
  }
  // finalize = 仅合成
  if (kind === 'finalize') {
    credits += CREDIT_PER_FINALIZE;
    seconds += SEC_PER_FINALIZE;
  }
  // 其他全局改动 → 加 finalize 代价 (因为会影响 final 合成)
  else if (FINAL_INVALIDATES.includes(kind) && !(kind === 'shot.insert' && shotCount === 0)) {
    credits += CREDIT_PER_FINALIZE;
    seconds += SEC_PER_FINALIZE;
  }

  // 6. rationale — 给人看的解释
  const rationale = rationaleFor(kind, idx, shotCount);

  return {
    invalidates,
    credits,
    seconds,
    requiresConfirm: credits > 5,
    rationale,
  };
}

function rationaleFor(kind: ChangeKind, idx: number, shotCount: number): string {
  switch (kind) {
    case 'who':
    case 'character':
      return '改主角 → 重新生成标准照/三视图 + 所有分镜重拍 + 整片合成';
    case 'spine.core':
    case 'spine.conflict':
    case 'spine.world':
    case 'spine.tone':
    case 'spine.theme_color':
    case 'spine.audience':
    case 'spine.ending_moral':
      return '改故事骨架 → 所有分镜 prompt 重生成 + 重拍 + 合成';
    case 'narrative.paragraph':
    case 'narrative.scene':
      return '改剧本 → LLM 重生 plan + 重拍 + 合成';
    case 'character.form':
      return '改角色形态 → 该角色所在镜重拍 + 合成';
    case 'character.expression':
      return '改角色微表情 → 相关镜重拍 + 合成';
    case 'character.voice':
      return '改配音音色 → 仅配音重生成';
    case 'style':
      return '改画风 → 所有分镜重拍 + 合成';
    case 'shot.prompt':
      return `改分镜 ${idx + 1} 的提示词 → 该镜重拍 + 合成`;
    case 'shot.cinematography':
      return `改分镜 ${idx + 1} 的镜头语言 → 该镜重拍 + 合成`;
    case 'shot.sound':
      return `改分镜 ${idx + 1} 的声音设计 → 该镜重拍 + 合成`;
    case 'shot.character_form':
      return `改分镜 ${idx + 1} 的角色形态 → 该镜重拍 + 合成`;
    case 'shot.reorder':
      return '改分镜顺序 → 衔接改变 → 所有镜重拍 + 合成';
    case 'shot.insert':
    case 'shot.delete':
      return shotCount > 0 ? `新增/删除分镜 → 所有镜重拍 + 合成` : '新增分镜 → 该镜试拍';
    case 'finalize':
      return '整片合成';
    case 'wants':
    case 'but':
    case 'ending':
      return '改故事走向 → 所有分镜 prompt 重生成 + 重拍 + 合成';
    default:
      return '影响下游资产';
  }
}

/** 帮 UI 把学分 + 秒显示成一句话 */
export function formatCost(cost: CostEstimate): string {
  return `约 ${cost.credits} 学币 · ${cost.seconds} 秒`;
}

/** 帮 UI 把下游影响显示成中文短句列表 */
export function formatInvalidates(invalidates: string[]): string[] {
  return invalidates.map((i) => {
    if (i === 'final') return '🎞️ 整片视频';
    if (i === 'shots[*].prompt') return '📝 所有分镜提示词';
    if (i === 'shots[*].preview') return '🎬 所有分镜视频';
    if (i.startsWith('shots[')) return `🎬 分镜 ${parseInt(i.slice(6, -8), 10) + 1} 视频`;
    if (i === 'character.standardImage') return '🖼️ 主角标准照';
    if (i === 'character.threeView') return '🖼️ 主角三视图';
    if (i === 'character.forms') return '🖼️ 主角形态集';
    if (i.startsWith('character.')) return `🖼️ ${i}`;
    return i;
  });
}