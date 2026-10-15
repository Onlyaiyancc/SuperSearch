import { describe, expect, it } from 'vitest';
import { summarizeResources } from '../scripts/resource-summary.mjs';

const sample = (time, privateMiB, cpuPercent = 0) => ({ timestampMs: 1000 + time, elapsedMs: time,
  roles: { app: { processCount: 1, privateMiB, workingSetMiB: privateMiB + 5, cpuPercent },
    webview: { processCount: 3, privateMiB: 50, workingSetMiB: 55, cpuPercent: cpuPercent === null ? null : 0.2 },
    engine: { processCount: 1, privateMiB: 10, workingSetMiB: 12, cpuPercent: cpuPercent === null ? null : 0 },
    service: { processCount: 1, privateMiB: 100, workingSetMiB: 120, cpuPercent: 0 } } });

describe('long-duration resource evidence', () => {
  it('does not turn vanished process observations into zero-memory measurements', () => {
    const report = summarizeResources([{ timestampMs: 1000, roles: {} }, sample(1000, 20, null),
      { timestampMs: 3000, roles: {} }], { startedUnixMs: 1000, phases: [] });
    expect(report.sampleCount).toBe(3);
    expect(report.processTree.samples).toBe(1);
    expect(report.processTree.initialPrivateMedianMiB).toBe(80);
    expect(report.processTree.finalPrivateMedianMiB).toBe(80);
    expect(report.processTree.privateGrowthMiBPerHour).toBeNull();
  });
  it('keeps machine service separate and measures idle against native timestamps', () => {
    const report = summarizeResources([sample(0, 20, null), sample(5000, 21, 1), sample(10000, 22, 0)],
      { startedUnixMs: 1000, phases: [{ name: 'hidden-idle', startMs: 5000, endMs: 10001 }] });
    expect(report.processTree.peakPrivateMiB).toBe(82);
    expect(report.roles.service.peakPrivateMiB).toBe(100);
    expect(report.phases['hidden-idle']).toMatchObject({ samples: 2, medianCpuPercent: 0.7 });
    expect(report.roles.app.cpuSamples).toBe(2);
  });
  it('returns null rather than fabricating idle or trend data from absent samples', () => {
    const report = summarizeResources([sample(0, 20, null)], { startedUnixMs: 1000, phases: [] });
    expect(report.processTree.medianCpuPercent).toBeNull();
    expect(report.processTree.privateGrowthMiBPerHour).toBeNull();
    expect(report.phases).toEqual({});
  });
  it('excludes samples that straddle an idle boundary from idle CPU statistics', () => {
    const samples = [sample(0, 20, 30), sample(7000, 21, 30), sample(9000, 22, 0)];
    samples[1].intervalStartUnixMs = 1000;
    samples[2].intervalStartUnixMs = 8000;
    const report = summarizeResources(samples,
      { startedUnixMs: 1000, phases: [{ name: 'hidden-idle', startMs: 5000, endMs: 10000 }] });
    expect(report.phases['hidden-idle'].cpuSamples).toBe(1);
    expect(report.phases['hidden-idle'].medianCpuPercent).toBeCloseTo(0.2);
  });
  it('reports sustained linear growth without labelling it a memory leak', () => {
    const report = summarizeResources(Array.from({ length: 10 }, (_, i) => sample(i * 60000, 20 + i)),
      { startedUnixMs: 1000, phases: [] });
    expect(report.roles.app.privateGrowthMiBPerHour).toBeCloseTo(60);
    expect(report.elapsedMs).toBe(540000);
  });
});
