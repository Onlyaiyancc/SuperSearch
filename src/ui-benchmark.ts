import type { TimingSample } from './search-metrics';

export type BenchmarkConfig = { uniqueQuery: string; uniqueTitle: string; broadQuery: string; completionQuery: string; rankingQuery: string; rankingTitle: string; minimumBroadResults: number; setupToFrontendMs: number; fixtureFiles?: number; coldTimeoutMs?: number; soakSeconds?: number | null };
type RenderedSearch = { sample: TimingSample | null; complete: boolean; count: number; firstTitle: string; selectedTitle: string; candidates: number; ready: boolean };
export function summarizeTimings(samples: TimingSample[]) {
  const summarize = (values: number[]) => {
    const sorted = values.sort((a, b) => a - b);
    return { samples: sorted.length, p50Ms: sorted[Math.ceil(sorted.length * .5) - 1] ?? null,
      p95Ms: sorted[Math.ceil(sorted.length * .95) - 1] ?? null, maximumMs: sorted[sorted.length - 1] ?? null };
  };
  // 主结果与最终补取分别计时；无结果样本不伪造首屏耗时。
  return { ...summarize(samples.map(sample => sample.settledMs)),
    firstResult: summarize(samples.flatMap(sample => sample.firstResultMs === null ? [] : [sample.firstResultMs])), timings: samples };
}

export async function runUiBenchmark(config: BenchmarkConfig, port: { search: (raw: string) => string; current: (id: string) => RenderedSearch | null; finish: (report: unknown) => Promise<unknown>; diagnostics?: () => unknown }) {
  const started = performance.now();
  const pause = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
  const groups: Record<string, TimingSample[]> = { unique: [], broad: [], completion: [], exactBeyondPage: [], rapid: [], overlapping: [] };
  let coldReadyMs = 0;
  let stage = 'cold';
  let lastState: RenderedSearch | null = null;
  try {
    const wait = async (id: string) => {
      const begin = performance.now();
      while (performance.now() - begin < 4000) {
        const result = port.current(id);
        lastState = result;
        if (result?.complete && result.sample) return result;
        await pause(10);
      }
      throw new Error('UI render did not settle within four seconds');
    };
    // 首次索引可用单独记录，不把启动重试混入暖查询的分位数。
    let cold: RenderedSearch;
    while (true) {
      cold = await wait(port.search(config.uniqueQuery));
      if (cold.ready && cold.firstTitle === config.uniqueTitle) break;
      if (performance.now() - started > (config.coldTimeoutMs ?? 120000)) throw new Error('Cold engine readiness exceeded its configured timeout');
      await pause(120);
    }
    coldReadyMs = Math.round(performance.now() - started);
    for (let index = 0; index < 20; index++) {
      stage = `unique-${index}`;
      const unique = await wait(port.search(config.uniqueQuery));
      if (!unique.ready || unique.firstTitle !== config.uniqueTitle || unique.selectedTitle !== config.uniqueTitle) throw new Error('Exact filename did not rank first or become the default selection');
      groups.unique.push(unique.sample!);
      stage = `broad-${index}`;
      const broad = await wait(port.search(config.broadQuery));
      if (!broad.ready || broad.count < config.minimumBroadResults) throw new Error('Broad-result fixture did not return its expected bounded results');
      groups.broad.push(broad.sample!);
    }
    stage = 'completion';
    const completion = await wait(port.search(config.completionQuery));
    if (!completion.ready || !completion.candidates) throw new Error('Indexed folder completion was not displayed');
    groups.completion.push(completion.sample!);
    stage = 'exactBeyondPage';
    const ranking = await wait(port.search(config.rankingQuery));
    if (!ranking.ready || ranking.firstTitle !== config.rankingTitle || ranking.selectedTitle !== config.rankingTitle) throw new Error('Exact filename beyond the alphabetical first page was omitted or not selected');
    groups.exactBeyondPage.push(ranking.sample!);
    let finalId = '';
    stage = 'rapid';
    for (let index = 0; index < 12; index++) {
      finalId = port.search(index === 11 ? config.uniqueQuery : `${config.broadQuery}-${index}`);
      await pause(5);
    }
    const rapid = await wait(finalId);
    if (!rapid.ready || rapid.firstTitle !== config.uniqueTitle) throw new Error('Rapid input displayed a superseded result');
    groups.rapid.push(rapid.sample!);
    // 45 毫秒间隔超过防抖时间，也覆盖已有 IPC 返回与新输入交错的情况。
    stage = 'overlapping';
    for (let index = 0; index < 6; index++) {
      finalId = port.search(index === 5 ? config.uniqueQuery : config.broadQuery);
      await pause(45);
    }
    const overlapping = await wait(finalId);
    if (!overlapping.ready || overlapping.firstTitle !== config.uniqueTitle) throw new Error('Overlapping input displayed a superseded result');
    groups.overlapping.push(overlapping.sample!);
    const summaries = Object.fromEntries(Object.entries(groups).map(([name, samples]) => [name, summarizeTimings(samples)]));
    await port.finish({ outcome: 'passed', measurement: 'Synthetic input through real Svelte debounce, Tauri IPC, snapshot updates, native window layout and two animation frames. Frame timing is a render proxy, not physical display or hardware key latency.', fixtureFiles: config.fixtureFiles ?? 483, coldReadyMs, setupToFrontendMs: config.setupToFrontendMs, scenarios: summaries, rapidInputs: 12, overlappingInputs: 6, exactBeyondFirstPageVerified: true, defaultSelectionVerified: true, finalQueryVerified: true });
  } catch (error) {
    await port.finish({ outcome: 'failed', error: error instanceof Error ? error.message : 'UI benchmark failed', stage, lastState, uiState: port.diagnostics?.(), coldReadyMs, elapsedMs: Math.round(performance.now() - started) });
  }
}

