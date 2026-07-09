// Agent 状态：当前会话、消息流、运行中、事件订阅
// W2.4: 订阅 Tauri agent://event 事件流，实时更新 UI
// W2.6: 收集 generated assets

import { create } from 'zustand';
import {
  runAgent,
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

interface AgentState {
  // 当前会话
  sessionId: string | null;
  levelId: string | null;

  // 消息流
  messages: AgentMessage[];

  // 生成资产
  assets: AgentAsset[];

  // UI
  isRunning: boolean;
  lastResponse: AgentRunResponse | null;
  error: string | null;
  unlisten: (() => void) | null;

  // actions
  startSession: (levelId: string) => void;
  appendMessage: (msg: Omit<AgentMessage, 'id' | 'createdAt'>) => void;
  send: (levelId: string, userInput: string, systemPrompt: string) => Promise<void>;
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
      set((s) => ({
        isRunning: false,
        lastResponse: resp,
        messages: [
          ...s.messages,
          {
            id: nowId('a'),
            role: 'assistant',
            content: resp.finalAnswer,
            createdAt: Date.now(),
          },
        ],
        assets: resp.assets,
      }));
    } catch (e) {
      set({ isRunning: false, error: String(e) });
    }
  },

  reset: () => {
    get().unsubscribeEvents();
    set({
      sessionId: null,
      levelId: null,
      messages: [],
      assets: [],
      isRunning: false,
      lastResponse: null,
      error: null,
    });
  },

  subscribeEvents: async () => {
    if (get().unlisten) return;
    const unlisten = await onAgentEvent((evt: AgentEvent) => {
      switch (evt.kind) {
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
        case 'final_answer': {
          // run_agent 的响应也会 push 一条；这里只更新，不重复加
          break;
        }
        case 'error': {
          set({ error: evt.message });
          break;
        }
        // 'started' / 'done' 不需要显示
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
