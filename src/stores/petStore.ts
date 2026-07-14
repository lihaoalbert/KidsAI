// P0 fix: Pet MVP 起步 store
// 单只宠物 + 基础心情; 跨设备同步 + 完整等级体系 留给 M2

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type PetId = 'huomiao';
export type PetMood = 'happy' | 'sleepy' | 'thinking';

interface PetState {
  petId: PetId;
  mood: PetMood;
  level: number;
  lastSeenAt: number;
  bumpLastSeen: () => void;
  setMood: (m: PetMood) => void;
}

export const usePetStore = create<PetState>()(
  persist(
    (set) => ({
      petId: 'huomiao',
      mood: 'happy',
      level: 1,
      lastSeenAt: Date.now(),
      bumpLastSeen: () => set({ lastSeenAt: Date.now() }),
      setMood: (mood) => set({ mood }),
    }),
    { name: 'kidsai-pet-v1' },
  ),
);