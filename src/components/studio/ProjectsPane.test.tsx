// ProjectsPane 组件测试 (W8 M1-C / M1-E)
// 覆盖:
//   - 空状态: 显示 "还没有项目" + 新建按钮
//   - 有列表: 渲染项目标题 + 阶段徽标
//   - 当前项目卡: 显示 + 阶段 + 学币
//   - 新建按钮: 调 create + start
//   - 首页按钮: 调 onBackHome
//   - 错误: 显示 lastError 红条

import { fireEvent, render, screen } from '@testing-library/react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { ProjectMeta, ProjectSummary } from '../../api/tauri';

let mockCurrent: ProjectMeta | null = null;
let mockList: ProjectSummary[] = [];
let mockLoading = false;
let mockSaving = false;
let mockLastError: string | null = null;

const createMock = vi.fn();
const openMock = vi.fn();
const renameMock = vi.fn();
const removeMock = vi.fn();
const startMock = vi.fn();

vi.mock('../../stores/projectStore', () => {
  const callable = ((selector: (s: unknown) => unknown) => {
    const state = {
      current: mockCurrent,
      list: mockList,
      loading: mockLoading,
      saving: mockSaving,
      lastError: mockLastError,
    };
    return selector(state);
  }) as ((s: (s: unknown) => unknown) => unknown) & {
    getState: () => {
      create: typeof createMock;
      open: typeof openMock;
      rename: typeof renameMock;
      remove: typeof removeMock;
    };
  };
  callable.getState = () => ({
    create: createMock,
    open: openMock,
    rename: renameMock,
    remove: removeMock,
  });
  return { useProjectStore: callable };
});

vi.mock('../../stores/studioStore', () => {
  const callable = ((_selector: (s: unknown) => unknown) => undefined) as ((s: (s: unknown) => unknown) => unknown) & {
    getState: () => { start: typeof startMock };
  };
  callable.getState = () => ({ start: startMock });
  return { useStudioStore: callable };
});

import ProjectsPane from './ProjectsPane';

beforeEach(() => {
  createMock.mockReset();
  openMock.mockReset();
  renameMock.mockReset();
  removeMock.mockReset();
  startMock.mockReset();
  mockCurrent = null;
  mockList = [];
  mockLoading = false;
  mockSaving = false;
  mockLastError = null;
});

function summary(o: Partial<ProjectSummary> = {}): ProjectSummary {
  return {
    id: 'p1',
    title: '小猫追蝴蝶',
    levelId: null,
    cursor: 2,
    thumbPath: null,
    totalCredits: 10,
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_000,
    ...o,
  };
}

describe('ProjectsPane', () => {
  it('空状态: 显示 "还没有项目" + 新建按钮', () => {
    render(<ProjectsPane onBackHome={vi.fn()} />);
    expect(screen.getByText(/还没有项目/)).toBeDefined();
    expect(screen.getByRole('button', { name: /新建项目/ })).toBeDefined();
  });

  it('首页按钮: 调 onBackHome', () => {
    const onBack = vi.fn();
    render(<ProjectsPane onBackHome={onBack} />);
    fireEvent.click(screen.getByRole('button', { name: '首页' }));
    expect(onBack).toHaveBeenCalled();
  });

  it('有列表: 渲染项目标题 + 阶段徽标', () => {
    mockList = [
      summary({ id: 'a', title: '小猫历险', cursor: 2 }),
      summary({ id: 'b', title: '小狗历险', cursor: 4 }),
    ];
    render(<ProjectsPane onBackHome={vi.fn()} />);
    expect(screen.getByText('小猫历险')).toBeDefined();
    expect(screen.getByText('小狗历险')).toBeDefined();
    expect(screen.getAllByText(/主角|画风|分镜/).length).toBeGreaterThan(0);
  });

  it('当前项目卡: 显示标题 + 学币', () => {
    mockCurrent = summary({ id: 'cur', title: '我的小电影', cursor: 1, totalCredits: 5 });
    render(<ProjectsPane onBackHome={vi.fn()} />);
    expect(screen.getByText('我的小电影')).toBeDefined();
    expect(screen.getByText(/已用 5 学币/)).toBeDefined();
  });

  it('点击项目 → 调 open', () => {
    mockList = [summary({ id: 'a', title: '点这个' })];
    render(<ProjectsPane onBackHome={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /打开项目 点这个/ }));
    expect(openMock).toHaveBeenCalledWith('a');
  });

  it('新建按钮: 调 create + start', () => {
    mockList = [summary({ id: 'a' })];
    createMock.mockResolvedValueOnce(summary({ id: 'new' }));
    render(<ProjectsPane onBackHome={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /新建项目/ }));
    return Promise.resolve().then(() => {
      expect(createMock).toHaveBeenCalled();
      expect(startMock).toHaveBeenCalled();
    });
  });

  it('错误: 显示 lastError', () => {
    mockLastError = '保存失败';
    render(<ProjectsPane onBackHome={vi.fn()} />);
    expect(screen.getByText('保存失败')).toBeDefined();
  });

  it('saving 时显示 "正在保存…"', () => {
    mockSaving = true;
    render(<ProjectsPane onBackHome={vi.fn()} />);
    expect(screen.getByText(/正在保存/)).toBeDefined();
  });
});
