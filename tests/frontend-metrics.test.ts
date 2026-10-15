import { describe, expect, it } from 'vitest';
import { createSearchMetrics } from '../src/search-metrics';

describe('anonymous input-to-frame metrics', () => {
  it('separates debounce, invoke, first result and settled frame timings', () => {
    const metrics = createSearchMetrics('desktop');
    metrics.begin('a', 10);
    metrics.dispatched('a', 40);
    metrics.invoked('a', 50);
    metrics.rendered('a', 66, { complete: false, count: 2 });
    metrics.rendered('a', 85, { complete: true, count: 8 });
    expect(metrics.latest()).toMatchObject({ debounceMs: 30, invokeMs: 10, firstResultMs: 56, settledMs: 75, resultCount: 8 });
    expect(metrics.report()).toMatchObject({ mode: 'desktop', sampleCount: 1, settled: { p50Ms: 75, p95Ms: 75 } });
    expect(JSON.stringify(metrics.report())).not.toContain('"a"');
  });
  it('drops superseded replies and counts empty settled queries without invented first results', () => {
    const metrics = createSearchMetrics('preview');
    metrics.begin('old', 0);
    metrics.begin('new', 20);
    metrics.rendered('old', 60, { complete: true, count: 10 });
    metrics.rendered('new', 70, { complete: true, count: 0 });
    metrics.rendered('new', 90, { complete: true, count: 1 });
    expect(metrics.latest()).toMatchObject({ firstResultMs: null, settledMs: 50, resultCount: 0 });
    expect(metrics.report()).toMatchObject({ mode: 'preview', sampleCount: 1, cancelledCount: 1 });
  });
  it('bounds samples and computes nearest rank p95, while preserving late invoke timings', () => {
    const metrics = createSearchMetrics('desktop');
    for (let index = 1; index <= 105; index++) {
      metrics.begin(`request-${index}`, 0);
      metrics.dispatched(`request-${index}`, 10);
      metrics.rendered(`request-${index}`, index + 20, { complete: true, count: 1 });
      metrics.invoked(`request-${index}`, 15);
    }
    expect(metrics.latest()?.invokeMs).toBe(5);
    expect(metrics.report()).toMatchObject({ sampleCount: 100, settled: { p95Ms: 120 } });
  });
  it('cancels without a sample and rejects invalid clocks', () => {
    const metrics = createSearchMetrics('desktop');
    metrics.begin('a', 30);
    metrics.cancel('a');
    metrics.rendered('a', 70, { complete: true, count: 1 });
    metrics.begin('b', 90);
    metrics.rendered('b', 80, { complete: true, count: 1 });
    expect(metrics.report().sampleCount).toBe(0);
  });
});
