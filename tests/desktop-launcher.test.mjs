import { afterEach, describe, expect, it } from 'vitest';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { describeLaunch } from '../scripts/desktop.mjs';

const projectPath = fileURLToPath(new URL('../', import.meta.url));
const fixtures = [];

afterEach(() => {
  for (const fixture of fixtures.splice(0)) rmSync(fixture, { recursive: true, force: true });
});

function fixtureProject(cliSource) {
  const fixture = mkdtempSync(join(tmpdir(), 'supersearch-desktop-'));
  fixtures.push(fixture);
  mkdirSync(join(fixture, 'scripts'), { recursive: true });
  copyFileSync(join(projectPath, 'scripts/desktop.mjs'), join(fixture, 'scripts/desktop.mjs'));
  if (cliSource !== undefined) {
    mkdirSync(join(fixture, 'node_modules/@tauri-apps/cli'), { recursive: true });
    writeFileSync(join(fixture, 'node_modules/@tauri-apps/cli/tauri.js'), cliSource);
  }
  return fixture;
}

describe('desktop launcher commands', () => {
  it.each(['dev', 'build'])('runs the local Tauri CLI for %s', (mode) => {
    const launch = describeLaunch(mode, projectPath, {});
    expect(launch.command).toBe(process.execPath);
    expect(launch.args).toEqual([join(projectPath, 'node_modules/@tauri-apps/cli/tauri.js'), mode]);
  });

  it.each(['test', 'check'])('runs local cargo for %s', (mode) => {
    const launch = describeLaunch(mode, projectPath, {});
    expect(launch.command).toBe(join(projectPath, '.tools/cargo/bin/cargo.exe'));
    expect(launch.args).toEqual([mode, '--manifest-path', 'src-tauri/Cargo.toml']);
  });

  it('passes the smoke argument to the native app', () => {
    const launch = describeLaunch('smoke', projectPath, {});
    expect(launch.command).toBe(join(projectPath, '.tools/cargo/bin/cargo.exe'));
    expect(launch.args).toEqual(['run', '--manifest-path', 'src-tauri/Cargo.toml', '--', '--smoke-test']);
  });

  it('rejects unsupported modes before launching anything', () => {
    expect(() => describeLaunch('publish', projectPath, {})).toThrow('Unknown desktop mode: publish');
  });

  it('installs the advisory auditor only through the project-local toolchain', () => {
    const launch = describeLaunch('audit-install', projectPath, {});
    expect(launch.command).toBe(join(projectPath, '.tools/cargo/bin/cargo.exe'));
    expect(launch.args).toEqual(['install', 'cargo-audit', '--locked']);
    expect(launch.options.env.CARGO_HOME).toBe(join(projectPath, '.tools/cargo'));
  });

  it('forwards native test filters and exposes the isolated benchmark mode', () => {
    expect(describeLaunch('test', projectPath, {}, ['ranking::tests', '--', '--nocapture']).args)
      .toEqual(['test', '--manifest-path', 'src-tauri/Cargo.toml', 'ranking::tests', '--', '--nocapture']);
    expect(describeLaunch('benchmark', projectPath, {}).args)
      .toEqual(['run', '--manifest-path', 'src-tauri/Cargo.toml', '--', '--ui-benchmark']);
  });

  it('sets only child environment overrides and normalizes Windows PATH keys', () => {
    const environment = { Path: 'system-bin', PATH: 'duplicate-bin', cargo_home: 'global-cargo',
      Rustup_Home: 'global-rustup', NPM_CONFIG_CACHE: 'global-cache', Temp: 'global-temp',
      tmp: 'global-tmp', KEEP_ME: 'unchanged' };
    const before = { ...environment };
    const { options } = describeLaunch('test', projectPath, environment);
    expect(options).toMatchObject({ cwd: projectPath, stdio: 'inherit', windowsHide: true });
    expect(options.env).toEqual({
      PATH: `${join(projectPath, '.tools/cargo/bin')}${delimiter}system-bin`,
      CARGO_HOME: join(projectPath, '.tools/cargo'),
      RUSTUP_HOME: join(projectPath, '.tools/rustup'),
      npm_config_cache: join(projectPath, '.tools/npm-cache'),
      TEMP: join(projectPath, '.tools/tmp'),
      TMP: join(projectPath, '.tools/tmp'),
      KEEP_ME: 'unchanged',
    });
    expect(environment).toEqual(before);
  });
});

describe('desktop launcher process behavior', () => {
  it('inherits output, creates local temp, and propagates child exit codes', () => {
    const fixture = fixtureProject(`
      console.log(JSON.stringify({ cwd: process.cwd(), cargo: process.env.CARGO_HOME,
        temp: process.env.TEMP, mode: process.argv[2] }));
      process.exit(7);
    `);
    const result = spawnSync(process.execPath, [join(fixture, 'scripts/desktop.mjs'), 'build'], { encoding: 'utf8' });
    expect(result.status).toBe(7);
    expect(JSON.parse(result.stdout)).toEqual({ cwd: resolve(fixture),
      cargo: join(fixture, '.tools/cargo'), temp: join(fixture, '.tools/tmp'), mode: 'build' });
    expect(existsSync(join(fixture, '.tools/tmp'))).toBe(true);
  });

  it('reports child spawn errors with a failing exit code', () => {
    const fixture = fixtureProject();
    const result = spawnSync(process.execPath, [join(fixture, 'scripts/desktop.mjs'), 'test'], { encoding: 'utf8' });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Failed to launch desktop test:');
    expect(result.stderr).toContain('ENOENT');
  });

  it('reports invalid CLI modes without creating temp directories', () => {
    const fixture = fixtureProject();
    const result = spawnSync(process.execPath, [join(fixture, 'scripts/desktop.mjs'), 'unknown'], { encoding: 'utf8' });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('Unknown desktop mode: unknown');
    expect(existsSync(join(fixture, '.tools/tmp'))).toBe(false);
  });
});
