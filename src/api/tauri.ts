// Tauri 命令的 TypeScript 封装
// 所有跨进程调用都集中在这里，方便 mock 和测试

import { invoke } from '@tauri-apps/api/core';
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
}

export async function runAgent(
  request: AgentRunRequest,
): Promise<AgentRunResponse> {
  return invoke<AgentRunResponse>('run_agent', { request });
}

// ============ 系统命令 ============

export async function getAppVersion(): Promise<string> {
  return invoke<string>('get_app_version');
}

export async function greet(name: string): Promise<string> {
  return invoke<string>('greet', { name });
}
