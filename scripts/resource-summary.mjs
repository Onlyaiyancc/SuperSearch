import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const roles = ['app', 'webview', 'engine', 'service'];
const treeRoles = roles.slice(0, 3);
const median = values => {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!sorted.length) return null;
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
};

function statistics(samples) {
  const cpu = samples.map(value => value.cpuPercent).filter(Number.isFinite);
  const memory = samples.filter(value => Number.isFinite(value.privateMiB));
  const origin = memory[0]?.timestampMs ?? 0;
  const averageX = memory.reduce((sum, value) => sum + value.timestampMs - origin, 0) / memory.length;
  const averageY = memory.reduce((sum, value) => sum + value.privateMiB, 0) / memory.length;
  const variance = memory.reduce((sum, value) => sum + (value.timestampMs - origin - averageX) ** 2, 0);
  const covariance = memory.reduce((sum, value) => sum + (value.timestampMs - origin - averageX) * (value.privateMiB - averageY), 0);
  const max = key => samples.length ? Math.max(...samples.map(value => value[key] ?? 0)) : null;
  const quarter = Math.max(1, Math.floor(memory.length / 4));
  return { samples: samples.length, cpuSamples: cpu.length, medianCpuPercent: median(cpu),
    peakCpuPercent: cpu.length ? Math.max(...cpu) : null,
    peakPrivateMiB: max('privateMiB'), peakWorkingSetMiB: max('workingSetMiB'),
    initialPrivateMedianMiB: median(memory.slice(0, quarter).map(value => value.privateMiB)),
    finalPrivateMedianMiB: median(memory.slice(-quarter).map(value => value.privateMiB)),
    privateGrowthMiBPerHour: variance > 0 ? covariance / variance * 3600000 : null };
}

export function summarizeResources(samples, nativeReport) {
  const tree = samples.flatMap(sample => {
    const values = treeRoles.map(role => sample.roles[role]).filter(Boolean);
    // 枚举期间进程已退出时整条观察无效，不能制造零内存及下降趋势。
    if (!values.length) return [];
    // 第一个区间无 CPU 差值；缺失值不能伪装成零占用。
    return [{ timestampMs: sample.timestampMs, intervalStartUnixMs: sample.intervalStartUnixMs ?? sample.timestampMs,
      privateMiB: values.reduce((sum, value) => sum + value.privateMiB, 0),
      workingSetMiB: values.reduce((sum, value) => sum + value.workingSetMiB, 0),
      cpuPercent: values.some(value => Number.isFinite(value.cpuPercent))
        ? values.reduce((sum, value) => sum + (value.cpuPercent ?? 0), 0) : null }];
  });
  const phases = {};
  for (const phase of nativeReport.phases ?? []) {
    const start = nativeReport.startedUnixMs + phase.startMs;
    const end = nativeReport.startedUnixMs + phase.endMs;
    const rows = tree.filter(sample => sample.timestampMs >= start && sample.timestampMs < end)
      .map(sample => ({ ...sample, cpuPercent: sample.intervalStartUnixMs >= start ? sample.cpuPercent : null }));
    (phases[phase.name] ??= []).push(...rows);
  }
  return { sampleCount: samples.length, elapsedMs: samples.length > 1 ? samples.at(-1).timestampMs - samples[0].timestampMs : 0,
    processTree: statistics(tree),
    roles: Object.fromEntries(roles.map(role => [role, statistics(samples.flatMap(sample => sample.roles[role]
      ? [{ timestampMs: sample.timestampMs, ...sample.roles[role] }] : []))])),
    phases: Object.fromEntries(Object.entries(phases).map(([name, rows]) => [name, statistics(rows)])) };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [samplesPath, nativePath, outputPath, metadataPath] = process.argv.slice(2);
  if (!samplesPath || !nativePath || !outputPath || !metadataPath) throw new Error('Expected samples, native report, output and metadata paths');
  const samplesBytes = readFileSync(samplesPath);
  const nativeBytes = readFileSync(nativePath);
  const samples = samplesBytes.toString('utf8').replace(/^\uFEFF/, '').trim().split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line));
  const native = JSON.parse(nativeBytes.toString('utf8').replace(/^\uFEFF/, ''));
  const metadata = JSON.parse(readFileSync(metadataPath, 'utf8').replace(/^\uFEFF/, ''));
  const summary = summarizeResources(samples, native);
  const report = { ...metadata, ...summary,
    nativeReportSha256: createHash('sha256').update(nativeBytes).digest('hex'),
    samplesSha256: createHash('sha256').update(samplesBytes).digest('hex'),
    outcome: native.outcome === 'passed' && summary.roles.webview.samples > 0 ? 'passed' : 'failed',
    measurement: 'Isolated synthetic UI soak. Process tree excludes machine service; service is a separate reference. CPU is normalized by logical processors. Idle CPU excludes intervals spanning phase boundaries. Memory slope alone does not prove leakage.' };
  writeFileSync(outputPath, JSON.stringify(report, null, 2) + '\n');
  console.log(`Native soak resources: ${report.outcome}; samples=${report.sampleCount}; peakPrivateMiB=${report.processTree.peakPrivateMiB}`);
  process.exitCode = report.outcome === 'passed' ? 0 : 1;
}
