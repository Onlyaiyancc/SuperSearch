import { describe, expect, it } from 'vitest';
import { releaseOptions, candidateInstaller } from '../scripts/release-policy.mjs';

describe('release policy', () => {
  it('prepares stable releases as drafts and preserves beta publication', () => {
    expect(releaseOptions('0.1.7', 'v0.1.7')).toEqual({ draft: true, prerelease: false });
    expect(releaseOptions('0.1.7', 'v0.1.7-beta.1')).toEqual({ draft: false, prerelease: true });
  });
  it.each(['v0.1.6', 'v0.1.70', 'v0.1.7-rc.1', 'v0.1.7-beta.0', 'v0.1.7-beta.1\n', '--draft'])
  ('rejects mismatched or malformed tags: %j', tag => {
    expect(() => releaseOptions('0.1.7', tag)).toThrow();
  });
  it('derives the installer name only for an actual version upgrade', () => {
    expect(candidateInstaller('0.1.7', '0.1.6')).toBe('SuperSearch_0.1.7_x64-setup.exe');
    expect(candidateInstaller('0.2.0', '0.1.6')).toBe('SuperSearch_0.2.0_x64-setup.exe');
    expect(candidateInstaller('0.1.10', '0.1.9')).toBe('SuperSearch_0.1.10_x64-setup.exe');
  });
  it.each(['0.1.6', '0.1.5', '0.1.7/evil', '0.1.7\n', '00.1.7', '0.1.7-beta.1'])
  ('rejects non-upgrades and unsafe version inputs: %j', version => {
    expect(() => candidateInstaller(version, '0.1.6')).toThrow();
  });
});
