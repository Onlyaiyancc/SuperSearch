import { afterEach, describe, expect, it } from 'vitest';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { parseReleaseArgs, publishRelease } from '../scripts/release.mjs';

const roots = [];
const now = new Date('2026-10-14T16:00:00.000Z');
const output = 'outputs/Priority-Optimization-2026-10-15';
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'supersearch-release-'));
  roots.push(root);
  const write = (path, value, age = 60_000) => {
    const target = join(root, path);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, typeof value === 'string' ? value : JSON.stringify(value));
    const time = new Date(now.getTime() - age);
    utimesSync(target, time, time);
  };
  write('package.json', { version: '0.1.6' }, 600_000);
  write('src-tauri/Cargo.toml', '[package]\nname = "supersearch"\nversion = "0.1.6"\n\n[dependencies]\nother = { version = "2" }', 600_000);
  write('src-tauri/tauri.conf.json', { version: '0.1.6' }, 600_000);
  write('src/App.svelte', '<main>Search</main>', 600_000);
  write('src-tauri/target/release/supersearch.exe', 'final executable', 240_000);
  write('src-tauri/target/release/bundle/nsis/SuperSearch_0.1.6_x64-setup.exe', 'final installer', 180_000);
  const verification = {
    version: '0.1.6', outcome: 'passed', exitCode: 0,
    timestamp: new Date(now.getTime() - 60_000).toISOString(),
    steps: ['frontend-tests', 'frontend-check', 'frontend-build', 'rust-tests', 'rust-check', 'npm-audit', 'bundled-smoke', 'rust-audit']
      .map(name => ({ name, outcome: 'passed', exitCode: 0 })),
  };
  const ui = { outcome: 'passed', native: true, visibleWindow: true,
    scenarios: { unique: { firstResult: { samples: 20, p95Ms: 50 } }, broad: { firstResult: { samples: 20, p95Ms: 80 } } } };
  const resources = { outcome: 'passed', executableSha256: sha256('final executable').toUpperCase(),
    nativeReportSha256: sha256(JSON.stringify(ui)),
    webViewObserved: true, sampleCount: 1, samples: [{ processCount: 2, privateMiB: 20 }] };
  write('evidence/checks.json', verification);
  write('evidence/ui.json', ui);
  write('evidence/resources.json', resources);
  const options = { projectRoot: root, output, now,
    evidence: { verification: 'evidence/checks.json', uiBenchmark: 'evidence/ui.json', resources: 'evidence/resources.json' } };
  return { root, write, options, verification, ui, resources };
}

afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

