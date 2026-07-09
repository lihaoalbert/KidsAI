import type { Meta, StoryObj } from '@storybook/react';
import Input from '../Input';

const meta: Meta<typeof Input> = {
  title: 'Components/Input',
  component: Input,
  tags: ['autodocs'],
  argTypes: {
    label: { control: 'text' },
    helperText: { control: 'text' },
    errorMessage: { control: 'text' },
    showCount: { control: 'boolean' },
    maxLength: { control: 'number' },
    placeholder: { control: 'text' },
  },
};

export default meta;
type Story = StoryObj<typeof Input>;

export const Default: Story = {
  args: { placeholder: '请输入...' },
};

export const WithLabel: Story = {
  args: { label: '昵称', placeholder: '你的昵称' },
};

export const WithHelper: Story = {
  args: {
    label: '视频描述',
    helperText: '提示：描述越具体，AI 生成效果越好',
    placeholder: '一只小猫在月光下追蝴蝶',
    maxLength: 200,
    showCount: true,
  },
};

export const WithError: Story = {
  args: {
    label: '邮箱',
    defaultValue: 'abc',
    errorMessage: '邮箱格式不正确',
  },
};
