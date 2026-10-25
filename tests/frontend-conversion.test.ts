import { describe, expect, it } from 'vitest';
import { commonTargets, conversionError, conversionRetryRequest, createConversionRefresh, createCsvPreviewSession, csvPreviewKey, csvPreviewReady, defaultConversionOptions, loadConversionDraft, mergeConversionSources, optionsForConversionSources, parseConversionQuery, refreshConversionEvent, requiredComponents, saveConversionDraft, validateConversionOptions, clearConversionDraftSources, pendingConversionDraftClear } from '../src/conversion';
import type { ConversionJob, ConversionOptions, CsvPreviewResult, CsvPreviewState } from '../src/conversion';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, refuse) => { resolve = accept; reject = refuse; });
  return { promise, resolve, reject };
}

describe('format conversion commands', () => {
  it('keeps a failed clear retryable and applies a successful clear to the latest draft after remount', async () => {
    const first = { id: 'clear-first', name: 'first.png', extension: 'png', bytes: 20 };
    const second = { ...first, id: 'clear-second' };
    const draft = { sources: [first], directory: { id: 'clear-destination', label: 'output' }, options: { ...defaultConversionOptions('jpg'), quality: 82 } };
    saveConversionDraft(draft);
    const failed = deferred<void>();
    const failure = clearConversionDraftSources([first.id], () => failed.promise);
    expect(pendingConversionDraftClear()).toBe(failure);
    expect(loadConversionDraft()).toEqual(draft);
    failed.reject(new Error('release failed'));
    await expect(failure).rejects.toThrow('release failed');
    expect(pendingConversionDraftClear()).toBeNull();
    expect(loadConversionDraft()).toEqual(draft);
    const reply = deferred<void>();
    let ids: string[] = [];
    const success = clearConversionDraftSources([first.id], selected => { ids = selected; return reply.promise; });
    saveConversionDraft({ ...draft, sources: [first, second] });
    reply.resolve(); await success;
    expect(ids).toEqual([first.id]);
    expect(pendingConversionDraftClear()).toBeNull();
    expect(loadConversionDraft()).toEqual({ ...draft, sources: [second] });
    saveConversionDraft({ sources: [], directory: null, options: defaultConversionOptions() });
  });
  it('recognizes only standalone commands and leaves ordinary filenames searchable', () => {
    for (const query of ['转换', ' convert ', 'CONVERT']) expect(parseConversionQuery(query)).toBe(true);
    for (const query of ['', '转换说明.docx', 'converter.rs', '转换 文件', 'convert picture.png', '我的转换']) expect(parseConversionQuery(query)).toBe(false);
  });
  it('offers only targets compatible with every selected source', () => {
    expect(commonTargets(['png', 'docx'])).toEqual(['pdf']);
    expect(commonTargets(['jpeg', 'tif'])).toContain('webp');
    expect(commonTargets(['xlsx', 'pptx'])).toEqual(['pdf']);
    expect(commonTargets(['exe'])).toEqual([]);
    expect(commonTargets([])).toEqual([]);
  });
  it('de-duplicates native selection grants while allowing removal and re-addition', () => {
    const first = { id: 'file-a', name: 'picture.png', extension: 'png', bytes: 1024 };
    const otherFolder = { ...first, id: 'file-b' };
    expect(mergeConversionSources([first], [first])).toEqual([first]);
    expect(mergeConversionSources([first], [otherFolder])).toEqual([first, otherFolder]);
    expect(mergeConversionSources([], [first])).toEqual([first]);
    expect(validateConversionOptions(['png'], defaultConversionOptions('png'))).toBe('');
    expect(requiredComponents(['png'], defaultConversionOptions('png'))).toEqual([]);
    expect(defaultConversionOptions().target).toBe('png');
  });
  it('keeps the chosen format across removal of the last file and re-addition', () => {
    const chosen = { ...defaultConversionOptions('png'), quality: 82 };
    const empty = optionsForConversionSources([], chosen);
    expect(empty).toEqual(chosen);
    expect(optionsForConversionSources(['png'], empty).target).toBe('png');
    expect(optionsForConversionSources(['docx'], empty).target).toBe('pdf');
    expect(optionsForConversionSources(['png'], empty).quality).toBe(82);
  });
  it('clears irrelevant OCR flags after replacing PDF sources with Office documents', () => {
    const scanned = { ...defaultConversionOptions('pdf'), ocr: true };
    expect(optionsForConversionSources(['docx'], scanned).ocr).toBe(false);
    expect(optionsForConversionSources(['pdf', 'docx'], { ...scanned, target: 'txt' }).ocr).toBe(true);
    expect(requiredComponents(['docx'], scanned)).toEqual(['office']);
  });
  it('clears hidden merge state for incompatible sources and targets but retains compatible and empty choices', () => {
    const selected = { ...defaultConversionOptions('pdf'), merge: true, quality: 82 };
    const mixed = optionsForConversionSources(['png', 'docx'], selected);
    expect(mixed).toMatchObject({ target: 'pdf', merge: false, quality: 82 });
    expect(validateConversionOptions(['png', 'docx'], mixed)).toBe('');
    expect(optionsForConversionSources(['png', 'JPEG'], selected)).toEqual(selected);
    expect(optionsForConversionSources([], selected)).toEqual(selected);
    expect(optionsForConversionSources(['png'], { ...selected, target: 'jpg' }).merge).toBe(false);
    expect(optionsForConversionSources(['docx'], selected).merge).toBe(false);
  });
  it('normalizes restored source selections including newly added menu files', () => {
    const image = { id: 'merge-image', name: 'image.png', extension: 'png', bytes: 20 };
    saveConversionDraft({ sources: [image], directory: null, options: { ...defaultConversionOptions('pdf'), merge: true } });
    expect(loadConversionDraft().options.merge).toBe(true);
    const restored = loadConversionDraft([{ id: 'merge-document', name: 'document.docx', extension: 'docx', bytes: 30 }]);
    expect(restored.options).toMatchObject({ target: 'pdf', merge: false });
    saveConversionDraft({ ...restored, sources: [], options: defaultConversionOptions() });
  });
  it('uses explicit local components for document and OCR routes', () => {
    expect(requiredComponents(['png'], defaultConversionOptions('jpg'))).toEqual([]);
    expect(requiredComponents(['docx'], defaultConversionOptions('pdf'))).toEqual(['office']);
    expect(requiredComponents(['md'], defaultConversionOptions('pdf'))).toEqual(['office', 'pandoc']);
    expect(requiredComponents(['pdf'], { ...defaultConversionOptions('docx'), ocr: true })).toEqual(['pdf', 'ocr']);
  });
  it('rejects invalid options before dispatch and requires a sheet for CSV export', () => {
    expect(validateConversionOptions(['png'], defaultConversionOptions('jpg'))).toBe('');
    expect(validateConversionOptions(['xlsx'], defaultConversionOptions('csv'))).toContain('工作表');
    expect(validateConversionOptions(['xlsx'], { ...defaultConversionOptions('csv'), sheet: 2 })).toBe('');
    expect(validateConversionOptions(['png'], { ...defaultConversionOptions('jpg'), quality: 101 })).toBeTruthy();
    expect(validateConversionOptions(['pdf'], { ...defaultConversionOptions('png'), pages: '1-3,5' })).toBe('');
    expect(validateConversionOptions(['pdf'], { ...defaultConversionOptions('png'), pages: '0,4-2' })).toBeTruthy();
    expect(validateConversionOptions(['png'], { ...defaultConversionOptions('ico'), icoSize: 100 })).toBeTruthy();
    expect(validateConversionOptions(['pdf'], defaultConversionOptions('pdf'))).toContain('文字识别');
    expect(validateConversionOptions(['png'], defaultConversionOptions('docx'))).toContain('文字识别');
    expect(validateConversionOptions(['png'], { ...defaultConversionOptions('docx'), ocr: true })).toBe('');
  });
  it('defaults and validates CSV encoding and delimiter without accepting arbitrary filter text', () => {
    const options = defaultConversionOptions('xlsx');
    expect(options.csvDelimiter).toBe(',');
    expect(options.csvEncoding).toBe('utf-8');
    for (const csvDelimiter of [',', ';', '\t']) for (const csvEncoding of ['utf-8', 'gb18030', 'utf-16le', 'utf-16be']) expect(validateConversionOptions(['csv'], { ...options, csvDelimiter, csvEncoding })).toBe('');
    expect(validateConversionOptions(['csv'], { ...options, csvDelimiter: '44,34,76,1' })).toContain('分隔符');
    expect(validateConversionOptions(['csv'], { ...options, csvEncoding: 'auto' })).toContain('编码');
    const { csvDelimiter: _delimiter, csvEncoding: _encoding, ...legacy } = options;
    const restored = optionsForConversionSources(['csv'], legacy as ConversionOptions);
    expect(restored.csvDelimiter).toBe(',');
    expect(restored.csvEncoding).toBe('utf-8');
  });
  it('localizes component, capacity and source failures without exposing raw internals', () => {
    expect(conversionError('Conversion disabled')).toContain('扩展');
    expect(conversionError('Conversion component missing')).toContain('组件');
    expect(conversionError('Conversion queue full')).toContain('队列');
    expect(conversionError('No tables detected')).toContain('表格');
    expect(conversionError('预览模式仅展示样例，此操作需要桌面应用。')).toBe('浏览器预览仅展示样例，请在桌面应用中操作。');
    expect(conversionError('private C:\\Users\\name\\file')).not.toContain('Users');
  });
  it('keeps installer download and archive failures distinct from a missing conversion dependency', () => {
    expect(conversionError('Conversion component download unavailable')).toContain('下载失败');
    for (const raw of ['Conversion component entry missing', 'Conversion component manifest missing', 'Conversion unsafe package entry', 'Conversion invalid package ZIP']) {
      expect(conversionError(raw)).toContain('组件包');
      expect(conversionError(raw)).not.toContain('请先安装');
    }
    expect(conversionError('Conversion component size mismatch')).toContain('校验失败');
    expect(conversionError('Conversion component package is not published in this build')).toContain('未提供');
    expect(conversionError('Conversion component requires local package')).toContain('随附');
    expect(conversionError('Conversion installer launched; finish installation and refresh')).toContain('安装程序已打开');
    expect(conversionError('Conversion component pandoc missing')).toBe('请先安装此转换所需的本地组件。');
  });
  it('shows actionable transport context without exposing download URLs', () => {
    expect(conversionError('Conversion component download request: HTTP 403')).toContain('服务器拒绝');
    expect(conversionError('Conversion component download request: HTTP 404')).toContain('地址暂不可用');
    expect(conversionError('Conversion component download body: network timeout')).toContain('下载超时');
    expect(conversionError('Conversion component download request: redirect rejected')).toContain('重定向');
  });
});

