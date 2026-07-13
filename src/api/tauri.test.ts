// W4.6 #4: DirectorPlan 严格 schema + 5 拍节奏 + 容错
// 5 case: 老 schema 容错 / 严格 enum / shots 长度 3-5 边界 / transition_to_next 边界 / 完整 plan 通过

import { describe, it, expect } from 'vitest';
import {
  parseDirectorPlan,
  type DirectorPlan,
} from './tauri';

function buildShot(overrides: Partial<{
  description: string;
  motion: string;
  beat: string;
  mood: string;
  camera: string;
  character_refs: string[];
  transition_to_next: string;
}> = {}) {
  return {
    description: overrides.description ?? '小猫追蝴蝶',
    motion: overrides.motion ?? '小猫在花园里追蝴蝶跑',
    beat: overrides.beat ?? 'hook',
    mood: overrides.mood ?? 'joyful',
    camera: overrides.camera ?? 'wide',
    character_refs: overrides.character_refs ?? ['xiaoqi'],
    transition_to_next: overrides.transition_to_next ?? 'cut',
  };
}

function buildPlan(shotOverrides: Array<Parameters<typeof buildShot>[0]> = []) {
  return {
    idea: '小猫追蝴蝶',
    character_id: 'xiaoqi',
    style_id: 'cartoon',
    shots: shotOverrides.map(buildShot),
  };
}

