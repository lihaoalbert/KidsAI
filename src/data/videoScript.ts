// 视频流对话剧本（内容层，非逻辑）。
// 阶段1 用声明式 beats（干净的线性问答）；阶段2–6 的文案在 STAGE_COPY 里，
// 流程逻辑由 studioStore 驱动（涉及后端候选/学分/异步生成，不适合纯声明式）。
import type { StorySlot } from '../stores/directorStore';

export type OptionKind = 'choice' | 'free' | 'stuck' | 'dice' | 'action';

export interface OptionCard {
  id: string;
  label: string;
  value: string;
  emoji?: string;
  kind?: OptionKind; // 默认 'choice'
}

export const FREE_CARD: OptionCard = {
  id: 'free',
  label: '我自己说',
  value: '__free__',
  emoji: '🎤',
  kind: 'free',
};

export const STUCK_CARD: OptionCard = {
  id: 'stuck',
  label: '我也不知道',
  value: '__stuck__',
  emoji: '🤔',
  kind: 'stuck',
};

/** 阶段1 单个问答 beat */
export interface QuestionBeat {
  id: string;
  slot?: StorySlot;
  ai: string;
  cards: OptionCard[]; // 选择卡（不含自动追加的 🎤 / 不知道）
  allowFree?: boolean; // 追加「🎤 我自己说」
  stuck?: OptionCard[]; // 点「不知道」后 AI 抛出的具体选项
  echo?: (label: string) => string; // AI 复述确认
  next: string; // 下一 beat id；'STORY_CARD' 表示进故事卡
}

export const STORY_CARD_BEAT = 'STORY_CARD';

// —— 阶段1：故事骨架四槽 ——
export const STAGE1_BEATS: QuestionBeat[] = [
  {
    id: 's1_who',
    slot: 'who',
    ai: '太好了，我们来做一部你的小电影！🎬\n先想好故事——这部电影的主角是谁呀？🌟',
    cards: [
      { id: 'dragon', label: '会喷火的小恐龙', value: '一只会喷火的小恐龙', emoji: '🐉' },
      { id: 'cat', label: '会魔法的猫', value: '一只会魔法的猫', emoji: '🐱' },
      { id: 'robot', label: '爱冒险的机器人', value: '一个爱冒险的机器人', emoji: '🤖' },
    ],
    allowFree: true,
    stuck: [
      { id: 'dragon', label: '会喷火的小恐龙', value: '一只会喷火的小恐龙', emoji: '🐉' },
      { id: 'unicorn', label: '独角兽', value: '一只闪闪发光的独角兽', emoji: '🦄' },
      { id: 'astronaut', label: '小宇航员', value: '一个勇敢的小宇航员', emoji: '👩‍🚀' },
    ],
    echo: (label) => `${label}，好棒的主角！🌟`,
    next: 's1_wants',
  },
  {
    id: 's1_wants',
    slot: 'wants',
    ai: '它最想做的一件事是什么？有目标故事才好看！🔥',
    cards: [
      { id: 'find', label: '找回丢失的东西', value: '找回丢失的宝贝', emoji: '🔍' },
      { id: 'beat', label: '打败大坏蛋', value: '打败大坏蛋', emoji: '⚔️' },
      { id: 'friend', label: '交到新朋友', value: '交到新朋友', emoji: '🤝' },
    ],
    allowFree: true,
    stuck: [
      { id: 'find', label: '找回丢失的东西', value: '找回丢失的宝贝', emoji: '🔍' },
      { id: 'save', label: '拯救伙伴', value: '拯救被困的伙伴', emoji: '🦸' },
      { id: 'treasure', label: '寻找宝藏', value: '找到传说中的宝藏', emoji: '💎' },
    ],
    echo: (label) => `想要「${label}」，这个目标很酷！🔥`,
    next: 's1_but',
  },
  {
    id: 's1_but',
    slot: 'but',
    ai: '可是路上会遇到什么麻烦呢？有一点点困难，主角会更厉害哦！😲',
    cards: [
      { id: 'ice', label: '被大冰山挡住', value: '被一座大冰山挡住了去路', emoji: '⛰️' },
      { id: 'monster', label: '遇到凶怪兽', value: '遇到一只凶巴巴的怪兽', emoji: '👹' },
      { id: 'lost', label: '迷路了', value: '在半路上迷路了', emoji: '🌫️' },
      { id: 'smooth', label: '一路顺顺利利', value: '一路上顺顺利利', emoji: '🌈' },
    ],
    allowFree: true,
    stuck: [
      { id: 'ice', label: '被大冰山挡住', value: '被一座大冰山挡住了去路', emoji: '⛰️' },
      { id: 'storm', label: '遇到大风暴', value: '遇到一场大风暴', emoji: '🌪️' },
      { id: 'trap', label: '掉进陷阱', value: '不小心掉进了陷阱', emoji: '🕳️' },
    ],
    echo: (label) => `「${label}」，这下更精彩了！😲`,
    next: 's1_ending',
  },
  {
    id: 's1_ending',
    slot: 'ending',
    ai: '最后你希望是哪种结尾？💫',
    cards: [
      { id: 'warm', label: '暖心的', value: '交到朋友，一起开心地过关', emoji: '🤗' },
      { id: 'funny', label: '搞笑的', value: '用搞笑的办法赢了，大家哈哈大笑', emoji: '😆' },
      { id: 'epic', label: '超酷大冒险', value: '经过一场超酷的大冒险，成功啦', emoji: '🚀' },
    ],
    allowFree: true,
    stuck: [
      { id: 'warm', label: '暖心的', value: '交到朋友，一起开心地过关', emoji: '🤗' },
      { id: 'brave', label: '勇敢取胜', value: '鼓起勇气，成功战胜了困难', emoji: '🦁' },
    ],
    echo: (label) => `${label}的结尾，我喜欢！💫`,
    next: STORY_CARD_BEAT,
  },
];