describe('conversion retry credentials', () => {
  const failed: ConversionJob = { id: 'original-job', status: 'failed', target: 'xlsx', completed: 1, total: 2, options: { ...defaultConversionOptions('xlsx'), csvDelimiter: ';', csvEncoding: 'gb18030' }, items: [{ sourceId: 'original-csv', name: 'source.csv', status: 'failed', outputs: [], error: 'timeout' }, { sourceId: 'done-csv', name: 'done.csv', status: 'completed', outputs: [], error: '' }] };
  it('selects only original failed item credentials without draft destination or CSV options', () => {
    expect(conversionRetryRequest(failed, 'original-csv')).toEqual({ jobId: 'original-job', sourceId: 'original-csv' });
    for (const sourceId of ['done-csv', 'unrelated', null]) expect(conversionRetryRequest(failed, sourceId)).toBeNull();
    for (const status of ['queued', 'running', 'completed', 'cancelled'] as const) expect(conversionRetryRequest({ ...failed, status }, 'original-csv')).toBeNull();
  });
  it('retries a merged failure only as one original batch', () => {
    const merged = { ...failed, target: 'pdf', options: { ...defaultConversionOptions('pdf'), merge: true } };
    expect(conversionRetryRequest(merged, null)).toEqual({ jobId: 'original-job', sourceId: null });
    expect(conversionRetryRequest(merged, 'original-csv')).toBeNull();
    expect(conversionRetryRequest({ ...merged, status: 'running' }, null)).toBeNull();
  });
  it('localizes expired task and invalid retry state failures', () => {
    expect(conversionError('Conversion job unavailable')).toContain('任务已失效');
    expect(conversionError('Conversion retry requires failed terminal job')).toContain('仅可重试');
  });
});

