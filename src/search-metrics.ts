export type TimingSample = { debounceMs: number | null; invokeMs: number | null; firstResultMs: number | null; settledMs: number; resultCount: number };

export function createSearchMetrics(mode: 'desktop' | 'preview') {
  let active: { id: string; start: number; dispatched: number | null; first: number | null; sample: TimingSample | null } | null = null;
  let cancelledCount = 0;
  let invokeEnd: { id: string; now: number } | null = null;
  const samples: TimingSample[] = [];
  const elapsed = (start: number, end: number) => Number.isFinite(end) && end >= start ? Math.round((end - start) * 100) / 100 : null;
  const stats = (values: number[]) => {
    const sorted = values.sort((a, b) => a - b);
    return { count: sorted.length, p50Ms: sorted.length ? sorted[Math.ceil(sorted.length * .5) - 1] : null, p95Ms: sorted.length ? sorted[Math.ceil(sorted.length * .95) - 1] : null, maximumMs: sorted.length ? sorted[sorted.length - 1] : null };
  };
  return {
    begin(id: string, now: number) {
      if (active && !active.sample) cancelledCount++;
      active = { id, start: now, dispatched: null, first: null, sample: null };
      invokeEnd = null;
    },
    dispatched(id: string, now: number) { if (active?.id === id) active.dispatched = now; },
    invoked(id: string, now: number) {
      if (active?.id === id && active.dispatched !== null) {
        // 事件可能先于 invoke 返回；完成后的同一条样本仍可补齐原生调用时间。
        if (active.sample) active.sample.invokeMs = elapsed(active.dispatched, now);
        else invokeEnd = { id, now };
      }
    },
    rendered(id: string, now: number, state: { complete: boolean; count: number }) {
      if (active?.id !== id || active.sample) return;
      const duration = elapsed(active.start, now);
      if (duration === null) return;
      if (state.count > 0 && active.first === null) active.first = duration;
      if (!state.complete) return;
      active.sample = { debounceMs: active.dispatched === null ? null : elapsed(active.start, active.dispatched), invokeMs: invokeEnd?.id === id && active.dispatched !== null ? elapsed(active.dispatched, invokeEnd.now) : null, firstResultMs: active.first, settledMs: duration, resultCount: state.count };
      samples.push(active.sample);
      if (samples.length > 100) samples.shift();
    },
    cancel(id: string) { if (active?.id === id) { if (!active.sample) cancelledCount++; active = null; } },
    latest() { return samples.length ? { ...samples[samples.length - 1] } : null; },
    settled(id: string) { return active?.id === id && active.sample ? { ...active.sample } : null; },
    report() {
      // 只保留数量和耗时，不保存请求标识、搜索词或本地文件路径。
      return { mode, sampleCount: samples.length, cancelledCount, measurement: 'Input scheduling to DOM update and two animation frames; includes debounce and native IPC, not physical display or human typing latency.', firstResult: stats(samples.flatMap(s => s.firstResultMs === null ? [] : [s.firstResultMs])), settled: stats(samples.map(s => s.settledMs)), debounce: stats(samples.flatMap(s => s.debounceMs === null ? [] : [s.debounceMs])), invoke: stats(samples.flatMap(s => s.invokeMs === null ? [] : [s.invokeMs])) };
    },
  };
}
