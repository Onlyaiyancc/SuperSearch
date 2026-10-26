import { describe, expect, it } from 'vitest';
import { call } from '../src/bridge';
import type { Settings } from '../src/types';

describe('conversion preview boundary', () => {
  it('defaults off and round-trips independently of other extensions', async () => {
    const original = await call<Settings>('get_settings');
    expect(original).toHaveProperty('conversionEnabled', false);
    try {
      await call('update_settings', { patch: { conversionEnabled: true } });
      const saved = await call<Settings>('update_settings', { patch: { theme: 'dark' } });
      expect(saved).toMatchObject({ conversionEnabled: true, translationEnabled: original.translationEnabled, filePreviewEnabled: original.filePreviewEnabled });
    } finally { await call('update_settings', { patch: original }); }
  });

  it('never performs native selection, downloads, conversion or execution in the browser', async () => {
    for (const command of ['choose_conversion_directory', 'start_conversion', 'retry_conversion', 'install_conversion_component', 'import_conversion_component', 'open_conversion_output']) {
      await expect(call(command, { id: 'pdf', sourceIds: ['fake-path'] })).rejects.toThrow('桌面应用');
    }
    const sources = await call<{ id: string; name: string; extension: string }[]>('choose_conversion_files');
    expect(sources).toEqual([{ id: 'preview-conversion-image', name: '图片转换预览样例.png', extension: 'png', bytes: 2048 }]);
    await expect(call('list_conversions')).resolves.toEqual([]);
    const components = await call<{ status: string; name: string }[]>('get_conversion_components');
    expect(components).toHaveLength(4);
    expect(components.every(item => item.status !== 'ready' && item.name.includes('预览'))).toBe(true);
    expect(components.every(item => 'canInstall' in item && item.canInstall === false && 'installationSource' in item && item.installationSource === 'unavailable')).toBe(true);
  });
});