describe('conversion refresh lifecycle', () => {
  it('preserves RAM drafts and does not resurrect removed menu sources', () => {
    const source = { id: 'menu-grant', name: 'note.txt', extension: 'txt', bytes: 20 };
    const selection = [source];
    const draft = loadConversionDraft(selection);
    expect(draft.sources).toContainEqual(source);
    saveConversionDraft({ ...draft, sources: [], options: defaultConversionOptions('html') });
    const restored = loadConversionDraft(selection);
    expect(restored.sources).toEqual([]);
    expect(restored.options.target).toBe('html');
    restored.options.target = 'pdf';
    expect(loadConversionDraft().options.target).toBe('html');
    expect(loadConversionDraft([source]).sources).toEqual([source]);
  });
  it('coalesces a burst into one serial follow-up and makes every caller await the latest value', async () => {
    const replies = [deferred<number>(), deferred<number>()];
    const followupStarted = deferred<void>();
    let reads = 0;
    const seen: number[] = [];
    const refresh = createConversionRefresh(() => { const reply = replies[reads++]; if (reads === 2) followupStarted.resolve(); return reply.promise; }, value => seen.push(value), () => {});
    const first = refresh.run();
    const burst = Array.from({ length: 20 }, () => refresh.run());
    expect(reads).toBe(1);
    for (const call of burst) expect(call).toBe(first);
    let settled = false;
    void first.then(() => { settled = true; });
    replies[0].resolve(1);
    await followupStarted.promise;
    expect(reads).toBe(2);
    expect(seen).toEqual([]);
    expect(settled).toBe(false);
    replies[1].resolve(2);
    await Promise.all([first, ...burst]);
    expect(reads).toBe(2);
    expect(seen).toEqual([2]);
    expect(settled).toBe(true);
  });
  it('suppresses superseded errors, recovers after failures, and ignores disposed replies and queued work', async () => {
    const replies = [deferred<number>(), deferred<number>(), deferred<number>(), deferred<number>()];
    const followupStarted = deferred<void>();
    let reads = 0;
    const seen: number[] = [];
    const errors: unknown[] = [];
    const refresh = createConversionRefresh(() => { const reply = replies[reads++]; if (reads === 2) followupStarted.resolve(); return reply.promise; }, value => seen.push(value), value => errors.push(value));
    const first = refresh.run(); refresh.run();
    replies[0].reject('stale error');
    await followupStarted.promise;
    replies[1].reject('current error'); await first;
    expect(errors).toEqual(['current error']);
    const recovered = refresh.run(); replies[2].resolve(3); await recovered;
    expect(seen).toEqual([3]);
    const disposed = refresh.run(); refresh.run(); refresh.dispose(); replies[3].resolve(4); await disposed;
    await refresh.run();
    expect(reads).toBe(4);
    expect(seen).toEqual([3]);
    expect(errors).toEqual(['current error']);
  });
  it('does not lose a refresh arriving in the completion microtask', async () => {
    let reads = 0;
    const seen: number[] = [];
    const refresh = createConversionRefresh(async () => ++reads, value => seen.push(value), () => {});
    const first = refresh.run();
    await Promise.resolve();
    const second = refresh.run();
    await Promise.all([first, second]);
    expect(reads).toBe(2);
    expect(seen).toEqual([1, 2]);
  });
  it('routes each native event to only its own resource', async () => {
    let jobReads = 0;
    let componentReads = 0;
    const jobs = createConversionRefresh(async () => ++jobReads, () => {}, () => {});
    const components = createConversionRefresh(async () => ++componentReads, () => {}, () => {});
    await refreshConversionEvent('conversions_changed', jobs, components);
    expect([jobReads, componentReads]).toEqual([1, 0]);
    await refreshConversionEvent('conversion_components_changed', jobs, components);
    expect([jobReads, componentReads]).toEqual([1, 1]);
  });
  it('continues job updates while disabled without querying disabled components', async () => {
    let jobReads = 0;
    let componentReads = 0;
    const jobs = createConversionRefresh(async () => ++jobReads, () => {}, () => {});
    const components = createConversionRefresh(async () => ++componentReads, () => {}, () => {});
    await refreshConversionEvent('conversion_components_changed', jobs, components, false);
    expect([jobReads, componentReads]).toEqual([0, 0]);
    await refreshConversionEvent('conversions_changed', jobs, components, false);
    expect([jobReads, componentReads]).toEqual([1, 0]);
  });
});

