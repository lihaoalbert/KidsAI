// 关卡数据模型

export type AgeTier = 1 | 2 | 3;
export type StepType = 'input' | 'choice' | 'action' | 'free';
export type ToolName =
  | 'generate_image'
  | 'generate_image_hd'
  | 'image_to_video'
  | 'synthesize_speech'
  | 'clone_voice'
  | 'add_subtitle'
  | 'add_bgm'
  | 'text_chat';

export interface LevelStep {
  id: string;
  orderNum: number;
  title: string;
  instruction: string;
  type: StepType;
  placeholder?: string;
  options?: string[];
  hint?: string;
}

export interface ScoringCriteria {
  creativity: number;
  technical: number;
  narrative: number;
  aesthetic: number;
  compliance: number;
}

export interface Level {
  id: string;
  orderNum: number;
  title: string;
  description: string;
  coverEmoji: string;
  estimatedMinutes: number;
  rewardTokens: number;
  difficulty: 1 | 2 | 3 | 4 | 5;
  prerequisites: string[];

  // 关卡内容
  videoSubtitle?: string;
  steps: LevelStep[];

  // AI 助教配置
  aiName: string;
  aiAvatar: string;
  systemPrompt: string;

  // 工具白名单
  tools: ToolName[];

  // 评分标准（5 维权重和=100）
  scoringCriteria: ScoringCriteria;
}

export interface LevelProgress {
  levelId: string;
  status: 'locked' | 'available' | 'in_progress' | 'completed';
  attempts: number;
  bestScore?: number;
  completedAt?: number;
}

export interface LevelSubmission {
  id: string;
  levelId: string;
  userInput: string;
  agentOutput: AgentOutput;
  score: number;
  rubricScores: ScoringCriteria;
  feedback: string;
  createdAt: number;
}

export interface AgentOutput {
  thoughts: string[];
  toolCalls: ToolCallRecord[];
  finalAnswer: string;
  generatedAssets: GeneratedAsset[];
}

export interface ToolCallRecord {
  tool: ToolName;
  args: Record<string, any>;
  result: string;
  timestamp: number;
}

export interface GeneratedAsset {
  type: 'image' | 'video' | 'audio';
  url: string;
  thumbnailUrl?: string;
  prompt: string;
  tool: ToolName;
  tokensCost: number;
}
