// W10 Day 4 — skillStore 单测
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { useSkillStore } from './skillStore';

vi.mock('../api/tauri', () => ({
  listInstalledSkills: vi.fn(),
  listAvailableSkills: vi.fn(),
  installSkill: vi.fn(),
  uninstallSkill: vi.fn(),
  toggleSkill: vi.fn(),
}));

import * as tauri from '../api/tauri';

describe('skillStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSkillStore.setState({
      installed: [],
      available: [],
      loadingInstalled: false,
      loadingAvailable: false,
      error: null,
      busy: null,
    });
  });

  it('refreshInstalled fetches and stores', async () => {
    (tauri.listInstalledSkills as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: 'eng', name: '英语', version: 'v1', enabled: true, installedAt: 1, audience: 'child' },
    ]);
    await useSkillStore.getState().refreshInstalled();
    expect(useSkillStore.getState().installed).toHaveLength(1);
    expect(useSkillStore.getState().installed[0].id).toBe('eng');
  });

  it('refreshAvailable fetches and stores', async () => {
    (tauri.listAvailableSkills as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: 'eng',
        name: '英语',
        version: 'v1',
        audience: 'child',
        ageTier: [1],
        category: 'language',
        sizeBytes: 1000,
        installed: false,
        enabled: false,
        creditsPerUse: 0,
        dailyQuota: 0,
        fromCache: false,
      },
    ]);
    await useSkillStore.getState().refreshAvailable();
    expect(useSkillStore.getState().available).toHaveLength(1);
  });

  it('install() invokes api and refreshes', async () => {
    (tauri.installSkill as ReturnType<typeof vi.fn>).mockResolvedValue({
      skillId: 'eng',
      version: 'v1',
      sizeBytes: 100,
      installedAt: 1,
      auditId: 'audit-1',
    });
    (tauri.listInstalledSkills as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: 'eng', name: '英语', version: 'v1', enabled: true, installedAt: 1, audience: 'child' },
    ]);
    (tauri.listAvailableSkills as ReturnType<typeof vi.fn>).mockResolvedValue([]);

    const receipt = await useSkillStore.getState().install('eng', '1234');
    expect(receipt.auditId).toBe('audit-1');
    expect(tauri.installSkill).toHaveBeenCalledWith('eng', '1234');
    expect(useSkillStore.getState().installed).toHaveLength(1);
  });

  it('install() failure records error and rethrows', async () => {
    (tauri.installSkill as ReturnType<typeof vi.fn>).mockRejectedValue(new Error('bad pin'));
    await expect(useSkillStore.getState().install('eng', 'wrong')).rejects.toThrow('bad pin');
    expect(useSkillStore.getState().error).toBe('bad pin');
    expect(useSkillStore.getState().busy).toBeNull();
  });

  it('toggle() flips enabled', async () => {
    (tauri.toggleSkill as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    await useSkillStore.getState().toggle('eng', true);
    expect(tauri.toggleSkill).toHaveBeenCalledWith('eng', true);
  });

  it('uninstall() removes', async () => {
    (tauri.uninstallSkill as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    await useSkillStore.getState().uninstall('eng');
    expect(tauri.uninstallSkill).toHaveBeenCalledWith('eng');
  });
});