export function getStage1Beat(id: string): QuestionBeat | undefined {
  return STAGE1_BEATS.find((b) => b.id === id);
}

/** 随机故事（🎲 一键顺流用） */
export const DICE_STORIES = [
  { who: '一只会喷火的小恐龙', wants: '找回丢失的火焰', but: '被一座大冰山挡住了去路', ending: '交到朋友，一起闯过去' },
  { who: '一只会魔法的猫', wants: '找到传说中的月亮鱼', but: '遇到一场大风暴', ending: '用魔法化解危机，大家哈哈大笑' },
  { who: '一个爱冒险的机器人', wants: '寻找失落的能量核心', but: '在迷宫里迷路了', ending: '经过一场超酷的大冒险，成功啦' },
];

// —— 阶段2–6 文案（流程逻辑在 studioStore）——
export const STAGE_COPY = {
  planLoading: '让我把这个故事变成小电影的方案…✨',
  planReady: '方案好啦！我给主角、画风、分镜都准备了一版，我们一起看看～',
  // 阶段2 主角
  character:
    '这就是我们的主角！我给它拍了张"标准照"📸\n喜欢就点确认，也可以在右边给它换换颜色、大小、表情～',
  characterConfirmed: (name: string) => `主角就定成 ${name} 啦！🌟`,
  // 阶段3 画风
  style: '给电影选个画风吧🎨 你喜欢哪一种感觉？',
  styleConfirmed: (name: string) => `画风就用「${name}」！整部电影都会是这个味道～`,
  // 阶段4 分镜
  storyboard:
    '我把故事分成了几个小片段（分镜），右边能看到它们连起来的样子🎞️\n看看顺序合适吗？',
  storyboardConfirmed: '分镜排好啦！接下来我们一段一段拍出来～',
  // 阶段5 试拍
  preview: (n: number, credits: number, balance: number) =>
    `准备拍第 ${n} 段！这一镜要花 ${credits} 学分（你还有 ${balance}）。开始吗？`,
  previewDone: '这一段拍好啦！右边可以看，也能微调速度、音效、滤镜～',
  // 阶段6 定稿
  finalize: (credits: number, balance: number) =>
    `全部片段都好啦！要不要合成一部完整的电影？定稿要花 ${credits} 学分（你还有 ${balance}）。`,
  finalizeDone: '🎉 你的电影完成啦！要把它放进作品墙吗？',
} as const;
