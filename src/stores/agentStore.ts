// Agent 状态：当前会话、消息流、运行中、事件订阅
// W2.4: 订阅 Tauri agent://event 事件流，实时更新 UI
// W2.6: 收集 generated assets
// W3.2: chunk 流式累积到 assistant 消息；cancel 动作
// W3.3: streaming 槽位 + 取消事件 + Started 事件回填 sessionId
// W3.4: 角色一致性 — 选一个角色，同一会话多次生图保持形象稳定
// W3.5: 指哪打哪 — 点击图片 → 右侧抽屉 → 输入修改意图 → 生成新图（原图保留为缩略）
// W3.6: 风格模板切换 — 选一种视觉风格，同一会话生成的图片共享同一视觉语言
// W3.7+: 拉片复刻 — 抽帧 → 顺序 runAgent → patch sourceAssetUrl 形成编辑链

import { create } from 'zustand';
import {
  runAgent,
  cancelAgent,
  onAgentEvent,
  listCharacters,
  listStyles,
  type AgentEvent,
  type AgentAsset,
  type AgentRunResponse,
  type Character,
  type StylePreset,
} from '../api/tauri';
import type { ExtractedFrame } from '../utils/frameExtractor';

export interface AgentMessage {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool' | 'thought';
  content: string;
  createdAt: number;
  meta?: {
    toolName?: string;
    step?: number;
    assetUrl?: string;
    assetType?: 'image' | 'video' | 'audio';
  };
}

interface StreamingSlot {
  messageId: string;
  step: number;
}

interface AgentState {
  // 当前会话
  sessionId: string | null;
  levelId: string | null;

  // 消息流
  messages: AgentMessage[];

  // 生成资产
  assets: AgentAsset[];

  // W3.3: 流式累积槽位 — 当前正在 streaming 的 assistant 消息
  streaming: StreamingSlot | null;

  // UI
  isRunning: boolean;
  lastResponse: AgentRunResponse | null;
  error: string | null;
  unlisten: (() => void) | null;

  // W3.4: 角色一致性 — 当前选中的角色 + 内置角色列表
  characters: Character[];
  character: Character | null;

  // W3.6: 风格模板切换 — 当前选中的风格 + 内置风格列表
  styles: StylePreset[];
  style: StylePreset | null;

  // W3.5: 指哪打哪 — 当前正在编辑的资产 + 归一化点击坐标 (0-1)
  editing: {
    asset: AgentAsset;
    clickX: number; // 0-1 of image width
    clickY: number; // 0-1 of image height
  } | null;

  // W3.7+: 拉片复刻 — 已抽好的参考帧(L6 / L7)
  extractedFrames: ExtractedFrame[];

  // W3.7+: 整段复刻进度(L7 batch 模式)
  recreateProgress: { done: number; total: number } | null;

  // actions
  startSession: (levelId: string) => void;
  appendMessage: (msg: Omit<AgentMessage, 'id' | 'createdAt'>) => void;
  send: (levelId: string, userInput: string, systemPrompt: string) => Promise<void>;
  cancel: () => Promise<void>;
  reset: () => void;
  subscribeEvents: () => Promise<void>;
  unsubscribeEvents: () => void;
  /// W3.4: 加载内置角色清单（页面初始化时调一次）
  loadCharacters: () => Promise<void>;
  /// W3.4: 设定当前 session 角色（send 之前）；null 表示不绑定角色（向后兼容）
  setCharacter: (c: Character | null) => void;
  /// W3.6: 加载内置风格清单
  loadStyles: () => Promise<void>;
  /// W3.6: 设定当前 session 风格；null 表示不绑定风格
  setStyle: (s: StylePreset | null) => void;
  /// W3.5: 打开右侧编辑抽屉，记录被点击的资产 + 归一化坐标
  setEditing: (asset: AgentAsset, clickX: number, clickY: number) => void;
  /// W3.5: 关闭编辑抽屉
  clearEditing: () => void;
  /// W3.5: 提交编辑 — 把「(x,y) 修改：xxx」+ 源图信息拼成 runAgent 请求
  editImageAsset: (params: {
    levelId: string;
    systemPrompt: string;
    prompt: string;
    tools: string[];
  }) => Promise<void>;
  /// W3.7+: 设置已抽好的参考帧(ReferenceVideoPicker onChange)
  setExtractedFrames: (frames: ExtractedFrame[]) => void;
  /// W3.7+: 清空抽好的帧(关卡切换 / 重置)
  clearExtractedFrames: () => void;
  /// W3.7+: 设置整段复刻进度
  setRecreateProgress: (p: { done: number; total: number } | null) => void;
  /// W3.7+: 顺序抽帧 + 顺序 runAgent,每张新 asset 自动 patch sourceAssetUrl = frame.dataUrl
  /// 不污染 chat 流(不插入 user/assistant),只发 summary system 消息
  recreateFrames: (params: {
    levelId: string;
    systemPrompt: string;
    frames: ExtractedFrame[];
    characterId?: string;
    styleId?: string;
    tools: string[];
  }) => Promise<void>;
}

