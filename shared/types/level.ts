// 关卡数据模型

export type AgeTier = 1 | 2 | 3;
export type StepType =
  | 'input'
  | 'choice'
  | 'action'
  | 'free'
  // W3.7+ 拉片复刻
  | 'reference_setup'
  | 'reference_recreate';
export type ToolName =
  | 'generate_image'
  | 'generate_image_hd'
  | 'image_to_video'
  | 'synthesize_speech'
  | 'clone_voice'
  | 'add_subtitle'
  | 'add_bgm'
  | 'text_chat';

/// W3.7+ reference_recreate 步骤专属 — UI 用哪种模式：
///   - 'single'：单选 1 帧精修（L6 拉一帧复刻）
///   - 'batch'：整段抽帧后统一复刻（L7 整段分镜复刻）
export type ReferenceRecreateMode = 'single' | 'batch';

export interface LevelStep {
  id: string;
  orderNum: number;
  title: string;
  instruction: string;
  type: StepType;
  placeholder?: string;
  options?: string[];
  hint?: string;
  /// W3.7+ 当 type='reference_recreate' 时告诉 UI 用哪种模式
  mode?: ReferenceRecreateMode;
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
