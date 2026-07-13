// W9: costModel 单测 — 12 个 case 覆盖每种 ChangeKind 的下游链路

import { describe, expect, it } from 'vitest';
import { estimateCost, formatCost, formatInvalidates, type ChangeKind } from './costModel';
import type { DirectorShot, Story } from './directorStore';

const story: Story = {
  who: '小猫',
  wants: '追蝴蝶',
  but: '迷路了',
  ending: '找到了',
  spine: { core: '', conflict: '', world: '', tone: 'playful', audience: '', theme_color: '', ending_moral: '' },
  narrative: { paragraphs: [], updatedAt: 0 },
};

const shot = (id: string): DirectorShot => ({
  id,
  description: 'desc',
  motion: 'motion',
  previewUrl: null,
  seed: 1,
  previewing: false,
  beat: 'hook',
  mood: 'joyful',
  camera: 'medium',
  characterRefs: ['xiaoqi'],
  transitionToNext: 'cut',
});

const ctx3 = { story, shots: [shot('s1'), shot('s2'), shot('s3')] };
const ctx0 = { story, shots: [] };

describe('costModel.estimateCost', () => {
  it('who/character: 影响 character 三件套 + 所有 shots + final', () => {
    const c = estimateCost('who', ctx3);
    expect(c.invalidates).toContain('character.standardImage');
    expect(c.invalidates).toContain('character.threeView');
    expect(c.invalidates).toContain('character.forms');
    expect(c.invalidates).toContain('shots[*].prompt');
    expect(c.invalidates).toContain('shots[*].preview');
    expect(c.invalidates).toContain('final');
    expect(c.credits).toBeGreaterThan(5); // triggers confirm
    expect(c.requiresConfirm).toBe(true);
  });

  it('spine.* 7 维: 不影响 character 但影响所有 shots + final', () => {
    const c = estimateCost('spine.conflict', ctx3);
    expect(c.invalidates).not.toContain('character.standardImage');
    expect(c.invalidates).toContain('shots[*].prompt');
    expect(c.invalidates).toContain('shots[*].preview');
    expect(c.invalidates).toContain('final');
  });

  it('narrative.paragraph: LLM 重生 plan + shots[*] + final', () => {
    const c = estimateCost('narrative.paragraph', ctx3);
    expect(c.invalidates).toContain('shots[*].prompt');
    expect(c.invalidates).toContain('shots[*].preview');
    expect(c.invalidates).toContain('final');
    expect(c.credits).toBeGreaterThan(0);
  });

  it('character.form: 影响所有 shots prompt + preview + final', () => {
    // 依赖图: character.form → shots[*].character_form + prompt → shots[*].preview → final
    const c = estimateCost('character.form', ctx3);
    expect(c.invalidates).toContain('shots[*].prompt');
    expect(c.invalidates).toContain('shots[*].preview');
    expect(c.invalidates).toContain('final');
  });

  it('character.voice: 仅配音, 不影响 shots[*].prompt 或 preview (不影响画)', () => {
    const c = estimateCost('character.voice', ctx3);
    expect(c.invalidates).not.toContain('shots[*].prompt');
    expect(c.invalidates).not.toContain('shots[*].preview');
    expect(c.invalidates).toContain('final'); // 合成要配音
  });

  it('shot.prompt with shotIndex: 仅该镜 (final 单独走 finalize)', () => {
    const c = estimateCost('shot.prompt', { ...ctx3, shotIndex: 1 });
    expect(c.invalidates).toContain('shots[1].preview');
    expect(c.invalidates).not.toContain('shots[*].preview');
    expect(c.invalidates).not.toContain('final');
    expect(c.credits).toBe(CREDIT_PREVIEW);
  });

  it('shot.cinematography with shotIndex: 仅该镜 (final 单独走 finalize)', () => {
    const c = estimateCost('shot.cinematography', { ...ctx3, shotIndex: 0 });
    expect(c.invalidates).toContain('shots[0].preview');
    expect(c.invalidates).not.toContain('shots[*].prompt'); // 单镜改动不重生 plan
    expect(c.invalidates).not.toContain('final'); // finalize 是单独操作
  });

  it('shot.sound: 仅该镜 (final 单独走 finalize)', () => {
    const c = estimateCost('shot.sound', { ...ctx3, shotIndex: 2 });
    expect(c.invalidates).toContain('shots[2].preview');
    expect(c.invalidates).not.toContain('final');
  });

  it('shot.reorder: 影响所有镜衔接 → all shots[*] + final', () => {
    const c = estimateCost('shot.reorder', ctx3);
    expect(c.invalidates).toContain('shots[*].prompt');
    expect(c.invalidates).toContain('shots[*].preview');
    expect(c.invalidates).toContain('final');
  });

  it('shot.insert 在空 shots 上: 仅该镜 + 不需 finalize', () => {
    const c = estimateCost('shot.insert', ctx0);
    expect(c.invalidates).not.toContain('final');
    expect(c.credits).toBe(CREDIT_PREVIEW);
  });

  it('shot.insert 在 3 镜上: 所有镜 + final', () => {
    const c = estimateCost('shot.insert', ctx3);
    expect(c.invalidates).toContain('shots[*].preview');
    expect(c.invalidates).toContain('final');
  });

  it('finalize: 仅 final', () => {
    const c = estimateCost('finalize', ctx3);
    expect(c.invalidates).toEqual(['final']);
    expect(c.credits).toBe(CREDIT_FINALIZE);
  });

  it('requiresConfirm 阈值 = 5 学币', () => {
    expect(estimateCost('shot.cinematography', { ...ctx0, shotIndex: 0 }).requiresConfirm).toBe(false);
    expect(estimateCost('who', ctx3).requiresConfirm).toBe(true);
    expect(estimateCost('finalize', ctx3).requiresConfirm).toBe(true); // 8 > 5
  });

  it('formatCost: 学币 + 秒', () => {
    const c = estimateCost('who', ctx3);
    expect(formatCost(c)).toMatch(/学币/);
    expect(formatCost(c)).toMatch(/秒/);
  });

  it('formatInvalidates: final + shots[*].prompt 等转中文', () => {
    const labels = formatInvalidates(['final', 'shots[*].prompt', 'shots[0].preview', 'character.threeView']);
    expect(labels[0]).toContain('整片');
    expect(labels[1]).toContain('所有分镜提示词');
    expect(labels[2]).toContain('分镜 1');
    expect(labels[3]).toContain('三视图');
  });

  it('所有 ChangeKind 都能 estimate（不抛）', () => {
    const all: ChangeKind[] = [
      'who', 'wants', 'but', 'ending',
      'spine.core', 'spine.conflict', 'spine.world', 'spine.tone', 'spine.theme_color', 'spine.audience', 'spine.ending_moral',
      'narrative.paragraph', 'narrative.scene',
      'character', 'character.form', 'character.expression', 'character.voice',
      'style',
      'shot.prompt', 'shot.cinematography', 'shot.sound', 'shot.character_form',
      'shot.reorder', 'shot.insert', 'shot.delete',
      'finalize',
    ];
    for (const kind of all) {
      expect(() => estimateCost(kind, ctx3)).not.toThrow();
    }
  });
});

// 在文件底部重复成本常量, 与 costModel.ts 同步 (避免循环 import)
const CREDIT_PREVIEW = 5;
const CREDIT_FINALIZE = 8;