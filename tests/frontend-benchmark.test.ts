import { describe, expect, it, vi } from 'vitest';
import { runUiBenchmark, runUiSoak, summarizeTimings } from '../src/ui-benchmark';

describe('native benchmark summaries', () => {
  it('reports first-result and completion p95 separately without inventing empty first results', () => {
    const samples = Array.from({ length: 20 }, (_, index) => ({
      debounceMs: 30, invokeMs: 2, firstResultMs: index === 0 ? null : index * 2, settledMs: index * 3, resultCount: index,
    }));
    expect(summarizeTimings(samples)).toMatchObject({
      samples: 20, p50Ms: 27, p95Ms: 54, maximumMs: 57,
      firstResult: { samples: 19, p50Ms: 20, p95Ms: 38, maximumMs: 38 },
    });
    expect(samples[0].settledMs).toBe(0);
  });
  it('represents a missing measurement as null instead of zero', () => {
    expect(summarizeTimings([])).toMatchObject({
      samples: 0, p50Ms: null, p95Ms: null, maximumMs: null,
      firstResult: { samples: 0, p50Ms: null, p95Ms: null, maximumMs: null },
    });
  });
  it('rejects a first-ranked exact result when a different default is selected', async () => {
    let report: unknown;
    await runUiBenchmark({ uniqueQuery: 'report', uniqueTitle: 'report.txt', broadQuery: 'many',
      completionQuery: 'folder/', rankingQuery: 'late', rankingTitle: 'late.txt', minimumBroadResults: 50, setupToFrontendMs: 0 }, {
      search: () => 'current',
      current: () => ({ complete: true, count: 2, firstTitle: 'report.txt', selectedTitle: 'report-notes.txt',
        candidates: 0, ready: true, sample: { debounceMs: 30, invokeMs: 2, firstResultMs: 40, settledMs: 50, resultCount: 2 } }),
      finish: async value => { report = value; },
    });
    expect(report).toMatchObject({ outcome: 'failed', error: 'Exact filename did not rank first or become the default selection' });
  });
});

describe('native soak protocol', () => {
  const config = { uniqueQuery: 'report', uniqueTitle: 'report.txt', broadQuery: 'many',
    completionQuery: 'folder/', rankingQuery: 'late', rankingTitle: 'late.txt', minimumBroadResults: 50,
    setupToFrontendMs: 0, fixtureFiles: 483, coldTimeoutMs: 120000, soakSeconds: 2 };

  it('runs real searches and idle hide/show without reading rendered state while hidden', async () => {
    vi.useFakeTimers();
    let report: any;
    let query = '';
    let hidden = false;
    let compact = true;
    let reads = 0;
    const run = runUiSoak(config, {
      search: raw => { query = raw; compact = false; return raw; },
      current: () => {
        expect(hidden).toBe(false);
        reads++;
        return { complete: true, count: query === 'many' ? 50 : 2, firstTitle: 'report.txt',
          selectedTitle: 'report.txt', candidates: 0, ready: true,
          sample: { debounceMs: 30, invokeMs: 2, firstResultMs: 40, settledMs: 50, resultCount: 2 } };
      },
      compact: async () => { compact = true; },
      window: async hiddenMs => {
        if (hiddenMs !== undefined) {
          hidden = true;
          await new Promise(resolve => setTimeout(resolve, hiddenMs));
          hidden = false;
          compact = true;
        }
        return { visible: true, logicalHeight: compact ? 76 : 628, widthPx: 680, heightPx: compact ? 76 : 628,
          hiddenVerified: hiddenMs !== undefined, hiddenMs: hiddenMs ?? 0 };
      },
      finish: async value => { report = value; },
    });
    await vi.runAllTimersAsync();
    await run;
    vi.useRealTimers();
    expect(reads).toBeGreaterThanOrEqual(3);
    expect(report).toMatchObject({ outcome: 'passed', requestedDurationMs: 2000, fixtureFiles: 483,
      cycles: 1, counts: { hiddenIdle: 1, visibleIdle: 1, compact: 2, unique: 1, broad: 1 } });
    expect(report.startedUnixMs).toBeGreaterThan(0);
    expect(report.phases.map((phase: any) => phase.name)).toEqual(['compact', 'hidden-idle', 'unique-search', 'broad-search', 'compact', 'visible-idle']);
    expect(report.phases.every((phase: any) => phase.endMs >= phase.startMs)).toBe(true);
  });

  it('fails and records the phase when native show does not restore visibility', async () => {
    let report: any;
    await runUiSoak(config, {
      search: () => 'current',
      current: () => ({ complete: true, count: 2, firstTitle: 'report.txt', selectedTitle: 'report.txt',
        candidates: 0, ready: true, sample: { debounceMs: 30, invokeMs: 2, firstResultMs: 40, settledMs: 50, resultCount: 2 } }),
      compact: async () => {},
      window: async () => ({ visible: false, logicalHeight: 76, widthPx: 680, heightPx: 76, hiddenVerified: false, hiddenMs: 0 }),
      finish: async value => { report = value; },
    });
    expect(report).toMatchObject({ outcome: 'failed', stage: 'compact', error: 'Native compact window was not visible or had unexpected geometry' });
  });
});