export type SoakWindow = { visible: boolean; logicalHeight: number; widthPx: number; heightPx: number; hiddenVerified: boolean; hiddenMs: number };
type SoakPort = { search: (raw: string) => string; current: (id: string) => RenderedSearch | null;
  compact: () => Promise<unknown>; window: (hiddenMs?: number) => Promise<SoakWindow>;
  finish: (report: unknown) => Promise<unknown>; diagnostics?: () => unknown };

export async function runUiSoak(config: BenchmarkConfig, port: SoakPort) {
  const startedUnixMs = Date.now();
  const started = performance.now();
  const pause = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
  const elapsed = () => Math.round(performance.now() - started);
  const phases: { name: string; cycle: number; startMs: number; endMs: number; window?: SoakWindow }[] = [];
  const samples: TimingSample[] = [];
  const counts = { compact: 0, hiddenIdle: 0, visibleIdle: 0, unique: 0, broad: 0 };
  let coldReadyMs = 0;
  let cycles = 0;
  let stage = 'cold';
  let soakStartedMs: number | null = null;
  let lastState: RenderedSearch | null = null;
  const requestedDurationMs = (config.soakSeconds ?? 1800) * 1000;
  const phase = async (name: string, action: () => Promise<SoakWindow | void>) => {
    stage = name;
    const startMs = elapsed();
    const window = await action();
    phases.push({ name, cycle: cycles + 1, startMs, endMs: elapsed(), ...(window ? { window } : {}) });
  };
  const wait = async (id: string) => {
    const begin = performance.now();
    while (performance.now() - begin < 4000) {
      lastState = port.current(id);
      if (lastState?.complete && lastState.sample) return lastState;
      await pause(10);
    }
    throw new Error('UI render did not settle within four seconds');
  };
  const compact = async () => {
    await port.compact();
    const window = await port.window();
    if (!window.visible || Math.abs(window.logicalHeight - 76) > 2 || window.widthPx <= 0) throw new Error('Native compact window was not visible or had unexpected geometry');
    counts.compact++;
    return window;
  };
  try {
    // 首次索引就绪单独计时，完整浸泡时长从冷启动完成后开始。
    while (true) {
      const cold = await wait(port.search(config.uniqueQuery));
      if (cold.ready && cold.firstTitle === config.uniqueTitle && cold.selectedTitle === config.uniqueTitle) break;
      if (performance.now() - started > (config.coldTimeoutMs ?? 120000)) throw new Error('Cold engine readiness exceeded its configured timeout');
      await pause(120);
    }
    coldReadyMs = elapsed();
    soakStartedMs = elapsed();
    const deadline = performance.now() + requestedDurationMs;
    while (performance.now() < deadline) {
      await phase('compact', compact);
      // 一次隐藏与一次可见空闲组成一轮；短验收保留两种空闲阶段。
      const hiddenMs = Math.max(1, Math.min(15000, Math.floor((deadline - performance.now()) / 2)));
      await phase('hidden-idle', async () => {
        const window = await port.window(hiddenMs);
        if (!window.hiddenVerified || !window.visible || Math.abs(window.logicalHeight - 76) > 2) throw new Error('Native hide/show cycle did not restore a visible compact window');
        counts.hiddenIdle++;
        return window;
      });
      for (const name of ['unique', 'broad'] as const) {
        await phase(`${name}-search`, async () => {
          const result = await wait(port.search(name === 'unique' ? config.uniqueQuery : config.broadQuery));
          if (!result.ready || (name === 'unique' ? result.firstTitle !== config.uniqueTitle || result.selectedTitle !== config.uniqueTitle : result.count < config.minimumBroadResults)) throw new Error(`Soak ${name} query failed its result assertion`);
          const window = await port.window();
          if (!window.visible || window.logicalHeight <= 78 || window.widthPx <= 0) throw new Error('Native results window did not expand');
          samples.push(result.sample!);
          counts[name]++;
          return window;
        });
      }
      await phase('compact', compact);
      await phase('visible-idle', async () => {
        // 空闲期间只等待一次计时器，无搜索、状态查询、渲染轮询或资源采样。
        const idleMs = Math.max(0, Math.min(15000, deadline - performance.now()));
        await pause(idleMs);
        counts.visibleIdle++;
      });
      cycles++;
    }
    await port.finish({ outcome: 'passed', startedUnixMs, soakStartedMs, requestedDurationMs,
      soakElapsedMs: elapsed() - soakStartedMs, elapsedMs: elapsed(), coldReadyMs,
      coldTimeoutMs: config.coldTimeoutMs ?? 120000, setupToFrontendMs: config.setupToFrontendMs,
      fixtureFiles: config.fixtureFiles ?? 483, cycles, counts, phases, searches: summarizeTimings(samples),
      protocol: 'Each cycle compacts, idles hidden for up to 15 seconds using a native wait, restores through service show and acknowledged events, searches unique and broad fixtures, compacts and idles visible for up to 15 seconds. Final idle waits are shortened to the remaining duration.',
      limitations: ['Synthetic isolated fixture index and temporary store; does not exercise installed service, personal data or full disks.',
        'Long soak windows are non-focusable and ignore manual UI input; ordinary foreground focus and physical keyboard interaction are covered separately, not by this soak.',
        'Native visibility and geometry plus two animation frames are render proxies; no physical key or display latency measurement.',
        'Single active monitor; does not establish multi-monitor or installation lifecycle correctness.',
        'Resource usage must be collected externally; this protocol does not sample CPU, memory or handles during idle.'] });
  } catch (error) {
    await port.finish({ outcome: 'failed', error: error instanceof Error ? error.message : 'UI soak failed',
      stage, startedUnixMs, soakStartedMs, requestedDurationMs, elapsedMs: elapsed(), coldReadyMs,
      fixtureFiles: config.fixtureFiles ?? 483, cycles, counts, phases, lastState, uiState: port.diagnostics?.() });
  }
}