describe('CSV preview lifecycle', () => {
  const sources = [{ id: 'csv-a', name: 'data.csv', extension: 'csv', bytes: 128 }, { id: 'csv-b', name: 'more.csv', extension: 'csv', bytes: 128 }];
  it('previews every CSV serially and publishes bounded cells as text', async () => {
    const requests: string[] = [];
    let state: CsvPreviewState | undefined;
    const session = createCsvPreviewSession(async request => { requests.push(request.sourceId); return { rows: [['<script>alert(1)</script>', '=1+1']], truncated: false }; }, value => { state = value; });
    await session.preview(sources, ';', 'gb18030');
    expect(requests).toEqual(['csv-a', 'csv-b']);
    expect(state?.pending).toBe(false);
    expect(state?.completed).toBe(2);
    expect(state?.key).toBe(csvPreviewKey(sources, ';', 'gb18030'));
    expect(state?.results['csv-a'].rows[0]).toEqual(['<script>alert(1)</script>', '=1+1']);
    expect(csvPreviewReady(state!, sources, ';', 'gb18030')).toBe(true);
    expect(csvPreviewReady(state!, sources, ',', 'gb18030')).toBe(false);
    expect(csvPreviewReady(state!, sources, ';', 'utf-8')).toBe(false);
    expect(csvPreviewReady(state!, [sources[0]], ';', 'gb18030')).toBe(false);
  });
  it('drops preview replies after settings change or disposal', async () => {
    const replies: ((result: CsvPreviewResult) => void)[] = [];
    let state: CsvPreviewState | undefined;
    const session = createCsvPreviewSession(() => new Promise(resolve => replies.push(resolve)), value => { state = value; });
    const pending = session.preview([sources[0]], ',', 'utf-8');
    session.reset();
    replies[0]({ rows: [['stale']], truncated: false }); await pending;
    expect(state?.results).toEqual({});
    expect(state?.key).toBe('');
    const next = session.preview([sources[1]], '\t', 'utf-16le');
    session.dispose(); replies[1]({ rows: [['late']], truncated: false }); await next;
    expect(state?.results).toEqual({});
  });
  it('marks a failed file as unreviewed without passing a partially successful batch', async () => {
    let state: CsvPreviewState | undefined;
    const session = createCsvPreviewSession(async request => { if (request.sourceId === 'csv-b') throw new Error('CSV invalid encoding'); return { rows: [['name']], truncated: false }; }, value => { state = value; });
    await session.preview(sources, ',', 'utf-8');
    expect(state?.completed).toBe(1);
    expect(state?.error).toContain('编码');
    expect(state?.results['csv-b']).toBeUndefined();
    expect(csvPreviewReady(state!, sources, ',', 'utf-8')).toBe(false);
  });
  it('rejects an oversized CSV before reading it and bounds preview rows, columns and text', async () => {
    let reads = 0;
    let state: CsvPreviewState | undefined;
    const session = createCsvPreviewSession(async () => { reads++; return { rows: Array.from({ length: 21 }, () => Array.from({ length: 21 }, () => 'x'.repeat(2100))), truncated: false }; }, value => { state = value; });
    await session.preview([{ ...sources[0], bytes: 32 * 1024 * 1024 + 1 }], ',', 'utf-8');
    expect(reads).toBe(0);
    expect(state?.error).toContain('32 MiB');
    await session.preview([sources[0]], ',', 'utf-8');
    const result = state?.results['csv-a'];
    expect(result?.rows).toHaveLength(20);
    expect(result?.rows[0]).toHaveLength(20);
    expect(result?.rows[0][0]).toHaveLength(2048);
    expect(result?.truncated).toBe(true);
  });
});
