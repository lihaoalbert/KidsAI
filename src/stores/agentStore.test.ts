// agentStore 状态机测试（W3.pre Batch B + W3.4 角色 + W3.5 指哪打哪 + W3.6 风格）
// 覆盖：chunk 累积 / final_answer 替换 / cancelled / error / started sessionId 回填
//       send() 异常路径兜底 / cancel() 幂等 / 角色 + 风格加载 + 注入 runAgent
//       editing 状态机 / editImageAsset 调 runAgent / tool_result 自动 patch sourceAssetUrl

import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';
import type { AgentEvent, AgentRunResponse, Character, StylePreset, AgentAsset } from '../api/tauri';
import type { ExtractedFrame } from '../utils/frameExtractor';

// 事件总线：测试用，agentStore 调 onAgentEvent 时捕获 handler
let capturedHandler: ((e: AgentEvent) => void) | null = null;
let capturedUnlisten: (() => void) | null = null;
const runAgentMock: Mock = vi.fn();
const cancelAgentMock: Mock = vi.fn(async () => true);
const listCharactersMock: Mock = vi.fn();
const listStylesMock: Mock = vi.fn();

vi.mock('../api/tauri', () => ({
  runAgent: (...args: unknown[]) => runAgentMock(...args),
  cancelAgent: (...args: unknown[]) => cancelAgentMock(...args),
  onAgentEvent: async (handler: (e: AgentEvent) => void) => {
    capturedHandler = handler;
    capturedUnlisten = () => {
      capturedHandler = null;
    };
    return capturedUnlisten;
  },
  listCharacters: (...args: unknown[]) => listCharactersMock(...args),
  listStyles: (...args: unknown[]) => listStylesMock(...args),
}));

// import 必须在 mock 之后
import { useAgentStore } from './agentStore';

function push(e: AgentEvent) {
  if (!capturedHandler) throw new Error('handler not captured — call subscribeEvents first');
  capturedHandler(e);
}

function makeRunResponse(overrides: Partial<AgentRunResponse> = {}): AgentRunResponse {
  return {
    sessionId: 'sess_server_1',
    levelId: 'L1',
    finalAnswer: '你好小启！',
    thoughts: [],
    toolCalls: [],
    assets: [],
    durationMs: 100,
    tokensUsed: 50,
    cancelled: false,
    ...overrides,
  };
}

