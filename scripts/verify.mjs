import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const projectRoot = dirname(dirname(scriptPath));

export function verificationSteps({ smoke = false, auditRust = false } = {}) {
  const steps = [
    { name: 'frontend-tests', args: ['test'] },
    { name: 'frontend-check', args: ['run', 'check'] },
    { name: 'frontend-build', args: ['run', 'build'] },
    { name: 'rust-tests', args: ['run', 'desktop:test'] },
    { name: 'rust-check', args: ['run', 'desktop:check'] },
    { name: 'npm-audit', args: ['audit', '--omit=dev'] },
  ];
  if (smoke) steps.push({ name: 'bundled-smoke', args: ['run', 'desktop:smoke:bundled'] });
  if (auditRust) steps.push({ name: 'rust-audit', args: ['run', 'desktop:audit'] });
  return steps;
}

export function runVerification(steps, run) {
  const results = [];
  for (const step of steps) {
    const started = Date.now();
    let result;
    try { result = run(step); }
    catch (error) { result = { status: null, error }; }
    const exitCode = result.status ?? 1;
    results.push({ name: step.name, outcome: exitCode === 0 ? 'passed' : 'failed', exitCode,
      elapsedMs: Date.now() - started, ...(result.error ? { error: result.error.message ?? String(result.error) } : {}) });
    // 首次失败立即停止，后续成功不能掩盖前面失败的命令。
    if (exitCode !== 0) return { outcome: 'failed', exitCode, steps: results };
  }
  return { outcome: 'passed', exitCode: 0, steps: results };
}

if (process.argv[1] && resolve(process.argv[1]) === scriptPath) {
  const flags = process.argv.slice(2);
  const unsupported = flags.filter(flag => !['--smoke', '--audit-rust'].includes(flag));
  if (unsupported.length || !process.env.npm_execpath) {
    console.error(unsupported.length ? `Unknown verification option: ${unsupported[0]}` : 'Run this script with npm run verify.');
    process.exitCode = 1;
  } else {
    const report = runVerification(verificationSteps({ smoke: flags.includes('--smoke'), auditRust: flags.includes('--audit-rust') }), step => {
      console.log(`Running ${step.name}...`);
      // Node 直接执行 npm CLI，避免 Windows .cmd 的转义和退出码差异。
      return spawnSync(process.execPath, [process.env.npm_execpath, ...step.args], {
        cwd: projectRoot, env: process.env, stdio: 'inherit', windowsHide: true,
      });
    });
    report.version = JSON.parse(readFileSync(join(projectRoot, 'package.json'), 'utf8')).version;
    report.timestamp = new Date().toISOString();
    mkdirSync(join(projectRoot, 'test-results'), { recursive: true });
    writeFileSync(join(projectRoot, 'test-results/verification.json'), JSON.stringify(report, null, 2) + '\n');
    console.log(`Verification ${report.outcome}; report: test-results/verification.json`);
    process.exitCode = report.exitCode;
  }
}
