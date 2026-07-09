import type { Meta, StoryObj } from '@storybook/react';
import Button from '../Button';

const meta: Meta<typeof Button> = {
  title: 'Components/Button',
  component: Button,
  tags: ['autodocs'],
  argTypes: {
    variant: {
      control: 'select',
      options: ['primary', 'secondary', 'ghost', 'danger'],
    },
    size: {
      control: 'select',
      options: ['sm', 'md', 'lg', 'xl'],
    },
    loading: { control: 'boolean' },
    disabled: { control: 'boolean' },
    children: { control: 'text' },
  },
};

export default meta;
type Story = StoryObj<typeof Button>;

export const Primary: Story = {
  args: { variant: 'primary', children: '开始挑战' },
};

export const Secondary: Story = {
  args: { variant: 'secondary', children: '取消' },
};

export const Ghost: Story = {
  args: { variant: 'ghost', children: '查看更多' },
};

export const Danger: Story = {
  args: { variant: 'danger', children: '删除作品' },
};

export const Large: Story = {
  args: { size: 'xl', children: '🚀 现在更新' },
};

export const Loading: Story = {
  args: { loading: true, children: '保存中' },
};

export const Disabled: Story = {
  args: { disabled: true, children: 'Token 不足' },
};