describe('parseDirectorPlan — W4.6 #4 严格 schema', () => {
  it('case 1: 老 plan 形状 (只有 description/motion) 失败, error 字段说明原因', () => {
    const oldShape = JSON.stringify({
      idea: '小猫追蝴蝶',
      character_id: 'xiaoqi',
      style_id: 'cartoon',
      shots: [
        { description: 'd1', motion: 'm1' },
        { description: 'd2', motion: 'm2' },
        { description: 'd3', motion: 'm3' },
      ],
    });
    const r = parseDirectorPlan(oldShape);
    expect(r.ok).toBe(false);
    expect(r.plan).toBeNull();
    expect(r.error).toContain('schema failed');
    expect(r.raw).toBe(oldShape);
  });

  it('case 2: 错误 enum 值 (mood=happiness 不在白名单) 失败', () => {
    const badEnum = JSON.stringify(
      buildPlan([
        { beat: 'hook', mood: 'happiness', camera: 'wide', transition_to_next: 'cut' },
        { beat: 'conflict', mood: 'tense', camera: 'medium', transition_to_next: 'cut' },
        { beat: 'payoff', mood: 'epic', camera: 'overhead', transition_to_next: 'none' },
      ]),
    );
    const r = parseDirectorPlan(badEnum);
    expect(r.ok).toBe(false);
    expect(r.error).toContain('schema failed');
  });

  it('case 3: shots.length=4 通过 (hook x2 + conflict + payoff)', () => {
    const fourShot = JSON.stringify(
      buildPlan([
        { beat: 'hook', mood: 'joyful', camera: 'wide', transition_to_next: 'fade' },
        { beat: 'hook', mood: 'tense', camera: 'close', transition_to_next: 'cut' },
        { beat: 'conflict', mood: 'tense', camera: 'follow', transition_to_next: 'dissolve' },
        { beat: 'payoff', mood: 'epic', camera: 'overhead', transition_to_next: 'none' },
      ]),
    );
    const r = parseDirectorPlan(fourShot);
    expect(r.ok).toBe(true);
    expect(r.plan?.shots).toHaveLength(4);
  });

  it('case 4: shots.length=5 通过 (hook x2 + conflict x2 + payoff)', () => {
    const fiveShot = JSON.stringify(
      buildPlan([
        { beat: 'hook', mood: 'calm', camera: 'wide', transition_to_next: 'fade' },
        { beat: 'hook', mood: 'joyful', camera: 'medium', transition_to_next: 'cut' },
        { beat: 'conflict', mood: 'tense', camera: 'close', transition_to_next: 'wipe' },
        { beat: 'conflict', mood: 'tense', camera: 'follow', transition_to_next: 'dissolve' },
        { beat: 'payoff', mood: 'epic', camera: 'overhead', transition_to_next: 'none' },
      ]),
    );
    const r = parseDirectorPlan(fiveShot);
    expect(r.ok).toBe(true);
    expect(r.plan?.shots).toHaveLength(5);
  });

  it('case 5: shots.length=6 失败 (超过 5)', () => {
    const tooLong = JSON.stringify({
      idea: 'x',
      character_id: 'xiaoqi',
      style_id: 'cartoon',
      shots: Array.from({ length: 6 }, () => buildShot()),
    });
    const r = parseDirectorPlan(tooLong);
    expect(r.ok).toBe(false);
  });

  it('case 6: 最后一镜 transition_to_next != "none" 失败', () => {
    const bad = JSON.stringify(
      buildPlan([
        { beat: 'hook', transition_to_next: 'fade' },
        { beat: 'conflict', transition_to_next: 'cut' },
        { beat: 'payoff', transition_to_next: 'fade' }, // ❌ 应该 none
      ]),
    );
    const r = parseDirectorPlan(bad);
    expect(r.ok).toBe(false);
    expect(r.error).toContain('schema failed');
  });

  it('case 7: 非最后一镜 transition_to_next 缺失 失败 (手动构造 JSON 跳过 spread ??)', () => {
    // buildShot 用 ?? 会把 undefined 替换成 'cut', 所以直接构造 JSON 才能测出 undefined → 字段缺失
    const missing = JSON.stringify({
      idea: '小猫追蝴蝶',
      character_id: 'xiaoqi',
      style_id: 'cartoon',
      shots: [
        {
          description: 'd1',
          motion: 'm1',
          beat: 'hook',
          mood: 'joyful',
          camera: 'wide',
          character_refs: ['xiaoqi'],
          // transition_to_next 故意缺失
        },
        {
          description: 'd2',
          motion: 'm2',
          beat: 'conflict',
          mood: 'tense',
          camera: 'medium',
          character_refs: ['xiaoqi'],
          transition_to_next: 'cut',
        },
        {
          description: 'd3',
          motion: 'm3',
          beat: 'payoff',
          mood: 'epic',
          camera: 'overhead',
          character_refs: ['xiaoqi'],
          transition_to_next: 'none',
        },
      ],
    });
    const r = parseDirectorPlan(missing);
    expect(r.ok).toBe(false);
    expect(r.error).toContain('schema failed');
  });

  it('case 8: character_refs 为空数组失败', () => {
    const emptyRefs = JSON.stringify(
      buildPlan([
        { character_refs: [] },
        { character_refs: ['xiaoqi'] },
        { character_refs: ['xiaoqi'], transition_to_next: 'none', beat: 'payoff' },
      ]),
    );
    const r = parseDirectorPlan(emptyRefs);
    expect(r.ok).toBe(false);
  });

  it('case 9: character_id 不在白名单 (xiaoming 不存在) 失败', () => {
    const badChar = JSON.stringify({
      idea: 'x',
      character_id: 'xiaoming', // ❌
      style_id: 'cartoon',
      shots: [
        buildShot({ beat: 'hook', transition_to_next: 'cut' }),
        buildShot({ beat: 'conflict', transition_to_next: 'cut' }),
        buildShot({ beat: 'payoff', transition_to_next: 'none' }),
      ],
    });
    const r = parseDirectorPlan(badChar);
    expect(r.ok).toBe(false);
  });

  it('case 10: ```json 代码块包裹时仍能解析', () => {
    const fenced =
      '```json\n' +
      JSON.stringify(
        buildPlan([
          { beat: 'hook', mood: 'joyful', camera: 'wide', transition_to_next: 'cut' },
          { beat: 'conflict', mood: 'tense', camera: 'medium', transition_to_next: 'cut' },
          { beat: 'payoff', mood: 'epic', camera: 'overhead', transition_to_next: 'none' },
        ]),
      ) +
      '\n```';
    const r = parseDirectorPlan(fenced);
    expect(r.ok).toBe(true);
    expect(r.plan?.shots).toHaveLength(3);
  });

  it('case 11: 空字符串 / 非 JSON / 全部 fail', () => {
    expect(parseDirectorPlan('').ok).toBe(false);
    expect(parseDirectorPlan('not json').ok).toBe(false);
    expect(parseDirectorPlan('{').ok).toBe(false);
  });

  it('case 12: 完整 plan 通过 + 字段透传正确', () => {
    const full: DirectorPlan = {
      idea: '小猫追蝴蝶',
      character_id: 'xiaoqi',
      style_id: 'anime',
      shots: [
        {
          description: '小猫发现蝴蝶',
          motion: '小猫抬头看到蝴蝶',
          beat: 'hook',
          mood: 'joyful',
          camera: 'wide',
          character_refs: ['xiaoqi'],
          transition_to_next: 'fade',
        },
        {
          description: '小猫追逐',
          motion: '小猫飞奔',
          beat: 'conflict',
          mood: 'tense',
          camera: 'follow',
          character_refs: ['xiaoqi'],
          transition_to_next: 'cut',
        },
        {
          description: '小猫抓住',
          motion: '小猫扑向蝴蝶',
          beat: 'payoff',
          mood: 'epic',
          camera: 'close',
          character_refs: ['xiaoqi'],
          transition_to_next: 'none',
        },
      ],
    };
    const r = parseDirectorPlan(JSON.stringify(full));
    expect(r.ok).toBe(true);
    expect(r.plan).toEqual(full);
  });

  it('case 13: idea 是空字符串失败', () => {
    const emptyIdea = JSON.stringify({
      idea: '',
      character_id: 'xiaoqi',
      style_id: 'cartoon',
      shots: [buildShot(), buildShot(), buildShot({ transition_to_next: 'none', beat: 'payoff' })],
    });
    const r = parseDirectorPlan(emptyIdea);
    expect(r.ok).toBe(false);
  });

  it('case 14: parse error 时 error 字段填 JSON parse 失败原因 (排障)', () => {
    const r = parseDirectorPlan('{ invalid');
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/JSON parse failed/);
  });
});