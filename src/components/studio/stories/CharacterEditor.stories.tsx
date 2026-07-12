// 阶段2 主角微调 — 截图回归（Storybook）
// 每个 story 代表一种产品状态：空 / 单色 / 全选
import type { Meta, StoryObj } from '@storybook/react';
import CharacterEditor from '../editors/CharacterEditor';
import { useDirectorStore } from '../../../stores/directorStore';

const meta: Meta<typeof CharacterEditor> = {
  title: 'Studio/Editors/CharacterEditor',
  component: CharacterEditor,
  tags: ['autodocs'],
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[380px] bg-gradient-to-b from-gray-50 to-white">
        <Story />
      </div>
    ),
  ],
};
export default meta;
type Story = StoryObj<typeof CharacterEditor>;

const renderEmpty: Story['render'] = () => {
  useDirectorStore.getState().reset();
  return <CharacterEditor />;
};

export const Empty: Story = {
  name: '初始（无微调）',
  render: renderEmpty,
};

export const ColorOnly: Story = {
  name: '选了天空蓝',
  render: () => {
    useDirectorStore.getState().reset();
    useDirectorStore.getState().setCharacterTweak({ color: 'sky' });
    return <CharacterEditor />;
  },
};

export const Full: Story = {
  name: '全选（蓝 / 大 / 勇敢）',
  render: () => {
    useDirectorStore.getState().reset();
    useDirectorStore
      .getState()
      .setCharacterTweak({ color: 'sky', size: 'L', expression: 'brave' });
    return <CharacterEditor />;
  },
};