describe('agentStore', () => {
  beforeEach(() => {
    // 每个 case 重新拿 fresh store
    useAgentStore.setState({
      sessionId: null,
      levelId: null,
      messages: [],
      assets: [],
      streaming: null,
      isRunning: false,
      lastResponse: null,
      error: null,
      unlisten: null,
      characters: [],
      character: null,
      styles: [],
      style: null,
      editing: null,
      extractedFrames: [],
      recreateProgress: null,
    });
    capturedHandler = null;
    capturedUnlisten = null;
    runAgentMock.mockReset();
    cancelAgentMock.mockClear();
    listCharactersMock.mockReset();
    listStylesMock.mockReset();
  });

  // ---------- started event ----------
  it('started event 回填 server-side sessionId', async () => {
    useAgentStore.getState().startSession('L1');
    // 前端预设的 sess_xxx
    expect(useAgentStore.getState().sessionId).toMatch(/^sess_/);

    await useAgentStore.getState().subscribeEvents();
    push({ kind: 'started', sessionId: 'sess_server_real' });

    expect(useAgentStore.getState().sessionId).toBe('sess_server_real');
  });

  // ---------- chunk 累积 ----------
  it('chunk 事件：同 step 累积到一条消息，新 step 创建新消息', async () => {
    await useAgentStore.getState().subscribeEvents();

    // step 1: 3 个 chunk 累积
    push({ kind: 'chunk', sessionId: 's', step: 1, delta: '你' });
    push({ kind: 'chunk', sessionId: 's', step: 1, delta: '好' });
    push({ kind: 'chunk', sessionId: 's', step: 1, delta: '！' });

    let state = useAgentStore.getState();
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].role).toBe('assistant');
    expect(state.messages[0].content).toBe('你好！');
    expect(state.streaming).toEqual({ messageId: state.messages[0].id, step: 1 });

    // step 2: 新消息
    push({ kind: 'chunk', sessionId: 's', step: 2, delta: '欢迎' });

    state = useAgentStore.getState();
    expect(state.messages).toHaveLength(2);
    expect(state.messages[1].content).toBe('欢迎');
    expect(state.streaming?.step).toBe(2);
  });

  // ---------- final_answer 替换 ----------
  it('final_answer 事件用干净答案替换 streaming 槽位', async () => {
    await useAgentStore.getState().subscribeEvents();

    push({ kind: 'chunk', sessionId: 's', step: 1, delta: '草' });
    push({ kind: 'chunk', sessionId: 's', step: 1, delta: '稿' });
    expect(useAgentStore.getState().messages[0].content).toBe('草稿');

    push({ kind: 'final_answer', sessionId: 's', answer: '干净版' });

    const state = useAgentStore.getState();
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].content).toBe('干净版');
    expect(state.streaming).toBeNull();
  });

  // ---------- cancelled ----------
  it('cancelled 事件设置 error=已取消 + 清空 streaming + isRunning=false', async () => {
    // 先订阅事件（必须先于 send，否则 send 内部会自动调 subscribe）
    await useAgentStore.getState().subscribeEvents();

    runAgentMock.mockImplementation(
      () => new Promise<AgentRunResponse>(() => {}), // never resolves
    );

    // fire-and-forget — send 内部的 runAgent 永不 resolve
    void useAgentStore.getState().send('L1', 'hi', 'system');
    // 让 microtask 跑一下，isRunning 才被置为 true
    await Promise.resolve();

    expect(useAgentStore.getState().isRunning).toBe(true);

    push({ kind: 'chunk', sessionId: 's', step: 1, delta: '正在生成' });
    push({ kind: 'cancelled', sessionId: 's' });

    const state = useAgentStore.getState();
    expect(state.isRunning).toBe(false);
    expect(state.error).toBe('已取消');
    expect(state.streaming).toBeNull();
  });

  // ---------- error ----------
  it('error 事件设置 error 消息 + isRunning=false + 清空 streaming', async () => {
    await useAgentStore.getState().subscribeEvents();

    push({ kind: 'chunk', sessionId: 's', step: 1, delta: 'X' });
    push({
      kind: 'error',
      sessionId: 's',
      message: 'tool generate_image failed: missing prompt',
    });

    const state = useAgentStore.getState();
    expect(state.isRunning).toBe(false);
    expect(state.error).toBe('tool generate_image failed: missing prompt');
    expect(state.streaming).toBeNull();
  });

  // ---------- send() 异常路径兜底 ----------
  it('send() 异常时，streaming 槽位填入 resp.finalAnswer', async () => {
    runAgentMock.mockResolvedValue(makeRunResponse({ finalAnswer: '兜底内容' }));
    await useAgentStore.getState().subscribeEvents();

    // 模拟 streaming 槽位有内容
    push({ kind: 'chunk', sessionId: 's', step: 1, delta: 'partial' });

    await useAgentStore.getState().send('L1', 'hi', 'sys');

    const state = useAgentStore.getState();
    expect(state.isRunning).toBe(false);
    expect(state.error).toBeNull();
    // streaming 槽位的内容被 resp.finalAnswer 替换
    const streamingMsg = state.messages.find((m) => m.id === state.messages[0].id);
    expect(streamingMsg?.content).toBe('兜底内容');
    expect(state.lastResponse?.finalAnswer).toBe('兜底内容');
  });

  it('send() 异常时，若无 streaming 槽位则 append 新 assistant 消息', async () => {
    runAgentMock.mockResolvedValue(makeRunResponse({ finalAnswer: '全新内容' }));
    await useAgentStore.getState().subscribeEvents();
    // 不推任何 chunk 事件 → 无 streaming 槽位

    await useAgentStore.getState().send('L1', 'hi', 'sys');

    const state = useAgentStore.getState();
    expect(state.messages.some((m) => m.content === '全新内容' && m.role === 'assistant')).toBe(
      true,
    );
  });

  // ---------- cancel() 幂等 ----------
  it('cancel() 调 Tauri cancelAgent，可多次点击不抛错', async () => {
    useAgentStore.setState({ sessionId: 'sess_1', isRunning: true });

    await useAgentStore.getState().cancel();
    await useAgentStore.getState().cancel();
    await useAgentStore.getState().cancel();

    expect(cancelAgentMock).toHaveBeenCalledTimes(3);
    expect(cancelAgentMock).toHaveBeenCalledWith('sess_1');
  });

  it('cancel() 在没在跑时是 no-op', async () => {
    useAgentStore.setState({ sessionId: 'sess_1', isRunning: false });

    await useAgentStore.getState().cancel();

    expect(cancelAgentMock).not.toHaveBeenCalled();
  });

  // ---------- 工具事件累积 ----------
  it('tool_call + tool_result 事件在 messages 中追加并累计 assets', async () => {
    await useAgentStore.getState().subscribeEvents();

    push({
      kind: 'tool_call',
      sessionId: 's',
      step: 1,
      tool: 'generate_image',
      args: { prompt: 'x' },
    });
    push({
      kind: 'tool_result',
      sessionId: 's',
      step: 1,
      tool: 'generate_image',
      result: '已生成',
      assets: [
        { type: 'image', url: 'http://a', prompt: 'x', tool: 'generate_image', tokensCost: 10 },
      ],
    });

    const state = useAgentStore.getState();
    const toolCallMsg = state.messages.find((m) => m.meta?.toolName === 'generate_image' && m.role === 'system');
    const toolResultMsg = state.messages.find((m) => m.meta?.toolName === 'generate_image' && m.role === 'tool');
    expect(toolCallMsg).toBeDefined();
    expect(toolResultMsg).toBeDefined();
    expect(toolResultMsg?.content).toBe('已生成');
    expect(state.assets).toHaveLength(1);
    expect(state.assets[0].type).toBe('image');
  });

  it('thought 事件以 step 元数据写入 messages', async () => {
    await useAgentStore.getState().subscribeEvents();

    push({ kind: 'thought', sessionId: 's', step: 2, thought: '我在想...' });

    const state = useAgentStore.getState();
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].role).toBe('thought');
    expect(state.messages[0].content).toBe('我在想...');
    expect(state.messages[0].meta?.step).toBe(2);
  });

  // ---------- W3.4 角色 ----------
  const xiaoqi: Character = {
    id: 'xiaoqi',
    name: '小启',
    description: '9岁小猫女孩，黄色短发',
    styleTags: ['cartoon', 'child_friendly'],
  };
  const xiaoyue: Character = {
    id: 'xiaoyue',
    name: '小月',
    description: '8岁女孩，双马尾红裙',
    styleTags: ['cartoon', 'child_friendly'],
  };

  it('loadCharacters 从 backend 拉列表并写入 state', async () => {
    listCharactersMock.mockResolvedValue([xiaoqi, xiaoyue]);

    await useAgentStore.getState().loadCharacters();

    const state = useAgentStore.getState();
    expect(state.characters).toHaveLength(2);
    expect(state.characters[0].id).toBe('xiaoqi');
    expect(listCharactersMock).toHaveBeenCalledTimes(1);
  });

  it('loadCharacters 已有缓存时不再重复拉', async () => {
    listCharactersMock.mockResolvedValue([xiaoqi]);
    useAgentStore.setState({ characters: [xiaoqi] });

    await useAgentStore.getState().loadCharacters();

    expect(listCharactersMock).not.toHaveBeenCalled();
  });

  it('loadCharacters 失败时不抛错，characters 留空', async () => {
    listCharactersMock.mockRejectedValue(new Error('network'));

    await expect(useAgentStore.getState().loadCharacters()).resolves.toBeUndefined();

    expect(useAgentStore.getState().characters).toEqual([]);
  });

  it('setCharacter 切换当前角色', () => {
    expect(useAgentStore.getState().character).toBeNull();

    useAgentStore.getState().setCharacter(xiaoqi);
    expect(useAgentStore.getState().character).toEqual(xiaoqi);

    useAgentStore.getState().setCharacter(xiaoyue);
    expect(useAgentStore.getState().character).toEqual(xiaoyue);

    useAgentStore.getState().setCharacter(null);
    expect(useAgentStore.getState().character).toBeNull();
  });

  it('send() 时把当前 characterId 传给 backend；无角色则不传', async () => {
    runAgentMock.mockResolvedValue(makeRunResponse());

    // 路径 1：设了角色 → characterId 应该传给 backend
    useAgentStore.getState().setCharacter(xiaoqi);
    await useAgentStore.getState().send('L1', 'hi', 'sys');
    expect(runAgentMock).toHaveBeenLastCalledWith({
      levelId: 'L1',
      userInput: 'hi',
      systemPrompt: 'sys',
      characterId: 'xiaoqi',
      styleId: undefined,
    });

    // 路径 2：清掉角色 → 不传 characterId
    runAgentMock.mockResolvedValue(makeRunResponse());
    useAgentStore.getState().setCharacter(null);
    await useAgentStore.getState().send('L1', 'hi2', 'sys');
    expect(runAgentMock).toHaveBeenLastCalledWith({
      levelId: 'L1',
      userInput: 'hi2',
      systemPrompt: 'sys',
      characterId: undefined,
      styleId: undefined,
    });
  });

  // ---------- W3.6 风格 ----------
  const inkStyle: StylePreset = {
    id: 'ink',
    name: '🖌️ 水墨',
    description: '中国传统水墨画风格',
    styleTags: ['ink_wash'],
  };
  const pixelStyle: StylePreset = {
    id: 'pixel',
    name: '📺 像素',
    description: '复古 8-bit 像素艺术',
    styleTags: ['pixel_art'],
  };

  it('loadStyles 从 backend 拉列表并写入 state', async () => {
    listStylesMock.mockResolvedValue([inkStyle, pixelStyle]);

    await useAgentStore.getState().loadStyles();

    expect(useAgentStore.getState().styles).toHaveLength(2);
    expect(useAgentStore.getState().styles[0].id).toBe('ink');
    expect(listStylesMock).toHaveBeenCalledTimes(1);
  });

  it('loadStyles 已有缓存时不再重复拉', async () => {
    listStylesMock.mockResolvedValue([inkStyle]);
    useAgentStore.setState({ styles: [inkStyle] });

    await useAgentStore.getState().loadStyles();

    expect(listStylesMock).not.toHaveBeenCalled();
  });

  it('loadStyles 失败时不抛错，styles 留空', async () => {
    listStylesMock.mockRejectedValue(new Error('network'));

    await expect(useAgentStore.getState().loadStyles()).resolves.toBeUndefined();

    expect(useAgentStore.getState().styles).toEqual([]);
  });

  it('setStyle 切换当前风格', () => {
    expect(useAgentStore.getState().style).toBeNull();

    useAgentStore.getState().setStyle(inkStyle);
    expect(useAgentStore.getState().style).toEqual(inkStyle);

    useAgentStore.getState().setStyle(null);
    expect(useAgentStore.getState().style).toBeNull();
  });

  it('send() 时 characterId + styleId 二者独立注入；都没设则都不传', async () => {
    runAgentMock.mockResolvedValue(makeRunResponse());

    // 路径 A：都设 → 二者都传
    useAgentStore.getState().setCharacter(xiaoqi);
    useAgentStore.getState().setStyle(inkStyle);
    await useAgentStore.getState().send('L1', 'hi', 'sys');
    expect(runAgentMock).toHaveBeenLastCalledWith({
      levelId: 'L1',
      userInput: 'hi',
      systemPrompt: 'sys',
      characterId: 'xiaoqi',
      styleId: 'ink',
    });

    // 路径 B：只设风格 → 只传 styleId
    runAgentMock.mockResolvedValue(makeRunResponse());
    useAgentStore.getState().setCharacter(null);
    await useAgentStore.getState().send('L1', 'hi', 'sys');
    expect(runAgentMock).toHaveBeenLastCalledWith({
      levelId: 'L1',
      userInput: 'hi',
      systemPrompt: 'sys',
      characterId: undefined,
      styleId: 'ink',
    });

    // 路径 C：都清掉 → 都不传
    runAgentMock.mockResolvedValue(makeRunResponse());
    useAgentStore.getState().setStyle(null);
    await useAgentStore.getState().send('L1', 'hi', 'sys');
    expect(runAgentMock).toHaveBeenLastCalledWith({
      levelId: 'L1',
      userInput: 'hi',
      systemPrompt: 'sys',
      characterId: undefined,
      styleId: undefined,
    });
  });

  // ---------- W3.5 指哪打哪 ----------
  const sampleAsset: AgentAsset = {
    type: 'image',
    url: 'https://example.com/cat.jpg',
    prompt: '一只小猫',
    tool: 'generate_image',
    tokensCost: 10,
  };

  it('setEditing 打开抽屉，记录坐标 + 资产', () => {
    expect(useAgentStore.getState().editing).toBeNull();

    useAgentStore.getState().setEditing(sampleAsset, 0.45, 0.3);

    const editing = useAgentStore.getState().editing;
    expect(editing).not.toBeNull();
    expect(editing?.asset).toEqual(sampleAsset);
    expect(editing?.clickX).toBe(0.45);
    expect(editing?.clickY).toBe(0.3);
  });

  it('clearEditing 关闭抽屉', () => {
    useAgentStore.getState().setEditing(sampleAsset, 0.5, 0.5);
    expect(useAgentStore.getState().editing).not.toBeNull();

    useAgentStore.getState().clearEditing();

    expect(useAgentStore.getState().editing).toBeNull();
  });

  it('editImageAsset 调 runAgent：自动注入 edit_image + 坐标到 system_prompt', async () => {
    runAgentMock.mockResolvedValue(makeRunResponse());

    useAgentStore.getState().setEditing(sampleAsset, 0.45, 0.3);

    await useAgentStore.getState().editImageAsset({
      levelId: 'L1',
      systemPrompt: '你是小启',
      prompt: '把毛色改成橘色',
      tools: ['generate_image'], // store 自动追加 edit_image
    });

    expect(runAgentMock).toHaveBeenCalledTimes(1);
    const call = runAgentMock.mock.calls[0][0];
    expect(call.levelId).toBe('L1');
    expect(call.userInput).toBe('把毛色改成橘色');
    // system_prompt 应被加上 [Editor context] 段
    expect(call.systemPrompt).toContain('[Editor context]');
    expect(call.systemPrompt).toContain('source_image_url=https://example.com/cat.jpg');
    expect(call.systemPrompt).toContain('click_x=45%');
    expect(call.systemPrompt).toContain('click_y=30%');
    expect(call.systemPrompt).toContain('user_intent=把毛色改成橘色');
    // tools 自动追加 edit_image
    expect(call.tools).toContain('edit_image');
    expect(call.tools).toContain('generate_image');
  });

  it('editImageAsset 在没编辑上下文时是 no-op（不调 runAgent）', async () => {
    useAgentStore.getState().clearEditing();

    await useAgentStore.getState().editImageAsset({
      levelId: 'L1',
      systemPrompt: 'sys',
      prompt: '改色',
      tools: ['generate_image'],
    });

    expect(runAgentMock).not.toHaveBeenCalled();
  });

  it('tool_result (edit_image) 时自动 patch 新资产的 sourceAssetUrl + 关闭 editing', async () => {
    await useAgentStore.getState().subscribeEvents();
    useAgentStore.getState().setEditing(sampleAsset, 0.5, 0.5);

    push({
      kind: 'tool_result',
      sessionId: 's',
      step: 1,
      tool: 'edit_image',
      result: '已修改',
      assets: [
        {
          type: 'image',
          url: 'https://new.example.com/edited.jpg',
          prompt: '改色后',
          tool: 'edit_image',
          tokensCost: 12,
        },
      ],
    });

    const state = useAgentStore.getState();
    expect(state.assets).toHaveLength(1);
    expect(state.assets[0].url).toBe('https://new.example.com/edited.jpg');
    // sourceAssetUrl 自动 patch 成被点击资产的 URL
    expect(state.assets[0].sourceAssetUrl).toBe('https://example.com/cat.jpg');
    // editing 在 edit_image 工具完成后被自动清掉
    expect(state.editing).toBeNull();
  });

  it('tool_result (非 edit_image) 不影响 editing 状态', async () => {
    await useAgentStore.getState().subscribeEvents();
    useAgentStore.getState().setEditing(sampleAsset, 0.5, 0.5);

    // 非 edit_image 工具 → editing 应保留（避免无限标记）
    push({
      kind: 'tool_result',
      sessionId: 's',
      step: 1,
      tool: 'generate_image',
      result: '已生成',
      assets: [
        { type: 'image', url: 'https://new.jpg', prompt: '猫', tool: 'generate_image', tokensCost: 10 },
      ],
    });

    const state = useAgentStore.getState();
    expect(state.editing).not.toBeNull();
    // 但 assets 还是被 push 进去了（只是没标记 sourceAssetUrl）
    expect(state.assets).toHaveLength(1);
    expect(state.assets[0].sourceAssetUrl).toBeUndefined();
  });

  // ---------- W3.7+ 拉片复刻 ----------
  const makeFrame = (id: string, ms: number): ExtractedFrame => ({
    id,
    dataUrl: `data:image/jpeg;base64,${id}`,
    timestampMs: ms,
  });

  it('setExtractedFrames / clearExtractedFrames 状态机', () => {
    expect(useAgentStore.getState().extractedFrames).toEqual([]);
    const f1 = makeFrame('a', 100);
    const f2 = makeFrame('b', 200);

    useAgentStore.getState().setExtractedFrames([f1, f2]);
    expect(useAgentStore.getState().extractedFrames).toHaveLength(2);

    useAgentStore.getState().clearExtractedFrames();
    expect(useAgentStore.getState().extractedFrames).toEqual([]);
  });

  it('setRecreateProgress:写入和清空', () => {
    expect(useAgentStore.getState().recreateProgress).toBeNull();
    useAgentStore.getState().setRecreateProgress({ done: 1, total: 5 });
    expect(useAgentStore.getState().recreateProgress).toEqual({ done: 1, total: 5 });
    useAgentStore.getState().setRecreateProgress(null);
    expect(useAgentStore.getState().recreateProgress).toBeNull();
  });

  it('reset() 清掉 extractedFrames + recreateProgress', () => {
    useAgentStore.setState({
      extractedFrames: [makeFrame('a', 100)],
      recreateProgress: { done: 1, total: 5 },
    });

    useAgentStore.getState().reset();

    const state = useAgentStore.getState();
    expect(state.extractedFrames).toEqual([]);
    expect(state.recreateProgress).toBeNull();
  });

  it('recreateFrames:顺序调 N 次 runAgent,每张 prompt 含正确的 [Reference context] + 每张 asset patch sourceAssetUrl', async () => {
    const frames = [
      makeFrame('a', 0),
      makeFrame('b', 1000),
      makeFrame('c', 2000),
    ];

    // 每次 runAgent 返回不同 asset
    runAgentMock
      .mockResolvedValueOnce(
        makeRunResponse({
          assets: [
            {
              type: 'image',
              url: 'https://new/1.jpg',
              prompt: 'frame a',
              tool: 'generate_image',
              tokensCost: 5,
            },
          ],
        }),
      )
      .mockResolvedValueOnce(
        makeRunResponse({
          assets: [
            {
              type: 'image',
              url: 'https://new/2.jpg',
              prompt: 'frame b',
              tool: 'generate_image',
              tokensCost: 5,
            },
          ],
        }),
      )
      .mockResolvedValueOnce(
        makeRunResponse({
          assets: [
            {
              type: 'image',
              url: 'https://new/3.jpg',
              prompt: 'frame c',
              tool: 'generate_image',
              tokensCost: 5,
            },
          ],
        }),
      );

    await useAgentStore.getState().recreateFrames({
      levelId: 'L7',
      systemPrompt: '你是创创',
      frames,
      tools: ['generate_image'],
    });

    // runAgent 被调 3 次
    expect(runAgentMock).toHaveBeenCalledTimes(3);

    // 每次 systemPrompt 都拼了 [Reference context] 段,且含对应帧的 dataUrl + timestamp
    const calls = runAgentMock.mock.calls;
    expect((calls[0][0] as { systemPrompt: string }).systemPrompt).toContain('[Reference context]');
    expect((calls[0][0] as { systemPrompt: string }).systemPrompt).toContain('data:image/jpeg;base64,a');
    expect((calls[0][0] as { systemPrompt: string }).systemPrompt).toContain('timestamp=0.0s');

    expect((calls[1][0] as { systemPrompt: string }).systemPrompt).toContain('data:image/jpeg;base64,b');
    expect((calls[1][0] as { systemPrompt: string }).systemPrompt).toContain('timestamp=1.0s');

    expect((calls[2][0] as { systemPrompt: string }).systemPrompt).toContain('data:image/jpeg;base64,c');
    expect((calls[2][0] as { systemPrompt: string }).systemPrompt).toContain('timestamp=2.0s');

    // 三张新 asset 都进了 store,且各自 sourceAssetUrl = 自己的帧 dataUrl
    const state = useAgentStore.getState();
    expect(state.assets).toHaveLength(3);
    expect(state.assets[0].url).toBe('https://new/1.jpg');
    expect(state.assets[0].sourceAssetUrl).toBe('data:image/jpeg;base64,a');
    expect(state.assets[1].url).toBe('https://new/2.jpg');
    expect(state.assets[1].sourceAssetUrl).toBe('data:image/jpeg;base64,b');
    expect(state.assets[2].url).toBe('https://new/3.jpg');
    expect(state.assets[2].sourceAssetUrl).toBe('data:image/jpeg;base64,c');

    // 进度:全部完成后回 null
    expect(state.recreateProgress).toBeNull();
    expect(state.isRunning).toBe(false);

    // summary 消息存在
    expect(state.messages.some((m) => m.content.includes('开始复刻 3 帧'))).toBe(true);
    expect(state.messages.some((m) => m.content.includes('复刻完成'))).toBe(true);
  });

  it('recreateFrames 中途失败:已抽到的 asset 进 store,isRunning=false,error 被设', async () => {
    const frames = [makeFrame('a', 0), makeFrame('b', 1000)];

    runAgentMock
      .mockResolvedValueOnce(
        makeRunResponse({
          assets: [
            {
              type: 'image',
              url: 'https://new/1.jpg',
              prompt: 'frame a',
              tool: 'generate_image',
              tokensCost: 5,
            },
          ],
        }),
      )
      .mockRejectedValueOnce(new Error('mock fail'));

    await useAgentStore.getState().recreateFrames({
      levelId: 'L7',
      systemPrompt: 'sys',
      frames,
      tools: ['generate_image'],
    });

    const state = useAgentStore.getState();
    expect(state.assets).toHaveLength(1); // 第一张进了
    expect(state.assets[0].sourceAssetUrl).toBe('data:image/jpeg;base64,a');
    expect(state.isRunning).toBe(false);
    expect(state.recreateProgress).toBeNull();
    expect(state.error).toContain('mock fail');
  });

  it('recreateFrames 不污染 user 消息流(不被 send() 那样插入 user input)', async () => {
    const frames = [makeFrame('a', 0)];
    runAgentMock.mockResolvedValue(
      makeRunResponse({
        assets: [
          {
            type: 'image',
            url: 'https://new/1.jpg',
            prompt: 'frame a',
            tool: 'generate_image',
            tokensCost: 5,
          },
        ],
      }),
    );

    await useAgentStore.getState().recreateFrames({
      levelId: 'L7',
      systemPrompt: 'sys',
      frames,
      tools: ['generate_image'],
    });

    const state = useAgentStore.getState();
    // 不应有 role=user 的消息（recreate 不注入 user input）
    expect(state.messages.some((m) => m.role === 'user')).toBe(false);
    // 只应有 system summary
    expect(
      state.messages.every((m) => m.role === 'system' || m.role === 'tool'),
    ).toBe(true);
  });
});
