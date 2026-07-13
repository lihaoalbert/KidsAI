// CharacterEditor + ShotFxEditor 测试（Task D 右屏轻量编辑器）
// 覆盖: 渲染档位/色块/预设; 点击回写 directorStore; ↺还原清空

import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import CharacterEditor from './CharacterEditor';
import ShotFxEditor from './ShotFxEditor';
import { useDirectorStore } from '../../../stores/directorStore';

beforeEach(() => {
  useDirectorStore.getState().reset();
});

describe('CharacterEditor', () => {
  it('渲染 6 色块 + 3 档大小 + 4 表情预设', () => {
    render(<CharacterEditor />);
    expect(screen.getAllByTitle(/阳光黄|天空蓝|玫瑰粉|薄荷绿|薰衣紫|可可棕/)).toHaveLength(6);
    expect(screen.getByText('小小')).toBeDefined();
    expect(screen.getByText('正好')).toBeDefined();
    expect(screen.getByText('大大')).toBeDefined();
    expect(screen.getByText('开心')).toBeDefined();
    expect(screen.getByText('勇敢')).toBeDefined();
  });

  it('点击色块 → directorStore.characterTweak.color 被设', () => {
    render(<CharacterEditor />);
    fireEvent.click(screen.getByTitle('天空蓝'));
    expect(useDirectorStore.getState().characterTweak.color).toBe('sky');
  });

  it('点击"大大"档 → size = L', () => {
    render(<CharacterEditor />);
    fireEvent.click(screen.getByText('大大'));
    expect(useDirectorStore.getState().characterTweak.size).toBe('L');
  });

  it('点击"勇敢"表情 → expression = brave', () => {
    render(<CharacterEditor />);
    fireEvent.click(screen.getByText('勇敢'));
    expect(useDirectorStore.getState().characterTweak.expression).toBe('brave');
  });

  it('↺还原 清空所有 tweak', () => {
    useDirectorStore.setState({
      characterTweak: { color: 'sky', size: 'L', expression: 'brave' },
    });
    render(<CharacterEditor />);
    fireEvent.click(screen.getByText('↺ 还原'));
    const t = useDirectorStore.getState().characterTweak;
    expect(t.color).toBeUndefined();
    expect(t.size).toBeUndefined();
    expect(t.expression).toBeUndefined();
  });
});

describe('ShotFxEditor', () => {
  const SHOT_ID = 'shot_x';

  beforeEach(() => {
    useDirectorStore.setState({
      shots: [
        {
          id: SHOT_ID,
          description: '一镜',
          motion: '走',
          previewUrl: null,
          seed: 1,
          previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none",
        },
      ],
    });
  });

  it('渲染 3 速度档 + 5 音效 + 5 滤镜', () => {
    render(<ShotFxEditor shotId={SHOT_ID} />);
    expect(screen.getByText('慢动作')).toBeDefined();
    expect(screen.getByText('超快')).toBeDefined();
    expect(screen.getByText('森林鸟鸣')).toBeDefined();
    expect(screen.getByText('孩子笑声')).toBeDefined();
    expect(screen.getByText('暖阳光')).toBeDefined();
    expect(screen.getByText('梦境软')).toBeDefined();
  });

  it('点击"超快" → 该镜 fx.speed = fast', () => {
    render(<ShotFxEditor shotId={SHOT_ID} />);
    fireEvent.click(screen.getByText('超快'));
    const shot = useDirectorStore.getState().shots.find((s) => s.id === SHOT_ID);
    expect(shot?.fx?.speed).toBe('fast');
  });

  it('点击"魔法叮咚" → fx.sound = magic', () => {
    render(<ShotFxEditor shotId={SHOT_ID} />);
    fireEvent.click(screen.getByText('魔法叮咚'));
    const shot = useDirectorStore.getState().shots.find((s) => s.id === SHOT_ID);
    expect(shot?.fx?.sound).toBe('magic');
  });

  it('点击"樱花雨" → fx.filter = sakura', () => {
    render(<ShotFxEditor shotId={SHOT_ID} />);
    fireEvent.click(screen.getByText('樱花雨'));
    const shot = useDirectorStore.getState().shots.find((s) => s.id === SHOT_ID);
    expect(shot?.fx?.filter).toBe('sakura');
  });

  it('↺还原 清空该镜 fx', () => {
    useDirectorStore.getState().setShotFx(SHOT_ID, {
      speed: 'fast',
      sound: 'magic',
      filter: 'sakura',
    });
    render(<ShotFxEditor shotId={SHOT_ID} />);
    fireEvent.click(screen.getByText('↺ 还原'));
    const shot = useDirectorStore.getState().shots.find((s) => s.id === SHOT_ID);
    expect(shot?.fx?.speed).toBeUndefined();
    expect(shot?.fx?.sound).toBeUndefined();
    expect(shot?.fx?.filter).toBeUndefined();
  });

  it('shotId 不存在 → 渲染 null', () => {
    const { container } = render(<ShotFxEditor shotId="missing" />);
    expect(container.firstChild).toBeNull();
  });
});