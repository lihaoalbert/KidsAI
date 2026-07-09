import type { Meta, StoryObj } from '@storybook/react';
import Card from '../Card';

const meta: Meta<typeof Card> = {
  title: 'Components/Card',
  component: Card,
  tags: ['autodocs'],
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'elevated', 'filled', 'bordered'],
    },
  },
};

export default meta;
type Story = StoryObj<typeof Card>;

export const Default: Story = {
  args: {
    title: 'L1 · 我的第一个 AI 视频',
    description: '学习 AI 视频创作的第一步',
    children: <div className="aspect-video bg-gradient-to-br from-brand-100 to-warm-100 rounded-md" />,
    footer: (
      <div className="flex items-center justify-between text-xs text-gray-600">
        <span>💎 30 学币</span>
        <span>⏱ 20 分钟</span>
      </div>
    ),
  },
};

export const Elevated: Story = {
  args: {
    variant: 'elevated',
    title: '已解锁',
    children: <p className="text-sm text-gray-700">下一关卡已解锁，可以开始挑战了！</p>,
  },
};

export const Bordered: Story = {
  args: {
    variant: 'bordered',
    title: '推荐',
    description: '这是为你推荐的关卡',
    children: <p className="text-xs text-brand-700">⭐ 完成 L1 后解锁</p>,
  },
};
