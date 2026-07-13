// 阶段5 单镜微调 — 截图回归（Storybook）
// 每个 story 代表一种产品状态：空 / 单速 / 全选
import type { Meta, StoryObj } from '@storybook/react';
import ShotFxEditor from '../editors/ShotFxEditor';
import { useDirectorStore, type DirectorShot } from '../../../stores/directorStore';

const meta: Meta<typeof ShotFxEditor> = {
  title: 'Studio/Editors/ShotFxEditor',
  component: ShotFxEditor,
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
type Story = StoryObj<typeof ShotFxEditor>;

const SHOT: DirectorShot = {
  id: 'shot_demo',
  description: '小七跑出森林，来到冰山前',
  motion: '小七冲过草地，刹车仰望冰山',
  previewUrl: null,
  seed: 1,
  previewing: false, beat: "hook", mood: "joyful", camera: "wide", characterRefs: ["xiaoqi"], transitionToNext: "none",
};

const seedShot = () => {
  useDirectorStore.getState().reset();
  useDirectorStore.setState({ shots: [SHOT] });
};

export const Empty: Story = {
  name: '初始（无微调）',
  render: () => {
    seedShot();
    return <ShotFxEditor shotId={SHOT.id} />;
  },
};

export const SpeedOnly: Story = {
  name: '只改了速度(慢动作)',
  render: () => {
    seedShot();
    useDirectorStore.getState().setShotFx(SHOT.id, { speed: 'slow' });
    return <ShotFxEditor shotId={SHOT.id} />;
  },
};

export const Full: Story = {
  name: '全选（超快 / 魔法叮咚 / 樱花雨）',
  render: () => {
    seedShot();
    useDirectorStore.getState().setShotFx(SHOT.id, {
      speed: 'fast',
      sound: 'magic',
      filter: 'sakura',
    });
    return <ShotFxEditor shotId={SHOT.id} />;
  },
};