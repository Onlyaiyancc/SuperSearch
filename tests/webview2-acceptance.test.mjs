import { describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';

describe.skipIf(process.platform !== 'win32')('WebView2 cloud acceptance safety', () => {
  it.each([
    { GITHUB_ACTIONS: 'false', RUNNER_ENVIRONMENT: 'github-hosted', RUNNER_OS: 'Windows' },
    { GITHUB_ACTIONS: 'true', RUNNER_ENVIRONMENT: 'self-hosted', RUNNER_OS: 'Windows' },
    { GITHUB_ACTIONS: 'true', RUNNER_ENVIRONMENT: 'github-hosted', RUNNER_OS: 'Linux' },
  ])('rejects unsafe execution before touching runtime or firewall: %j', environment => {
    const result = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `
      . ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $PWD 'scripts/webview2-acceptance.ps1'), [Text.Encoding]::UTF8)))
      try { Invoke-WebView2Acceptance; throw 'Guard did not reject execution.' }
      catch { if ($_.Exception.Message -ne 'Disposable Windows GitHub-hosted runner required.') { throw }; Write-Output 'guard-passed' }
    `], { encoding: 'utf8', env: { ...process.env, ...environment }, windowsHide: true });
    expect(result.error).toBeUndefined();
    expect(result.status, result.stderr).toBe(0);
    expect(result.stdout.trim()).toBe('guard-passed');
  });
});
