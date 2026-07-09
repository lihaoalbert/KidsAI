// 关卡静态数据 - MVP 阶段 5 个关卡
import type { Level } from '../../shared/types/level';

export const LEVELS: Level[] = [
  {
    id: 'L1',
    orderNum: 1,
    title: '我的第一个 AI 视频',
    description: '一句话生成 5 秒视频，认识 AI 视频创作',
    coverEmoji: '🎬',
    estimatedMinutes: 20,
    rewardTokens: 30,
    difficulty: 1,
    prerequisites: [],

    videoSubtitle: '让 AI 帮我们做一件从来没做过的事',

    steps: [
      {
        id: 'L1-s1',
        orderNum: 1,
        title: '看一段示例',
        instruction: '看一段 AI 生成的 5 秒视频：小恐龙在草地上跑步。',
        type: 'action',
        hint: 'AI 可以把我们描述的画面变成真实的视频！',
      },
      {
        id: 'L1-s2',
        orderNum: 2,
        title: '输入你的描述',
        instruction: '用一句话描述你想看的画面。提示词公式：主体 + 动作 + 场景',
        type: 'input',
        placeholder: '比如：一只小猫在月光下追蝴蝶',
        hint: '描述越具体，AI 越懂你哦～',
      },
      {
        id: 'L1-s3',
        orderNum: 3,
        title: '查看生成结果',
        instruction: 'AI 老师会根据你的描述生成视频，看看你做出来的！',
        type: 'action',
      },
    ],

    aiName: '小启',
    aiAvatar: '🦉',
    systemPrompt: `你是"小启"，一个陪伴 8-10 岁孩子学习 AI 视频创作的 AI 老师。
你的形象是一只戴眼镜的猫头鹰，温暖、耐心、喜欢用象声词。
绝对不直接给答案，用问题引导孩子。
绝对不输出"宝贝/老公/老婆"等亲密称谓。
绝对不输出任何违规内容。
保持句子简短（不超过 15 字），多用 emoji。`,

    tools: ['generate_image', 'image_to_video', 'text_chat'],
    scoringCriteria: {
      creativity: 30,
      technical: 25,
      narrative: 20,
      aesthetic: 15,
      compliance: 10,
    },
  },

  {
    id: 'L2',
    orderNum: 2,
    title: '让视频有声音',
    description: '为你的视频加上配音和字幕',
    coverEmoji: '🎙️',
    estimatedMinutes: 25,
    rewardTokens: 40,
    difficulty: 1,
    prerequisites: ['L1'],

    steps: [
      {
        id: 'L2-s1',
        orderNum: 1,
        title: '看一段没有声音的视频',
        instruction: '视频只有画面，没有声音。是不是感觉少了点什么？',
        type: 'action',
      },
      {
        id: 'L2-s2',
        orderNum: 2,
        title: '为视频写一段台词',
        instruction: '在文本框输入 1-2 句台词。比如："小猫说：今天天气真好！"',
        type: 'input',
        placeholder: '小猫说：今天天气真好！',
      },
      {
        id: 'L2-s3',
        orderNum: 3,
        title: '选择声音和情绪',
        instruction: '选一种声音和一种情绪，AI 老师会按你的设置生成配音。',
        type: 'choice',
        options: ['开心', '温柔', '勇敢', '搞笑'],
      },
    ],

    aiName: '小启',
    aiAvatar: '🦉',
    systemPrompt: `你是"小启"，一个陪伴 8-10 岁孩子学习 AI 视频创作的 AI 老师。
当前任务：教孩子理解视频+音频=完整作品，体验 TTS 配音。
引导孩子选择合适的声音和情绪。`,

    tools: ['synthesize_speech', 'add_subtitle'],
    scoringCriteria: {
      creativity: 30,
      technical: 25,
      narrative: 20,
      aesthetic: 15,
      compliance: 10,
    },
  },

  {
    id: 'L3',
    orderNum: 3,
    title: '让角色"动"起来',
    description: '学习 5 要素，写出更具体的描述',
    coverEmoji: '✨',
    estimatedMinutes: 25,
    rewardTokens: 40,
    difficulty: 1,
    prerequisites: ['L2'],

    steps: [
      {
        id: 'L3-s1',
        orderNum: 1,
        title: '对比两个提示词',
        instruction: '对比"一只狗"和"白色小柴犬戴红色围巾在雪地打雪仗"生成的视频。哪个更好？',
        type: 'action',
      },
      {
        id: 'L3-s2',
        orderNum: 2,
        title: '用 5 要素写描述',
        instruction: '5 要素 = 颜色 + 品种/类型 + 装饰 + 场景 + 动作',
        type: 'input',
        placeholder: '比如：一只橙色的小猫戴着蝴蝶结在花园里追蜻蜓',
        hint: '5 要素齐全 = 满分！',
      },
    ],

    aiName: '小启',
    aiAvatar: '🦉',
    systemPrompt: `你是"小启"，一个陪伴 8-10 岁孩子学习 AI 视频创作的 AI 老师。
当前任务：教"提示词工程"基础——描述越具体，结果越好。
教孩子用"5 要素"（颜色+品种+装饰+场景+动作）写描述。`,

    tools: ['generate_image', 'image_to_video', 'text_chat'],
    scoringCriteria: {
      creativity: 30,
      technical: 25,
      narrative: 20,
      aesthetic: 15,
      compliance: 10,
    },
  },

  {
    id: 'L4',
    orderNum: 4,
    title: '5 秒分镜练习',
    description: '用 3 个镜头讲一段 15 秒的小故事',
    coverEmoji: '🎞️',
    estimatedMinutes: 30,
    rewardTokens: 50,
    difficulty: 2,
    prerequisites: ['L3'],

    steps: [
      {
        id: 'L4-s1',
        orderNum: 1,
        title: '了解分镜',
        instruction: '一段 15 秒视频 = 3 个镜头 × 5 秒。镜头：远景 / 中景 / 特写。',
        type: 'action',
      },
      {
        id: 'L4-s2',
        orderNum: 2,
        title: '写 3 个镜头的描述',
        instruction: '主题自选。每个镜头要有清楚的开始、过程、结束。',
        type: 'input',
        placeholder: '镜头1：...\n镜头2：...\n镜头3：...',
      },
    ],

    aiName: '创创',
    aiAvatar: '🎬',
    systemPrompt: `你是"创创"，一个陪伴 11-13 岁孩子学习 AI 创作的 AI 导师。
当前任务：教"分镜"概念，导演思维。
孩子需要写 3 个镜头的描述，引导他们用"开始→过程→结果"的逻辑。`,

    tools: ['generate_image', 'image_to_video', 'add_bgm'],
    scoringCriteria: {
      creativity: 30,
      technical: 25,
      narrative: 25,
      aesthetic: 10,
      compliance: 10,
    },
  },

  {
    id: 'L5',
    orderNum: 5,
    title: '色彩与情绪',
    description: '理解色彩如何传递情绪',
    coverEmoji: '🎨',
    estimatedMinutes: 30,
    rewardTokens: 50,
    difficulty: 2,
    prerequisites: ['L4'],

    steps: [
      {
        id: 'L5-s1',
        orderNum: 1,
        title: '看色彩对比',
        instruction: '看 6 张"森林"图。同一场景不同色彩，情绪完全不同。',
        type: 'action',
      },
      {
        id: 'L5-s2',
        orderNum: 2,
        title: '用色彩词描述画面',
        instruction: '选择一个你想表达的情绪（神秘/温馨/紧张/欢乐），用色彩词描述一个场景。',
        type: 'input',
        placeholder: '比如：寒冷的蓝色雪夜森林，神秘而安静',
      },
    ],

    aiName: '创创',
    aiAvatar: '🎬',
    systemPrompt: `你是"创创"，一个陪伴 11-13 岁孩子学习 AI 创作的 AI 导师。
当前任务：教"色彩"如何传递情绪。
教孩子识别冷色/暖色/低饱和/高饱和的不同情感。`,

    tools: ['generate_image'],
    scoringCriteria: {
      creativity: 30,
      technical: 25,
      narrative: 20,
      aesthetic: 15,
      compliance: 10,
    },
  },
];

export function getLevel(id: string): Level | undefined {
  return LEVELS.find((l) => l.id === id);
}

export function getAvailableLevels(completedIds: string[]): Level[] {
  return LEVELS.filter(
    (l) =>
      l.prerequisites.length === 0 ||
      l.prerequisites.every((p) => completedIds.includes(p)),
  );
}
