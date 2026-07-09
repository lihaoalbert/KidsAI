// Agent 状态：当前会话、消息流、运行中
// W2.2: 同步调用占位实现
// W2.4: 改为事件流

import { create } from 'zustand';
import { runAgent, type AgentRunResponse } from '../api/tauri';

export interface AgentMessage {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
  createdAt: number;
  meta?: {
    toolName?: string;
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

  // UI
  isRunning: boolean;
  lastResponse: AgentRunResponse | null;
  error: string | null;

  // actions
  startSession: (levelId: string) => void;
  appendMessage: (msg: Omit<AgentMessage, 'id' | 'createdAt'>) => void;
  send: (levelId: string, userInput: string, systemPrompt: string) => Promise<void>;
  reset: () => void;
}

export const useAgentStore = create<AgentState>((set) => ({
  sessionId: null,
  levelId: null,
  messages: [],
  isRunning: false,
  lastResponse: null,
  error: null,

  startSession: (levelId) =>
    set({
      sessionId: `sess_${Date.now()}`,
      levelId,
      messages: [],
      lastResponse: null,
      error: null,
    }),

  appendMessage: (msg) =>
    set((s) => ({
      messages: [
        ...s.messages,
        {
          ...msg,
          id: `msg_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
          createdAt: Date.now(),
        },
      ],
    })),

  send: async (levelId, userInput, systemPrompt) => {
    set({ isRunning: true, error: null });
    // 记录用户消息
    set((s) => ({
      sessionId: s.sessionId ?? `sess_${Date.now()}`,
      levelId,
      messages: [
        ...s.messages,
        {
          id: `msg_${Date.now()}_u`,
          role: 'user',
          content: userInput,
          createdAt: Date.now(),
        },
      ],
    }));
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
            id: `msg_${Date.now()}_a`,
            role: 'assistant',
            content: resp.finalAnswer,
            createdAt: Date.now(),
          },
        ],
      }));
    } catch (e) {
      set({ isRunning: false, error: String(e) });
    }
  },

  reset: () =>
    set({
      sessionId: null,
      levelId: null,
      messages: [],
      isRunning: false,
      lastResponse: null,
      error: null,
    }),
}));
