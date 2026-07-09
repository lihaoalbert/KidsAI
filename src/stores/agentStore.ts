// Agent 状态：当前会话、消息流、运行中、事件订阅
// W2.4: 订阅 Tauri agent://event 事件流，实时更新 UI
// W2.6: 收集 generated assets
// W3.2: chunk 流式累积到 assistant 消息；cancel 动作
// W3.3: streaming 槽位 + 取消事件 + Started 事件回填 sessionId

import { create } from 'zustand';
import {
  runAgent,
  cancelAgent,
  onAgentEvent,
  type AgentEvent,
  type AgentAsset,
  type AgentRunResponse,
} from '../api/tauri';

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

  // actions
  startSession: (levelId: string) => void;
  appendMessage: (msg: Omit<AgentMessage, 'id' | 'createdAt'>) => void;
  send: (levelId: string, userInput: string, systemPrompt: string) => Promise<void>;
  cancel: () => Promise<void>;
  reset: () => void;
  subscribeEvents: () => Promise<void>;
  unsubscribeEvents: () => void;
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
      const resp = await runAgent({
        levelId,
        userInput,
        systemPrompt,
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
          set((s) => ({
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
              })),
            ],
          }));
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
}));