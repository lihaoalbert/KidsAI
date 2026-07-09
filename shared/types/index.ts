// 前后端共享类型定义

export type AgeTier = 1 | 2 | 3; // 8-10 / 11-13 / 14-16

export interface User {
  id: string;
  nickname: string;
  avatar: string;
  ageTier: AgeTier;
  createdAt: number;
}

export interface Level {
  id: string;
  courseLine: 'game' | 'video' | 'agent';
  tier: AgeTier;
  orderNum: number;
  title: string;
  description: string;
  estimatedMinutes: number;
  rewardTokens: number;
  prerequisites: string[];
}

export interface LevelProgress {
  levelId: string;
  status: 'locked' | 'available' | 'in_progress' | 'completed';
  score?: number;
  attempts: number;
  completedAt?: number;
}

export interface Creation {
  id: string;
  userId: string;
  levelId?: string;
  title: string;
  type: 'video' | 'image' | 'game' | 'agent';
  filePath: string;
  thumbnailPath?: string;
  score?: number;
  createdAt: number;
}

export interface TokenTransaction {
  id: string;
  userId: string;
  type: 'earn' | 'consume' | 'recharge' | 'refund';
  amount: number;
  direction: 'in' | 'out';
  reason: string;
  relatedId?: string;
  createdAt: number;
}

export interface ParentalControl {
  dailyTimeLimit: number; // 秒
  dailyTokenLimit: number;
  nightLockEnabled: boolean;
  nightLockStart: string; // HH:MM
  nightLockEnd: string; // HH:MM
}

// Tauri 命令调用结果
export interface CommandResult<T = void> {
  ok: boolean;
  data?: T;
  error?: string;
}