describe('release publication', () => {
  it.each(['src/App.svelte', 'src-tauri/src/engine.rs', 'index.html', 'src-tauri/vendor/bridge.c'])
  ('rejects a fresh verification of an executable built before %s changed', path => {
    const { root, write, options } = fixture();
    write(path, 'changed production input', 120_000);
    expect(() => publishRelease(options)).toThrow(/executable.*rebuild/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });

  it('rejects an installer built before the NSIS hook changed', () => {
    const { root, write, options } = fixture();
    write('src-tauri/windows/hooks.nsh', '; changed hook', 120_000);
    expect(() => publishRelease(options)).toThrow(/installer.*rebuild/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });
  it.each(['valid', 'native', 'samples', 'scale'])('checks %s half-hour soak and scale evidence when requested', mode => {
    const { root, write, options, ui, resources } = fixture();
    const soak = { outcome: 'passed', native: true, requestedDurationMs: 1800000, soakElapsedMs: 1800001, cycles: 60 };
    const scale = { ...ui, fixtureFiles: 100000, exactBeyondFirstPageVerified: true, defaultSelectionVerified: true, finalQueryVerified: true };
    const rawSamples = '{"timestampMs":0}\n';
    write('test-results/ui-native-soak.json', soak);
    write('test-results/resource-soak.json', { outcome: 'passed', executableSha256: resources.executableSha256,
      nativeReportSha256: sha256(JSON.stringify(soak)), samplesSha256: sha256(rawSamples),
      sampleCount: 700, elapsedMs: 1800000, roles: { webview: { samples: 700 } } });
    write('test-results/scale-native-benchmark.json', scale);
    write('test-results/scale-resource-usage.json', { ...resources, nativeReportSha256: sha256(JSON.stringify(scale)) });
    write('test-results/resource-soak-samples.ndjson', rawSamples);
    if (mode !== 'valid') {
      if (mode === 'native') write('test-results/ui-native-soak.json', { ...soak, cycles: 61 });
      if (mode === 'samples') write('test-results/resource-soak-samples.ndjson', rawSamples + '{"timestampMs":1}\n');
      if (mode === 'scale') write('test-results/scale-native-benchmark.json', { ...scale, fixtureFiles: 200000 });
      expect(() => publishRelease({ ...options, diagnostics: true })).toThrow(/digest|paired/i);
      expect(existsSync(join(root, 'outputs'))).toBe(false);
      return;
    }
    const manifest = publishRelease({ ...options, diagnostics: true });
    expect(manifest.files.some(file => file.name === 'Resource-Soak-Report.json')).toBe(true);
    expect(manifest.files.some(file => file.name === 'Scale-Native-Benchmark-Report.json')).toBe(true);
    expect(existsSync(join(root, output, 'Resource-Soak-Samples.ndjson'))).toBe(true);
  });

  it('rejects a different fresh native report paired with existing resource evidence', () => {
    const { root, write, options, ui } = fixture();
    write('evidence/ui.json', { ...ui, fixtureFiles: 999 });
    expect(() => publishRelease(options)).toThrow(/native.*digest|native.*pair/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });

  it('rejects abbreviated or mismatched soak evidence before publishing', () => {
    const { root, write, options, resources } = fixture();
    write('test-results/ui-native-soak.json', { outcome: 'passed', native: true, requestedDurationMs: 5000, soakElapsedMs: 5001, cycles: 1 });
    write('test-results/resource-soak.json', { outcome: 'passed', executableSha256: resources.executableSha256,
      sampleCount: 7, elapsedMs: 5000, roles: { webview: { samples: 7 } } });
    expect(() => publishRelease({ ...options, diagnostics: true })).toThrow(/soak/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });
  it('copies final versioned artifacts and evidence, with verifiable manifest and atomic latest pointer', () => {
    const { root, options } = fixture();
    const manifest = publishRelease(options);
    expect(manifest.version).toBe('0.1.6');
    expect(manifest.files).toHaveLength(5);
    for (const file of manifest.files) {
      const bytes = readFileSync(join(root, output, file.name));
      expect(file.sha256).toBe(sha256(bytes));
      expect(file.bytes).toBe(bytes.length);
    }
    const sums = readFileSync(join(root, output, 'SHA256SUMS.txt'), 'utf8').trim().split('\n');
    expect(sums).toHaveLength(6);
    for (const line of sums) {
      const [hash, name] = line.split('  ');
      expect(hash).toBe(sha256(readFileSync(join(root, output, name))));
    }
    const pointer = JSON.parse(readFileSync(join(root, 'outputs/Latest-Release.json'), 'utf8'));
    expect(pointer).toMatchObject({ version: '0.1.6', directory: output, executableSha256: sha256('final executable') });
    expect(pointer.manifestSha256).toBe(sha256(readFileSync(join(root, pointer.manifest))));
    expect(readdirSync(join(root, 'outputs'))).toEqual(['Latest-Release.json', 'Priority-Optimization-2026-10-15']);
  });

  it('produces identical release files given identical inputs and publication time', () => {
    const first = fixture();
    const second = fixture();
    publishRelease(first.options);
    publishRelease(second.options);
    for (const name of readdirSync(join(first.root, output))) {
      expect(readFileSync(join(first.root, output, name))).toEqual(readFileSync(join(second.root, output, name)));
    }
  });

  it('advances the latest pointer to a new release while preserving the previous release', () => {
    const { root, options } = fixture();
    publishRelease(options);
    const previous = readFileSync(join(root, output, 'Release-Manifest.json'));
    publishRelease({ ...options, output: 'outputs/Next-Release' });
    const latest = JSON.parse(readFileSync(join(root, 'outputs/Latest-Release.json'), 'utf8'));
    expect(latest.directory).toBe('outputs/Next-Release');
    expect(readFileSync(join(root, output, 'Release-Manifest.json'))).toEqual(previous);
    expect(readdirSync(join(root, 'outputs')).some(name => name.startsWith('.Latest-Release-'))).toBe(false);
  });

  it('reads PowerShell UTF-8 reports with a BOM', () => {
    const { write, options, resources } = fixture();
    write('evidence/resources.json', '\uFEFF' + JSON.stringify(resources));
    expect(publishRelease(options).version).toBe('0.1.6');
  });

  it('refuses existing release directories and preserves existing evidence and latest pointer', () => {
    const { root, write, options } = fixture();
    write(`${output}/old-evidence.json`, 'keep this');
    write('outputs/Latest-Release.json', 'keep pointer');
    expect(() => publishRelease(options)).toThrow(/already exists/i);
    expect(readFileSync(join(root, output, 'old-evidence.json'), 'utf8')).toBe('keep this');
    expect(readFileSync(join(root, 'outputs/Latest-Release.json'), 'utf8')).toBe('keep pointer');
  });

  it.each(['../escape', 'outputs/../escape', 'outputs', 'outputs/nested/release', 'C:\\escape', '/escape', 'outputs/release.', 'outputs/CON', 'outputs/release:stream', 'outputs/.hidden'])
  ('rejects unsafe output %s before writing', unsafe => {
    const { root, options } = fixture();
    expect(() => publishRelease({ ...options, output: unsafe })).toThrow(/output/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });

  it('rejects an outputs junction instead of writing through it', () => {
    const { root, options } = fixture();
    const outside = mkdtempSync(join(tmpdir(), 'supersearch-release-outside-'));
    roots.push(outside);
    symlinkSync(outside, join(root, 'outputs'), 'junction');
    expect(() => publishRelease(options)).toThrow(/symbolic|junction/i);
    expect(readdirSync(outside)).toEqual([]);
  });

  it('rejects a linked latest pointer and preserves its target', () => {
    const { root, write, options } = fixture();
    write('outside-pointer/old.json', 'old pointer');
    mkdirSync(join(root, 'outputs'));
    symlinkSync(join(root, 'outside-pointer'), join(root, 'outputs/Latest-Release.json'), 'junction');
    expect(() => publishRelease(options)).toThrow(/symbolic|junction/i);
    expect(readFileSync(join(root, 'outside-pointer/old.json'), 'utf8')).toBe('old pointer');
    expect(existsSync(join(root, output))).toBe(false);
  });

  it.each(['package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml'])
  ('rejects mismatched version in %s', path => {
    const { root, write, options } = fixture();
    write(path, path.endsWith('toml') ? '[package]\nversion = "9.0.0"' : { version: '9.0.0' }, 600_000);
    expect(() => publishRelease(options)).toThrow(/version/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });

  it.each([
    ['failed report', report => { report.outcome = 'failed'; }],
    ['failed step', report => { report.steps[2].exitCode = 1; }],
    ['missing native smoke', report => { report.steps = report.steps.filter(step => step.name !== 'bundled-smoke'); }],
    ['missing rust audit', report => { report.steps = report.steps.filter(step => step.name !== 'rust-audit'); }],
    ['duplicate step', report => { report.steps.push(report.steps[0]); }],
    ['wrong version', report => { report.version = '0.1.5'; }],
    ['stale timestamp', report => { report.timestamp = '2026-10-12T16:00:00.000Z'; }],
    ['before final build', report => { report.timestamp = new Date(now.getTime() - 300_000).toISOString(); }],
    ['future timestamp', report => { report.timestamp = '2026-10-15T16:00:00.000Z'; }],
  ])('rejects verification with %s and preserves the latest pointer', (_, mutate) => {
    const { root, write, options, verification } = fixture();
    mutate(verification);
    write('evidence/checks.json', verification);
    write('outputs/Latest-Release.json', 'old pointer');
    expect(() => publishRelease(options)).toThrow(/verification/i);
    expect(existsSync(join(root, output))).toBe(false);
    expect(readFileSync(join(root, 'outputs/Latest-Release.json'), 'utf8')).toBe('old pointer');
  });

  it('rejects verification older than changed source', () => {
    const { root, write, options } = fixture();
    write('src/App.svelte', '<main>Changed</main>', 30_000);
    expect(() => publishRelease(options)).toThrow(/verification.*source/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });

  it('rejects verification older than changed public assets', () => {
    const { root, write, options } = fixture();
    write('public/mark.svg', '<svg/>', 30_000);
    expect(() => publishRelease(options)).toThrow(/verification.*source/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });

  it('rejects evidence paths outside the project', () => {
    const { root, options } = fixture();
    expect(() => publishRelease({ ...options, evidence: { verification: '../checks.json' } })).toThrow(/unsafe/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });

  it('rejects verification whose file predates the final installer', () => {
    const { write, options, verification } = fixture();
    write('evidence/checks.json', verification, 300_000);
    expect(() => publishRelease(options)).toThrow(/verification.*fresh/i);
  });

  it.each([
    ['failed UI', ui => { ui.outcome = 'failed'; }, () => {}],
    ['invisible UI', ui => { ui.visibleWindow = false; }, () => {}],
    ['non-native UI', ui => { ui.native = false; }, () => {}],
    ['missing measurements', ui => { delete ui.scenarios; }, () => {}],
    ['failed resources', () => {}, resources => { resources.outcome = 'failed'; }],
    ['missing WebView', () => {}, resources => { resources.webViewObserved = false; }],
    ['empty samples', () => {}, resources => { resources.sampleCount = 0; resources.samples = []; }],
    ['wrong executable', () => {}, resources => { resources.executableSha256 = '0'.repeat(64); }],
  ])('rejects %s before publishing', (_, mutateUi, mutateResources) => {
    const { root, write, options, ui, resources } = fixture();
    mutateUi(ui);
    mutateResources(resources);
    write('evidence/ui.json', ui);
    write('evidence/resources.json', resources);
    expect(() => publishRelease(options)).toThrow(/native|resource|executable/i);
    expect(existsSync(join(root, 'outputs'))).toBe(false);
  });

  it.each(['evidence/ui.json', 'evidence/resources.json'])('rejects stale %s', path => {
    const { write, options, ui, resources } = fixture();
    write(path, path.includes('resources') ? resources : ui, 300_000);
    expect(() => publishRelease(options)).toThrow(/fresh/i);
  });

  it('rejects empty final artifacts', () => {
    const { write, options } = fixture();
    write('src-tauri/target/release/supersearch.exe', '', 240_000);
    expect(() => publishRelease(options)).toThrow(/empty/i);
  });
});

describe('release CLI options', () => {
  it('accepts explicit long-duration evidence without allowing arbitrary flags', () => {
    expect(parseReleaseArgs(['--with-diagnostics'])).toEqual({ output, diagnostics: true });
    expect(parseReleaseArgs(['--output', 'outputs/Next-Release', '--with-diagnostics']))
      .toEqual({ output: 'outputs/Next-Release', diagnostics: true });
  });
  it('uses the requested default and accepts one explicit output', () => {
    expect(parseReleaseArgs([])).toEqual({ output });
    expect(parseReleaseArgs(['--output', 'outputs/Next-Release'])).toEqual({ output: 'outputs/Next-Release' });
  });
  it.each([['--force'], ['--output'], ['--output', output, '--force'], ['--output', output, '--output', output]].map(args => [args]))
  ('rejects unsupported or incomplete arguments %j', args => {
    expect(() => parseReleaseArgs(args)).toThrow(/option|output/i);
  });
});