function nowId(prefix: string) {
  return `msg_${Date.now()}_${prefix}_${Math.random().toString(36).slice(2, 7)}`;
}

export const useAgentStore = create<AgentState>((set, get) => ({
  sessionId: null,
  levelId: null,
  messages: [],
  assets: [],
  streaming: null,
  isRunning: false,
  lastResponse: null,
  error: null,
  unlisten: null,
  // W3.4: 角色状态初始值
  characters: [],
  character: null,
  // W3.6: 风格状态初始值
  styles: [],
  style: null,
  // W3.5: 编辑抽屉初始关闭
  editing: null,
  // W3.7+: 抽好的帧初始空
  extractedFrames: [],
  recreateProgress: null,

  startSession: (levelId) => {
    set({
      sessionId: `sess_${Date.now()}`,
      levelId,
      messages: [],
      assets: [],
      streaming: null,
      lastResponse: null,
      error: null,
    });
  },

  appendMessage: (msg) =>
    set((s) => ({
      messages: [
        ...s.messages,
        {
          ...msg,
          id: nowId(msg.role),
          createdAt: Date.now(),
        },
      ],
    })),

  send: async (levelId, userInput, systemPrompt) => {
    set({ isRunning: true, error: null });
    set((s) => ({
      sessionId: s.sessionId ?? `sess_${Date.now()}`,
      levelId,
      messages: [
        ...s.messages,
        {
          id: nowId('u'),
          role: 'user',
          content: userInput,
          createdAt: Date.now(),
        },
      ],
    }));
    // 订阅事件（如果还没订阅）
    if (!get().unlisten) {
      await get().subscribeEvents();
    }
    try {
      // W3.4 + W3.6: 当前角色 + 风格 两个独立维度，分别传给 backend；没设就不传
      const characterId = get().character?.id;
      const styleId = get().style?.id;
      const resp = await runAgent({
        levelId,
        userInput,
        systemPrompt,
        characterId,
        styleId,
      });
      // 如果已经 streaming 但还没收到 final_answer 事件（mock / 错误路径），
      // 这里用 lastResponse.finalAnswer 兜底写入
      set((s) => {
        const messages = s.streaming
          ? s.messages.map((m) =>
              m.id === s.streaming!.messageId
                ? { ...m, content: resp.finalAnswer || m.content }
                : m,
            )
          : [
              ...s.messages,
              {
                id: nowId('a'),
                role: 'assistant' as const,
                content: resp.finalAnswer,
                createdAt: Date.now(),
              },
            ];
        return {
          isRunning: false,
          lastResponse: resp,
          messages,
          assets: resp.assets,
          streaming: null,
        };
      });
    } catch (e) {
      set({ isRunning: false, error: String(e), streaming: null });
    }
  },

  cancel: async () => {
    const { sessionId, isRunning } = get();
    if (!isRunning || !sessionId) return;
    try {
      await cancelAgent(sessionId);
    } catch (e) {
      // 取消失败也允许 UI 端兜底（用户已点取消）
      set({ error: String(e) });
    }
  },

  reset: () => {
    get().unsubscribeEvents();
    set({
      sessionId: null,
      levelId: null,
      messages: [],
      assets: [],
      streaming: null,
      isRunning: false,
      lastResponse: null,
      error: null,
      // W3.7+: 跨关清掉抽帧状态
      extractedFrames: [],
      recreateProgress: null,
    });
  },

  subscribeEvents: async () => {
    if (get().unlisten) return;
    const unlisten = await onAgentEvent((evt: AgentEvent) => {
      switch (evt.kind) {
        case 'started': {
          // W3.3: server-side 真实 sessionId 回填（前端预设的 sess_xxx 不可靠）
          set({ sessionId: evt.sessionId });
          break;
        }
        case 'thought': {
          set((s) => ({
            messages: [
              ...s.messages,
              {
                id: nowId(`t${evt.step}`),
                role: 'thought',
                content: evt.thought,
                createdAt: Date.now(),
                meta: { step: evt.step },
              },
            ],
          }));
          break;
        }
        case 'tool_call': {
          set((s) => ({
            messages: [
              ...s.messages,
              {
                id: nowId(`tc${evt.step}`),
                role: 'system',
                content: `🔧 调用工具：${evt.tool}`,
                createdAt: Date.now(),
                meta: { toolName: evt.tool, step: evt.step },
              },
            ],
          }));
          break;
        }
        case 'tool_result': {
          // W3.5: 如果这次 tool_result 是 edit_image 工具的结果，且编辑抽屉是开的，
          // 把新生成的 asset 标记 sourceAssetUrl = editing.asset.url（前端 metadata）
          // 并自动关掉抽屉。其他工具的结果不受影响。
          set((s) => {
            const isEdit = evt.tool === 'edit_image' && s.editing !== null;
            return {
              messages: [
                ...s.messages,
                {
                  id: nowId(`tr${evt.step}`),
                  role: 'tool',
                  content: evt.result,
                  createdAt: Date.now(),
                  meta: { toolName: evt.tool, step: evt.step },
                },
              ],
              assets: [
                ...s.assets,
                ...evt.assets.map((a) => ({
                  ...a,
                  type: a.type as 'image' | 'video' | 'audio',
                  sourceAssetUrl: isEdit ? s.editing!.asset.url : undefined,
                })),
              ],
              editing: isEdit ? null : s.editing,
            };
          });
          break;
        }
        case 'chunk': {
          // W3.3: 流式累积到一条 assistant 消息
          set((s) => {
            if (s.streaming && s.streaming.step === evt.step) {
              // 同一 step 的后续 chunk：append 到现有消息
              return {
                messages: s.messages.map((m) =>
                  m.id === s.streaming!.messageId
                    ? { ...m, content: m.content + evt.delta }
                    : m,
                ),
              };
            }
            // 新 step / 首次 chunk：创建新 assistant 消息 + 设置 streaming 槽位
            const newId = nowId(`s${evt.step}`);
            return {
              messages: [
                ...s.messages,
                {
                  id: newId,
                  role: 'assistant',
                  content: evt.delta,
                  createdAt: Date.now(),
                  meta: { step: evt.step },
                },
              ],
              streaming: { messageId: newId, step: evt.step },
            };
          });
          break;
        }
        case 'final_answer': {
          // 用 final_answer 替换 streaming 槽位的内容（更干净，无 chunk 残留）
          set((s) => {
            if (!s.streaming) return s;
            return {
              messages: s.messages.map((m) =>
                m.id === s.streaming!.messageId
                  ? { ...m, content: evt.answer }
                  : m,
              ),
              streaming: null,
            };
          });
          break;
        }
        case 'cancelled': {
          // W3.3: 中途取消 — 设置 error + 清空 streaming
          set({ error: '已取消', isRunning: false, streaming: null });
          break;
        }
        case 'error': {
          set({ error: evt.message, isRunning: false, streaming: null });
          break;
        }
        // 'done' 不需要显示
        default:
          break;
      }
    });
    set({ unlisten });
  },

  unsubscribeEvents: () => {
    const { unlisten } = get();
    if (unlisten) {
      unlisten();
      set({ unlisten: null });
    }
  },

  loadCharacters: async () => {
    // 已有数据就不重复拉（避免每次页面切换都打 backend）
    if (get().characters.length > 0) return;
    try {
      const list = await listCharacters();
      set({ characters: list });
    } catch (e) {
      // 失败不阻塞 — character 是可选特性，旧后端 / mock 环境也能跑
      console.warn('loadCharacters failed:', e);
      set({ characters: [] });
    }
  },

  setCharacter: (c) => set({ character: c }),

  loadStyles: async () => {
    if (get().styles.length > 0) return;
    try {
      const list = await listStyles();
      set({ styles: list });
    } catch (e) {
      // 失败不阻塞 — style 是可选特性
      console.warn('loadStyles failed:', e);
      set({ styles: [] });
    }
  },

  setStyle: (s) => set({ style: s }),

  // W3.5: 打开 / 关闭编辑抽屉
  setEditing: (asset, clickX, clickY) =>
    set({ editing: { asset, clickX, clickY } }),
  clearEditing: () => set({ editing: null }),

  // W3.7+: 拉片复刻 state 操作
  setExtractedFrames: (frames) => set({ extractedFrames: frames }),
  clearExtractedFrames: () => set({ extractedFrames: [] }),
  setRecreateProgress: (p) => set({ recreateProgress: p }),

  // W3.5: 提交编辑 — 把 (x,y) + 修改意图 + 源图 URL 拼到 system_prompt，
  // 然后调 runAgent。后端 model 看到 [Editor context] 段会决定调 edit_image。
  editImageAsset: async ({ levelId, systemPrompt, prompt, tools }) => {
    const editing = get().editing;
    if (!editing) {
      console.warn('editImageAsset called without editing context');
      return;
    }

    // 把坐标 + 源图 + 用户意图拼成 system_prompt 末尾的 [Editor context] 段
    // 模型（mock 或真 LLM）都能看到完整信息，便于决定调 edit_image
    const xPercent = Math.round(editing.clickX * 100);
    const yPercent = Math.round(editing.clickY * 100);
    const ctx = `\n\n[Editor context]\nsource_image_url=${editing.asset.url}\nclick_x=${xPercent}%\nclick_y=${yPercent}%\nuser_intent=${prompt}\n请用 edit_image 工具修改这张图。`;
    const augmentedPrompt = `${systemPrompt}${ctx}`;

    // 自动把 edit_image 加到 tools 列表（如果不在）
    const finalTools = tools.includes('edit_image')
      ? tools
      : [...tools, 'edit_image'];

    set({ isRunning: true, error: null });
    if (!get().unlisten) {
      await get().subscribeEvents();
    }
    try {
      const characterId = get().character?.id;
      const styleId = get().style?.id;
      await runAgent({
        levelId,
        userInput: prompt,
        systemPrompt: augmentedPrompt,
        tools: finalTools,
        characterId,
        styleId,
      });
      // 注意：editing 会在 tool_result 事件到达时被自动清掉（如果工具是 edit_image）
      // 这里不主动 clearEditing，保留容错：如果工具不对应 edit_image，editing 会保留
      // 由用户手动取消（防止无限标记 sourceAssetUrl）
      set({ isRunning: false });
    } catch (e) {
      set({ isRunning: false, error: String(e) });
    }
  },

  // W3.7+: 顺序跑 N 个 runAgent,每个拼 [Reference context],新 asset 标记 sourceAssetUrl
  // 与 send() 不同:不注入 user/assistant 消息(避免 chat 流被 5 个 user 灌满),
  // 只插一条 batch summary + 每张新 asset 进入 store.assets
  recreateFrames: async ({ levelId, systemPrompt, frames, characterId, styleId, tools }) => {
    set({
      isRunning: true,
      error: null,
      recreateProgress: { done: 0, total: frames.length },
    });

    // 启动 summary
    get().appendMessage({
      role: 'system',
      content: `🎞️ 开始复刻 ${frames.length} 帧…`,
    });

    if (!get().unlisten) {
      await get().subscribeEvents();
    }

    const aggregated: AgentAsset[] = [];
    try {
      for (let i = 0; i < frames.length; i++) {
        const frame = frames[i];
        const tsSec = (frame.timestampMs / 1000).toFixed(1);
        const augmented = `${systemPrompt}\n\n[Reference context]\nsource_image_url=${frame.dataUrl}\ntimestamp=${tsSec}s\nuser_intent=按当前角色+风格复刻这一帧,保留原构图\n`;

        const resp = await runAgent({
          levelId,
          userInput: `复刻第 ${i + 1} / ${frames.length} 帧`,
          systemPrompt: augmented,
          tools,
          characterId,
          styleId,
        });

        // 每张返回的 asset 自动 patch sourceAssetUrl,把链传给 W3.5 编辑抽屉
        const newAssets = resp.assets.map((a) => ({
          ...a,
          sourceAssetUrl: frame.dataUrl,
        }));
        aggregated.push(...newAssets);

        set((s) => ({
          assets: [...s.assets, ...newAssets],
          recreateProgress: { done: i + 1, total: frames.length },
        }));
      }
      get().appendMessage({
        role: 'system',
        content: `✅ 复刻完成:共 ${aggregated.length} 张图`,
      });
      set({ isRunning: false, recreateProgress: null });
    } catch (e) {
      set({
        isRunning: false,
        error: String(e),
        recreateProgress: null,
      });
      get().appendMessage({
        role: 'system',
        content: `❌ 复刻中断:${String(e)}`,
      });
    }
  },
}));