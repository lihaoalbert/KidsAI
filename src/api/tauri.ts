// Tauri 命令的 TypeScript 封装
// 所有跨进程调用都集中在这里，方便 mock 和测试

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Level, LevelProgress, ScoringCriteria } from '../../shared/types/level';

// ============ 关卡 ============

export async function listLevels(): Promise<Level[]> {
  return invoke<Level[]>('list_levels');
}

export async function getLevel(id: string): Promise<Level | null> {
  return invoke<Level | null>('get_level', { id });
}

export async function listProgress(): Promise<LevelProgress[]> {
  return invoke<LevelProgress[]>('list_progress');
}

export async function startLevel(id: string): Promise<LevelProgress> {
  return invoke<LevelProgress>('start_level', { id });
}

export async function submitLevel(
  levelId: string,
  score: number,
  rubric: ScoringCriteria,
  feedback: string,
): Promise<LevelProgress> {
  return invoke<LevelProgress>('submit_level', {
    levelId,
    score,
    rubric,
    feedback,
  });
}

export async function completedLevelIds(): Promise<string[]> {
  return invoke<string[]>('completed_level_ids');
}

// ============ 作品 (W2.3) ============

export interface AssetInput {
  type: 'image' | 'video' | 'audio';
  url: string;
  thumbnailUrl?: string;
  prompt: string;
  tool: string;
  tokensCost: number;
}

export interface SaveCreationRequest {
  id: string;
  levelId: string;
  userInput: string;
  agentOutput: Record<string, unknown>;
  score?: number;
  rubric?: ScoringCriteria;
  feedback?: string;
  assets: AssetInput[];
}

export interface CreationWithAssets {
  id: string;
  levelId: string;
  userInput: string;
  agentOutput: string; // JSON string from DB
  score: number | null;
  rubric: string | null;
  feedback: string | null;
  createdAt: number;
  assets: Array<{
    kind: string;
    url: string;
    thumbnailUrl: string | null;
    prompt: string;
    tool: string;
    tokensCost: number;
  }>;
}

export async function saveCreation(
  request: SaveCreationRequest,
): Promise<void> {
  return invoke<void>('save_creation', { request });
}

export async function listCreations(
  levelId?: string,
): Promise<CreationWithAssets[]> {
  return invoke<CreationWithAssets[]>('list_creations', { levelId });
}

// ============ Agent ============

export interface AgentRunRequest {
  levelId: string;
  userInput: string;
  systemPrompt: string;
  tools?: string[];
}

export interface AgentRunResponse {
  sessionId: string;
  levelId: string;
  finalAnswer: string;
  thoughts: string[];
  toolCalls: Array<{
    tool: string;
    args: Record<string, unknown>;
    result: string;
    timestamp: number;
  }>;
  assets: Array<{
    type: 'image' | 'video' | 'audio';
    url: string;
    thumbnailUrl?: string;
    prompt: string;
    tool: string;
    tokensCost: number;
  }>;
  durationMs: number;
  // W3.2: 真实 LLM token 累计 + 取消标记
  tokensUsed?: number;
  cancelled?: boolean;
}

export async function runAgent(
  request: AgentRunRequest,
): Promise<AgentRunResponse> {
  return invoke<AgentRunResponse>('run_agent', { request });
}

// W3.2: 中途取消正在运行的 agent 循环
export async function cancelAgent(sessionId: string): Promise<boolean> {
  return invoke<boolean>('cancel_agent', { sessionId });
}

// ============ 系统命令 ============

export async function getAppVersion(): Promise<string> {
  return invoke<string>('get_app_version');
}

export async function greet(name: string): Promise<string> {
  return invoke<string>('greet', { name });
}

// ============ 安全审核 (W2.7) ============

export type SafetyVerdict =
  | 'pass'
  | { warn: { reason: string } }
  | { block: { reason: string } };

export async function checkSafety(text: string): Promise<SafetyVerdict> {
  // Rust 返回的是枚举 tagged union（snake_case 自动），前端会收到
  // { "pass": null } | { "warn": { "reason": "..." } } | { "block": { "reason": "..." } }
  return invoke<SafetyVerdict>('check_safety', { text });
}

// ============ Agent 事件流 (W2.4 + W3.2 流式 + 取消) ============

export type AgentEvent =
  | { kind: 'started'; sessionId: string }
  | { kind: 'thought'; sessionId: string; step: number; thought: string }
  | { kind: 'tool_call'; sessionId: string; step: number; tool: string; args: unknown }
  | { kind: 'tool_result'; sessionId: string; step: number; tool: string; result: string; assets: AgentAsset[] }
  // W3.2: 流式 — 每个 delta 一条事件，前端累积到一条 assistant 消息
  | { kind: 'chunk'; sessionId: string; step: number; delta: string }
  // W3.2: 取消 — 收到这条后立即停止生成
  | { kind: 'cancelled'; sessionId: string }
  | { kind: 'final_answer'; sessionId: string; answer: string }
  | { kind: 'done'; sessionId: string; steps: number; durationMs: number }
  | { kind: 'error'; sessionId: string; message: string };

export interface AgentAsset {
  type: 'image' | 'video' | 'audio';
  url: string;
  thumbnailUrl?: string;
  prompt: string;
  tool: string;
  tokensCost: number;
}

export async function onAgentEvent(
  handler: (event: AgentEvent) => void,
): Promise<UnlistenFn> {
  return listen<AgentEvent>('agent://event', (e) => handler(e.payload));
}
