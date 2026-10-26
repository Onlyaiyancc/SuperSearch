<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Search, Settings as SettingsIcon, X, ArrowLeft, ArrowUpRight, Folder, Pin, MoreHorizontal, Command, RefreshCw, Plus, Check, AlertCircle, Copy, Keyboard, Monitor, Download, Sun, Moon, Database, Puzzle, Languages, FileSearch, Bell, ArrowRightLeft, ScanLine } from 'lucide-svelte';
  import { call, preview, subscribe } from './bridge';
  import { acceptSnapshot, blocksIme, nextScope, scopes, selectedEntity, nextRequestId, mergeProviders, enterAction, hasDirectoryQuery } from './search-state';
  import { emptySearchSession, searchLayout, createLayoutSync, menuPosition } from './search-layout';
  import { localizeMessage, providerMessage } from './localization';
  import { scopeDescription, subtitleParts, highlightSegments, createCompletionCycle } from './search-presentation';
  import { createSearchMetrics } from './search-metrics';
  import { captureShortcut, parseAliases } from './settings-input';
  import { watchLoadingProvider } from './provider-refresh';
  import { runUiBenchmark, runUiSoak, type BenchmarkConfig, type SoakWindow } from './ui-benchmark';
  import BrandMark from './BrandMark.svelte';
  import ResultIcon from './ResultIcon.svelte';
  import TranslationPanel from './TranslationPanel.svelte';
  import { parseTranslationQuery } from './translation';
  import FilePreviewPanel from './FilePreviewPanel.svelte';
  import ReminderPanel from './ReminderPanel.svelte';
  import ConversionPanel from './ConversionPanel.svelte';
  import QrScanPanel from './QrScanPanel.svelte';
  import { isQrScanQuery, qrScanError } from './qr-scan';
  import { parseConversionQuery, conversionError, type ConversionSource } from './conversion';
  import { parseReminderQuery } from './reminders';
  import { previewShortcut } from './file-preview';
  import type { WindowLayout } from './search-layout';
  import type { Provider, ResultItem, SearchSnapshot, Scope, Settings } from './types';

  let query = $state('');
  let scope = $state<Scope>('all');
  let snapshot = $state<SearchSnapshot>({ requestId: '', snapshotRevision: -1, scope: 'all', complete: false, items: [], providers: [], truncated: false, hasMore: false });
  let providers = $state<Provider[]>([]);
  let selected = $state<string | null>(null);
  let selectionEstablished = false;
  let pending = $state(false);
  let error = $state('');
  let notice = $state('');
  let busy = $state(false);
  let composing = $state(false);
  let compositionEndedAt = 0;
  const requestSession = `q-${Date.now().toString(36)}`;
  let requestEpoch = 0;
  let uiEpoch = 0;
  let providerProbe = 0;
  let settingsLoad = 0;
  let requestId = $state('');
  let scheduledKey = '';
  let debounce: ReturnType<typeof setTimeout>;
  let input = $state<HTMLInputElement>();
  let results = $state<HTMLDivElement>();
  let view = $state<'search' | 'settings'>('search');
  let settings = $state<Settings | null>(null);
  let draft = $state<Settings | null>(null);
  let includeText = $state('');
  let excludeText = $state('');
  let settingsTab = $state<'general' | 'search' | 'extensions' | 'advanced'>('general');
  let translationPanel = $state<{ submit: () => Promise<void> }>();
  let reminderPanel = $state<{ submit: () => Promise<void> }>();
  let conversionPanel = $state<{ submit: () => Promise<void> }>();
  let qrPanel = $state<{ submit: () => Promise<void>; scan: () => Promise<void>; cancel: () => void; move: (direction: number) => void }>();
  let qrShortcutError = $state('');
  let qrShortcutInput = $state<HTMLInputElement>();
  let qrDetails = $state<HTMLDetailsElement>();
  let conversionOpen = $state(false);
  let conversionSources = $state<ConversionSource[]>([]);
  let previewItem = $state<ResultItem | null>(null);
  let previewHeight = $state(320);
  let shortcutError = $state('');
  let shortcutRecording = $state(false);
  let shortcutInput = $state<HTMLInputElement>();
  let aliasesInput = $state<HTMLInputElement>();
  let menu = $state<string | null>(null);
  let menuLocation = $state({ left: 8, top: 8 });
  let menuPlaced = $state(false);
  let modal = $state<'add' | 'aliases' | 'clear' | 'diagnostics' | null>(null);
  let dialogName = $state('');
  let dialogAliases = $state('');
  let originalAliases = $state<string[]>([]);
  let dialogAppId = '';
  let diagnostics = $state('');
  let lastFocused: HTMLElement | null = null;
  let dialogElement = $state<HTMLDialogElement>();
  let hidden = $state(false);
  let documentVisible = $state(true);
  const completionCycle = createCompletionCycle();
  const metrics = createSearchMetrics(preview ? 'preview' : 'desktop');
  let benchmarkRunning = $state(false);
  const scopeLabels: Record<Scope, string> = { all: '全部', apps: '应用', files: '文件', folders: '文件夹' };
  const translationQuery = $derived(parseTranslationQuery(query));
  const reminderQuery = $derived(parseReminderQuery(query));
  const conversionQuery = $derived(conversionOpen || parseConversionQuery(query));
  const qrQuery = $derived(isQrScanQuery(query));
  const pluginQuery = $derived(!!translationQuery || !!reminderQuery || conversionQuery || qrQuery);
  const layout = $derived<WindowLayout>(previewItem && view === 'search' && !modal
    ? { layout: 'preview', rows: 0, requestId, height: previewHeight }
    : searchLayout(query, requestId, snapshot, pending, view === 'settings' || !!modal || pluginQuery));
  const expanded = $derived(layout.layout === 'results');
  const syncLayout = createLayoutSync(value => call('sync_window_layout', value));
  const activeItem = $derived(snapshot.items.find(item => item.entityId === selected));
  const actionable = $derived(!pending && !composing && !busy && !!activeItem?.available);
  const fileProvider = $derived(providers.find(provider => provider.id === 'everything'));
  const watchIndexLoading = $derived(!preview && !benchmarkRunning && !hidden && documentVisible && view === 'settings' && settingsTab === 'advanced' && (fileProvider?.status === 'indexLoading' || fileProvider?.status === 'loading'));
  const unavailableProvider = $derived(providers.find(provider => provider.status !== 'ready'));
  const directoryQuery = $derived(hasDirectoryQuery(query));
  const directoryScope = $derived(scopeDescription(snapshot.requestId === requestId ? snapshot.directory : null));
  const folderCompletions = $derived(!pending && snapshot.requestId === requestId ? (snapshot.folderCompletions ?? []).slice(0, 5) : []);
  const inlineStatus = $derived(pluginQuery ? '' : error || notice || (query.trim() && pending ? '搜索中…' : query.trim() && !snapshot.items.length ? folderCompletions.length ? `${folderCompletions.length} 个文件夹可补全` : directoryQuery ? '未找到此文件夹内的匹配文件' : '无匹配结果' : !preview && unavailableProvider ? providerMessage(unavailableProvider) : ''));
  const webFallback = $derived(!pluginQuery && !!query.trim() && !directoryQuery && !pending && snapshot.complete && snapshot.requestId === requestId && !snapshot.items.length && !composing && !busy);
  const theme = $derived(settings?.theme ?? 'light');
  const footerStatus = $derived(error || notice || (snapshot.truncated ? '结果数量已达上限，请缩小搜索范围。' : !preview && unavailableProvider ? providerMessage(unavailableProvider) : ''));
  const aliasInput = $derived(parseAliases(dialogAliases, originalAliases));
  const aliasError = $derived(({ count: '最多设置 16 个不同别名，请减少后保存。', length: '每个别名最多 128 个字符，请缩短后保存。', control: '别名不能包含换行、制表符等控制字符。', '': '' })[aliasInput.error]);

  const message = localizeMessage;

  $effect(() => {
    // 只在索引页可见且加载中检查；离开、隐藏、就绪或卸载都会取消计时和迟到回包。
    if (watchIndexLoading) return watchLoadingProvider(refreshProviderStatus);
  });

  $effect(() => {
    const epoch = uiEpoch;
    const target = { ...layout };
    if (!hidden) void syncLayout(target).catch(value => { if (epoch === uiEpoch) error = message(value); });
  });

  function applySnapshot(incoming: SearchSnapshot) {
    if (!acceptSnapshot(snapshot, incoming)) return;
    completionCycle.update(incoming.requestId, requestId, incoming.folderCompletions ?? []);
    selected = selectedEntity(incoming.items, selected, selectionEstablished);
    snapshot = incoming;
    providers = mergeProviders(providers, incoming.providers);
    pending = false;
    void measureRenderedSnapshot(incoming);
  }

  async function measureRenderedSnapshot(incoming: SearchSnapshot) {
    await tick();
    if (hidden || view !== 'search' || incoming.requestId !== requestId || snapshot.snapshotRevision !== incoming.snapshotRevision) return;
    try { await syncLayout({ ...layout }); }
    catch { return; }
    // DOM 更新及原生尺寸确认后等两帧；这是显示时机的近似值，不代表物理屏幕扫描。
    await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    if (hidden || view !== 'search' || incoming.requestId !== requestId || snapshot.snapshotRevision !== incoming.snapshotRevision) return;
    metrics.rendered(incoming.requestId, performance.now(), { complete: incoming.complete, count: incoming.items.length + (incoming.folderCompletions?.length ?? 0) });
  }

  async function refreshProviderStatus(isCurrent = () => true) {
    const epoch = uiEpoch;
    const probe = ++providerProbe;
    try {
      const status = await call<Provider[]>('get_provider_status');
      if (epoch !== uiEpoch || probe !== providerProbe || !isCurrent()) return;
      providers = mergeProviders(providers, status);
      return status;
    } catch (value) {
      if (epoch !== uiEpoch || probe !== providerProbe || !isCurrent()) return;
      error = message(value);
      return [];
    }
  }

  // 输入变化时先取消可执行状态；防抖期间的 Enter 不会打开旧结果。
  function invalidate(retainCompletion = false) {
    qrPanel?.cancel();
    previewItem = null;
    clearTimeout(debounce);
    if (!retainCompletion) completionCycle.reset();
    const previous = requestId;
    metrics.cancel(previous);
    requestId = nextRequestId(requestSession, ++requestEpoch);
    snapshot = { ...snapshot, requestId, snapshotRevision: -1, scope, complete: false, items: [], directory: null, folderCompletions: [], truncated: false, hasMore: false };
    pending = true;
    notice = '';
    selected = null;
    selectionEstablished = false;
    menu = null;
    error = '';
    if (previous) void call('cancel_search', { requestId: previous }).catch(() => {});
  }

  function scheduleSearch(force = false, retainCompletion = false) {
    const key = `${scope}\0${query}`;
    if (!force && scheduledKey === key) return;
    scheduledKey = key;
    invalidate(retainCompletion);
    // 明确的扩展命令独立处理，输入过程不触发本地查询或联网兜底。
    if (parseTranslationQuery(query) || parseReminderQuery(query) || parseConversionQuery(query) || isQrScanQuery(query)) { pending = false; snapshot = { ...snapshot, complete: true }; return; }
    if (!query.trim()) { pending = false; snapshot = { ...snapshot, complete: true }; return; }
    if (composing || hidden) return;
    const id = requestId;
    const rawQuery = query;
    const searchScope = scope;
    metrics.begin(id, performance.now());
    debounce = setTimeout(async () => {
      metrics.dispatched(id, performance.now());
      try {
        const incoming = await call<SearchSnapshot>('start_search', { requestId: id, rawQuery, scope: searchScope });
        metrics.invoked(id, performance.now());
        applySnapshot(incoming);
      } catch (value) {
        if (id !== requestId) return;
        error = message(value);
        pending = false;
        snapshot = { ...snapshot, complete: true, items: [] };
        void measureRenderedSnapshot(snapshot);
      }
    }, 30);
  }

  function changeScope(value: Scope) {
    scope = value;
    scheduleSearch();
    input?.focus();
  }

  function completeFolder(value: string, cycling = false) {
    query = value;
    scheduleSearch(true, cycling);
    input?.focus();
    void tick().then(() => input?.setSelectionRange(query.length, query.length));
  }

  function moveSelection(direction: number) {
    if (pending || !snapshot.items.length) return;
    const position = snapshot.items.findIndex(item => item.entityId === selected);
    const index = position < 0 ? (direction > 0 ? 0 : snapshot.items.length - 1) : (position + direction + snapshot.items.length) % snapshot.items.length;
    selected = snapshot.items[index].entityId;
    selectionEstablished = true;
    void tick().then(() => results?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' }));
  }

  async function openResultMenu(item: ResultItem, anchor: HTMLElement | undefined, keyboard = false) {
    if (menu === item.resultId && !keyboard) { menu = null; return; }
    if (!anchor) return;
    const bounds = anchor.getBoundingClientRect();
    selected = item.entityId;
    selectionEstablished = true;
    menuPlaced = false;
    menu = item.resultId;
    await tick();
    if (menu !== item.resultId) return;
    const element = document.querySelector<HTMLDivElement>('.action-menu');
    if (!element) return;
    menuLocation = menuPosition(bounds, element.getBoundingClientRect(), { width: window.innerWidth, height: window.innerHeight });
    menuPlaced = true;
    if (keyboard) {
      await tick();
      if (menu === item.resultId) element.querySelector<HTMLButtonElement>('button')?.focus();
    }
  }

  async function execute(item: ResultItem | undefined, action = 'open') {
    if (!item || pending || composing || busy || !item.available || !item.actions.includes(action)) return;
    busy = true;
    menu = null;
    error = '';
    const epoch = uiEpoch;
    const id = requestId;
    try {
      await call('execute_action', { requestId: id, resultId: item.resultId, action });
      if (epoch !== uiEpoch || id !== requestId) return;
      if (action === 'copyPath') notice = '路径已复制到剪贴板';
    } catch (value) { if (epoch === uiEpoch && id === requestId) error = message(value); }
    finally { if (epoch === uiEpoch) busy = false; }
  }

  async function searchWeb() {
    if (busy || !query.trim() || blocksIme({}, composing, compositionEndedAt, performance.now())) return;
    const rawQuery = query;
    const epoch = uiEpoch;
    const id = requestId;
    busy = true;
    menu = null;
    error = '';
    try { await call('search_web', { rawQuery }); }
    catch (value) { if (epoch === uiEpoch && id === requestId && rawQuery === query) error = message(value); }
    finally { if (epoch === uiEpoch) busy = false; }
  }

  function openPreview(item: ResultItem) {
    if (!settings?.filePreviewEnabled || item.kind !== 'file' || !item.available || pending || busy) return;
    menu = null;
    selected = item.entityId;
    selectionEstablished = true;
    previewHeight = 320;
    previewItem = item;
    void tick().then(() => document.querySelector<HTMLElement>('.file-preview-panel button')?.focus());
  }

  function closePreview() {
    previewItem = null;
    void tick().then(() => input?.focus());
  }

  async function openConversion(item: ResultItem) {
    if (!settings?.conversionEnabled || item.kind !== 'file' || !item.available || pending || busy) return;
    menu = null;
    busy = true;
    const epoch = uiEpoch;
    const id = requestId;
    try {
      const sources = await call<ConversionSource[]>('conversion_from_result', { requestId: id, resultId: item.resultId });
      if (epoch !== uiEpoch || id !== requestId) return;
      conversionSources = sources;
      conversionOpen = true;
      void tick().then(() => document.querySelector<HTMLElement>('.conversion-panel button')?.focus());
    } catch (value) { if (epoch === uiEpoch && id === requestId) error = conversionError(value); }
    finally { if (epoch === uiEpoch) busy = false; }
  }

  async function togglePin(item: ResultItem) {
    if (pending || busy) return;
    busy = true;
    menu = null;
    const epoch = uiEpoch;
    const id = requestId;
    try {
      if (item.pinned) await call('remove_pin', { pinId: item.entityId });
      else await call('set_pin', { requestId, resultId: item.resultId, enabled: true });
      if (epoch !== uiEpoch || id !== requestId) return;
      scheduleSearch(true);
      notice = item.pinned ? '已取消固定' : '已固定快捷方式';
    } catch (value) { if (epoch === uiEpoch && id === requestId) error = message(value); }
    finally { if (epoch === uiEpoch) busy = false; }
  }

  async function loadMore() {
    if (busy || pending) return;
    busy = true;
    const epoch = uiEpoch;
    const id = requestId;
    try {
      await call('load_more', { requestId });
    } catch (value) { if (epoch === uiEpoch && id === requestId) error = message(value); }
    finally { if (epoch === uiEpoch) busy = false; }
  }

  async function hide() {
    uiEpoch++;
    busy = false;
    invalidate();
    const epoch = uiEpoch;
    const id = requestId;
    if (preview) { hidden = true; return; }
    try { await call('hide_window'); }
    catch (value) {
      if (epoch !== uiEpoch || id !== requestId) return;
      scheduleSearch(true);
      error = message(value);
    }
  }

  function showSearch() {
    qrPanel?.cancel();
    previewItem = null;
    conversionOpen = false;
    uiEpoch++;
    settingsLoad++;
    busy = false;
    clearTimeout(debounce);
    completionCycle.reset();
    const previous = requestId;
    metrics.cancel(previous);
    const fresh = emptySearchSession(nextRequestId(requestSession, ++requestEpoch));
    requestId = fresh.snapshot.requestId;
    query = fresh.query;
    scope = fresh.scope;
    composing = fresh.composing;
    compositionEndedAt = fresh.compositionEndedAt;
    snapshot = fresh.snapshot;
    selected = fresh.selected;
    menu = fresh.menu;
    notice = fresh.notice;
    error = fresh.error;
    pending = fresh.pending;
    selectionEstablished = false;
    scheduledKey = '';
    dialogElement?.close();
    modal = null;
    hidden = false;
    view = 'search';
    const epoch = uiEpoch;
    // 隐藏时的 DPI 事件可能被忽略；真正呼出后再按当前屏幕强制确认紧凑尺寸。
    void syncLayout({ layout: 'compact', rows: 0 }, true).catch(value => { if (epoch === uiEpoch && !hidden) error = message(value); });
    if (previous) void call('cancel_search', { requestId: previous }).catch(() => {});
    void refreshProviderStatus();
    void tick().then(() => input?.focus());
  }

  async function showQrScan() {
    // 专用事件保留扫码视图；原生临时隐藏不会发送 hide_search。
    if (qrQuery && view === 'search' && !hidden && !modal) { await qrPanel?.scan(); return; }
    qrPanel?.cancel();
    uiEpoch++;
    settingsLoad++;
    busy = false;
    composing = false;
    compositionEndedAt = 0;
    previewItem = null;
    conversionOpen = false;
    dialogElement?.close(); modal = null;
    view = 'search'; hidden = false; query = 'qr';
    scheduleSearch(true);
    const epoch = uiEpoch;
    await tick();
    if (epoch === uiEpoch && qrQuery && view === 'search' && !hidden) { input?.focus(); await qrPanel?.submit(); }
  }

  function handleKey(event: KeyboardEvent) {
    // 自动诊断只接受合成查询，避免前台键盘操作改变测试状态或触发文件动作。
    if (benchmarkRunning) { event.preventDefault(); return; }
    if (blocksIme(event, composing, compositionEndedAt, performance.now())) return;
    if (event.ctrlKey && event.key === ',' && !modal) {
      event.preventDefault();
      hidden = false;
      void openSettings();
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      if (modal) closeModal();
      else if (menu) { menu = null; input?.focus(); }
      else if (previewItem) closePreview();
      else if (view === 'settings') closeSettings();
      else if (qrQuery) { qrPanel?.cancel(); query = ''; scheduleSearch(true); input?.focus(); }
      else if (conversionOpen) { conversionOpen = false; input?.focus(); }
      else if (conversionQuery) { query = ''; scheduleSearch(true); input?.focus(); }
      else void hide();
      return;
    }
    if (modal || view !== 'search') return;
    if (previewItem) return;
    if (qrQuery) {
      if (document.activeElement === input && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
        if (event.key === 'Enter' && !event.repeat) { event.preventDefault(); void qrPanel?.submit(); }
        else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); qrPanel?.move(event.key === 'ArrowDown' ? 1 : -1); }
      }
      return;
    }
    if (conversionQuery) {
      if (event.key === 'Enter' && !event.repeat && document.activeElement === input && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
        event.preventDefault();
        void conversionPanel?.submit();
      }
      return;
    }
    if (reminderQuery) {
      if (event.key === 'Enter' && !event.repeat && document.activeElement === input && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
        event.preventDefault();
        void reminderPanel?.submit();
      }
      return;
    }
    if (translationQuery) {
      if (event.key === 'Enter' && document.activeElement === input && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
        event.preventDefault();
        void translationPanel?.submit();
      }
      return;
    }
    if (!menu && previewShortcut(event, { enabled: settings?.filePreviewEnabled ?? false, established: selectionEstablished, inputFocused: document.activeElement === input, available: actionable, kind: activeItem?.kind ?? '', pending })) {
      event.preventDefault();
      if (activeItem) openPreview(activeItem);
    } else if (event.ctrlKey && event.key === 'Tab') {
      event.preventDefault();
      changeScope(nextScope(scope, event.shiftKey));
    } else if (event.key === 'Tab' && !menu) {
      const value = completionCycle.next({ inputFocused: document.activeElement === input, composing, ctrlKey: event.ctrlKey, shiftKey: event.shiftKey, altKey: event.altKey, metaKey: event.metaKey });
      if (value) { event.preventDefault(); completeFolder(value, true); }
    } else if (event.shiftKey && event.key === 'F10') {
      event.preventDefault();
      if (actionable && activeItem) {
        void openResultMenu(activeItem, results?.querySelector<HTMLElement>('.result-row.selected .result-menu-button') ?? undefined, true);
      }
    } else if (menu) {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        const buttons = Array.from(document.querySelectorAll<HTMLButtonElement>('.action-menu button'));
        const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
        buttons[(current + (event.key === 'ArrowDown' ? 1 : buttons.length - 1)) % buttons.length]?.focus();
      }
    } else if ((event.key === 'ArrowDown' || event.key === 'ArrowUp') && document.activeElement === input) {
      event.preventDefault();
      moveSelection(event.key === 'ArrowDown' ? 1 : -1);
    } else if (event.key === 'Enter' && document.activeElement === input) {
      event.preventDefault();
      const action = enterAction(event, { query, busy, imeBlocked: composing, pending, complete: snapshot.complete, hasMatches: snapshot.items.length > 0, available: !!activeItem?.available });
      if (action === 'web') void searchWeb();
      else if (action) void execute(activeItem, action);
    } else if (event.ctrlKey && event.shiftKey && event.key.toLowerCase() === 'c') {
      event.preventDefault();
      if (actionable) void execute(activeItem, 'copyPath');
    }
  }

  async function openSettings() {
    qrPanel?.cancel();
    previewItem = null;
    const load = ++settingsLoad;
    const epoch = uiEpoch;
    menu = null;
    error = '';
    shortcutError = '';
    qrShortcutError = '';
    view = 'settings';
    try {
      const loaded = await call<Settings>('get_settings');
      if (load !== settingsLoad || epoch !== uiEpoch || view !== 'settings') return;
      settings = loaded;
      draft = structuredClone($state.snapshot(settings));
      includeText = settings.includeDirectories.join('\n');
      excludeText = settings.excludeDirectories.join('\n');
      await refreshProviderStatus();
    } catch (value) { if (load === settingsLoad && epoch === uiEpoch) error = message(value); }
  }

  function openExtensions() {
    settingsTab = 'extensions';
    if (view !== 'settings') void openSettings();
  }

  function closeSettings() {
    settingsLoad++;
    view = 'search';
    error = '';
    scheduleSearch(true);
    void tick().then(() => input?.focus());
  }

  function recordShortcut(event: KeyboardEvent) {
    // 即使是 Tab、Escape 或无效键，也不能触发窗口级搜索与设置操作。
    event.stopPropagation();
    const result = captureShortcut(event);
    if (result.kind === 'navigate') return;
    event.preventDefault();
    if (result.kind === 'cancel') { shortcutError = ''; shortcutInput?.blur(); return; }
    if (result.kind === 'capture' && draft) { draft.shortcut = result.shortcut; shortcutError = ''; }
    else if (result.kind === 'error') shortcutError = result.error === 'modifier' ? '请同时按住 Ctrl、Alt、Shift 或 Win，再按一个键。' : '此键暂不支持，请使用字母、数字、空格、方向键或 F1–F24。';
  }

  async function focusShortcutError() {
    settingsTab = 'general';
    await tick();
    shortcutInput?.focus();
  }

  function recordQrShortcut(event: KeyboardEvent) {
    event.stopPropagation();
    const result = captureShortcut(event);
    if (result.kind === 'navigate') return;
    event.preventDefault();
    if (result.kind === 'cancel') { qrShortcutError = ''; qrShortcutInput?.blur(); return; }
    if (result.kind === 'capture' && draft) { draft.qrScanShortcut = result.shortcut; qrShortcutError = ''; }
    else if (result.kind === 'error') qrShortcutError = result.error === 'modifier' ? '请同时按住 Ctrl、Alt、Shift 或 Win，再按一个键。' : '此键暂不支持，请使用字母、数字、空格、方向键或 F1–F24。';
  }

  async function focusQrShortcutError() {
    settingsTab = 'extensions';
    await tick();
    if (qrDetails) qrDetails.open = true;
    qrShortcutInput?.focus();
  }

  async function saveSettings() {
    if (!draft || !settings || busy) return;
    if (shortcutError) { await focusShortcutError(); return; }
    if (qrShortcutError) { await focusQrShortcutError(); return; }
    busy = true;
    error = '';
    const epoch = uiEpoch;
    // 隐藏的旧引擎配置随草稿原样保留，避免保存其他设置时改写兼容数据。
    const patch = {
      ...$state.snapshot(draft),
      includeDirectories: includeText.split('\n').map(line => line.trim()).filter(Boolean),
      excludeDirectories: excludeText.split('\n').map(line => line.trim()).filter(Boolean),
    };
    const { settingsVersion: _, ...changes } = patch;
    try {
      const saved = await call<Settings>('update_settings', { settingsVersion: settings.settingsVersion, patch: changes });
      if (epoch !== uiEpoch) return;
      settings = saved;
      draft = structuredClone($state.snapshot(settings));
      notice = preview ? '预览设置已更新，仅当前会话有效' : '设置已保存';
      closeSettings();
    } catch (value) {
      if (epoch !== uiEpoch) return;
      const raw = value instanceof Error ? value.message : String(value);
      if (/qr.*shortcut|shortcut.*qr/i.test(raw)) { qrShortcutError = qrScanError(value); await focusQrShortcutError(); }
      else if (/shortcut/i.test(raw)) { shortcutError = message(value); await focusShortcutError(); }
      else error = message(value);
    }
    finally { if (epoch === uiEpoch) busy = false; }
  }

  async function recovery(command: string) {
    if (busy) return;
    busy = true;
    error = '';
    const epoch = uiEpoch;
    try {
      await call(command);
      if (epoch !== uiEpoch) return;
      await refreshProviderStatus();
      if (epoch !== uiEpoch) return;
      if (command === 'refresh_apps') notice = '已请求刷新应用列表';
      if (command === 'start_everything') notice = '已请求重新启动搜索引擎';
      // 设置操作的迟到完成不能重置后来进入的扩展面板。
      if (view === 'search' && !pluginQuery) scheduleSearch(true);
    } catch (value) { if (epoch === uiEpoch) error = message(value); }
    finally { if (epoch === uiEpoch) busy = false; }
  }

  async function openModal(value: typeof modal, item?: ResultItem) {
    const epoch = uiEpoch;
    lastFocused = document.activeElement as HTMLElement;
    modal = value;
    menu = null;
    error = '';
    dialogName = item?.title ?? '';
    originalAliases = value === 'aliases' ? [...(item?.aliases ?? [])] : [];
    dialogAliases = originalAliases.join(', ');
    dialogAppId = item?.entityId ?? '';
    if (value === 'diagnostics') {
      diagnostics = '正在收集诊断信息…';
      try { const report = await call<Record<string, unknown>>('get_diagnostics'); if (epoch === uiEpoch) diagnostics = JSON.stringify({ ...report, searchPerformance: metrics.report() }, null, 2); }
      catch (value) { if (epoch === uiEpoch) diagnostics = message(value); }
    }
    await tick();
    if (epoch !== uiEpoch || modal !== value) return;
    try { await syncLayout({ layout: 'settings', rows: 0 }); }
    catch (value) { if (epoch === uiEpoch) { error = message(value); modal = null; } return; }
    if (epoch !== uiEpoch || modal !== value) return;
    dialogElement?.showModal();
  }

  function closeModal() {
    const epoch = uiEpoch;
    const target = lastFocused;
    dialogElement?.close();
    modal = null;
    error = '';
    void tick().then(() => {
      if (epoch !== uiEpoch || modal) return;
      if (target?.isConnected) target.focus();
      else input?.focus();
    });
  }

  async function submitDialog() {
    if (busy) return;
    if ((modal === 'add' || modal === 'aliases') && aliasError) { aliasesInput?.focus(); return; }
    busy = true;
    const epoch = uiEpoch;
    try {
      const aliases = aliasInput.aliases;
      if (modal === 'add') await call('add_app', { displayName: dialogName.trim(), aliases });
      else if (modal === 'aliases') await call('set_app_aliases', { appId: dialogAppId, aliases });
      else if (modal === 'clear') await call('clear_usage_history');
      if (epoch !== uiEpoch) return;
      notice = modal === 'clear' ? '使用记录已清除' : modal === 'aliases' ? '别名已保存' : '';
      closeModal();
      if (view === 'search') scheduleSearch(true);
    } catch (value) { if (epoch === uiEpoch) error = message(value); }
    finally { if (epoch === uiEpoch) busy = false; }
  }

  function downloadDiagnostics() {
    const url = URL.createObjectURL(new Blob([diagnostics], { type: 'application/json' }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = 'supersearch-diagnostics.json';
    anchor.click();
    URL.revokeObjectURL(url);
  }

  onMount(() => {
    documentVisible = !document.hidden;
    let disposed = false;
    const cleanup: (() => void)[] = [];
    let soakCycle: { hiddenSeen: boolean; resolve: () => void; reject: (error: Error) => void } | null = null;
    async function initialize() {
      const epoch = uiEpoch;
      try {
        for (const [event, handler] of [
          ['search_snapshot', (value: SearchSnapshot) => applySnapshot(value)],
          ['window_scale_changed', () => {
            const epoch = uiEpoch;
            if (!hidden) void syncLayout({ ...layout }, true).catch(value => { if (epoch === uiEpoch && !hidden) error = message(value); });
          }],
          ['show_search', () => { showSearch(); if (soakCycle?.hiddenSeen) soakCycle.resolve(); }],
          ['qr_scan_requested', () => { void showQrScan(); }],
          ['settings_requested', () => { showSearch(); void openSettings(); }],
          ['hide_search', () => { uiEpoch++; busy = false; hidden = true; scheduledKey = ''; invalidate(); if (soakCycle) soakCycle.hiddenSeen = true; }],
          ['apps_status', (value: Provider) => {
            providers = mergeProviders(providers, [value]);
            // 后台应用刷新只重查普通搜索，不能清理扩展会话或取消临时隐藏中的扫码。
            if (!benchmarkRunning && !hidden && view === 'search' && !pluginQuery) scheduleSearch(true);
            void refreshProviderStatus();
          }],
        ] as const) {
          const stop = await subscribe(event, handler as (value: unknown) => void);
          if (disposed) { stop(); return; }
          cleanup.push(stop);
        }
        settings = await call<Settings>('get_settings');
        if (disposed) return;
        if (epoch === uiEpoch && requestEpoch === 0 && view === 'search') showSearch();
        void refreshProviderStatus();
        input?.focus();
        if (!preview) {
          const config = await call<BenchmarkConfig | null>('get_ui_benchmark');
          if (config && !disposed) {
            benchmarkRunning = true;
            const port: Parameters<typeof runUiBenchmark>[1] = {
              search: raw => { query = raw; scope = 'files'; scheduleSearch(true); return requestId; },
              current: id => snapshot.requestId === id && requestId === id ? { sample: metrics.settled(id), complete: snapshot.complete, count: snapshot.items.length, firstTitle: snapshot.items[0]?.title ?? '', selectedTitle: snapshot.items.find(item => item.entityId === selected)?.title ?? '', candidates: snapshot.folderCompletions?.length ?? 0, ready: snapshot.providers.find(provider => provider.id === 'everything')?.status === 'ready' } : null,
              finish: report => call('save_ui_benchmark', { report }),
              diagnostics: () => ({ hidden, view, requestId, snapshotRequestId: snapshot.requestId, revision: snapshot.snapshotRevision, pending, error }),
            };
            if (config.soakSeconds) {
              void runUiSoak(config, {
                ...port,
                compact: async () => { query = ''; scheduleSearch(true); await tick(); await syncLayout({ layout: 'compact', rows: 0 }); },
                window: async hiddenMs => {
                  if (hiddenMs === undefined) return call<SoakWindow>('ui_soak_window', { hiddenMs: null });
                  let timeout: ReturnType<typeof setTimeout>;
                  // 先登记事件再发原生命令；隐藏期间不轮询 DOM，也不等待动画帧。
                  const events = new Promise<void>((resolve, reject) => {
                    soakCycle = { hiddenSeen: false, resolve, reject };
                    timeout = setTimeout(() => reject(new Error('Native hide/show events were not acknowledged')), hiddenMs + 5000);
                  });
                  try {
                    const [window] = await Promise.all([call<SoakWindow>('ui_soak_window', { hiddenMs }), events]);
                    await tick();
                    return window;
                  } finally { clearTimeout(timeout!); soakCycle = null; }
                },
              });
            } else void runUiBenchmark(config, port);
          }
        }
      } catch (value) { if (!disposed && epoch === uiEpoch) { error = message(value); pending = false; } }
    }
    void initialize();
    return () => { disposed = true; qrPanel?.cancel(); clearTimeout(debounce); soakCycle?.reject(new Error('UI soak was disposed')); cleanup.forEach(stop => stop()); };
  });
</script>

<svelte:window onkeydown={handleKey} onclick={(event) => { if (menu && !(event.target as Element)?.closest('.action-menu, .result-menu-button, .footer-actions')) menu = null; }} />
<svelte:document onvisibilitychange={() => { documentVisible = !document.hidden; }} />

<main class="app-shell" class:preview-mode={preview} class:expanded class:settings-view={view === 'settings' || !!modal || pluginQuery || !!previewItem} style:height={previewItem && view === 'search' ? `${previewHeight}px` : undefined} data-theme={theme} data-tauri-drag-region inert={benchmarkRunning}>
  {#if hidden}
    <section class="hidden-state"><button class="primary" onclick={showSearch}>显示搜索 <ArrowUpRight size={16} /></button></section>
  {:else if view === 'search'}
    <section class="search-area" data-tauri-drag-region>
      <div class="search-box" data-tauri-drag-region>
        <button class="search-symbol" title={`设置 · Ctrl+,；呼出搜索 ${settings?.shortcut ?? 'Alt+Space'}`} aria-label="打开设置（Ctrl+,）" onclick={openSettings}>
          <span class="search-glyph" aria-hidden="true"><Search size={25} strokeWidth={1.8} /></span>
          <span class="settings-glyph" aria-hidden="true"><SettingsIcon size={22} strokeWidth={1.6} /></span>
        </button>
        <input bind:this={input} bind:value={query} oninput={() => { notice = ''; conversionOpen = false; scheduleSearch(); }}
          oncompositionstart={() => { composing = true; scheduledKey = ''; invalidate(); }}
          oncompositionend={() => { composing = false; compositionEndedAt = performance.now(); scheduledKey = ''; scheduleSearch(); }}
          placeholder="搜索应用、文件或文件夹" aria-label="搜索应用、文件和文件夹" aria-describedby="search-input-help"
          aria-controls={expanded ? 'search-results' : undefined} aria-activedescendant={expanded && activeItem ? `result-${activeItem.resultId}` : undefined}
          role="combobox" aria-expanded={expanded} aria-autocomplete="list" autocomplete="off" spellcheck="false" />
        {#if error || busy || (query.trim() && pending) || notice}<span class="inline-status" class:error-status={!!error} title={error || (busy ? '正在打开…' : inlineStatus)} aria-hidden="true">{#if error}<AlertCircle size={16} />{:else if busy || pending}<RefreshCw size={16} class="loading-icon" />{:else}<Check size={16} />{/if}</span>{/if}
        {#if directoryScope && !expanded}<span class="compact-directory" title={`搜索范围：${directoryScope.path} · ${directoryScope.detail}`} aria-label={`搜索范围：${directoryScope.path}，${directoryScope.detail}`}><Folder size={13} /><span>{directoryScope.path}<small>{directoryScope.detail}</small></span></span>{/if}
        {#if webFallback}<button class="web-search-button" title="Enter 使用必应搜索；Shift+Enter 始终搜索网页" aria-label="使用默认浏览器打开必应搜索" onclick={searchWeb}>↵ 必应搜索</button>{/if}
        {#if query}<button class="icon-button clear-search" aria-label="清空搜索" onclick={() => { query = ''; notice = ''; scheduleSearch(); input?.focus(); }}><X size={18} /></button>{/if}
      </div>
      <span id="search-input-help" class="sr-only">输入名称搜索；也可输入 项目/报告 或 D:\项目\报告。目录范围包含子文件夹。同名文件夹会一起搜索，完整路径可指定唯一目录。也支持文件夹路径，空格后加文件名。</span>
      <span class="sr-only" role={error ? 'alert' : 'status'} aria-live={error ? 'assertive' : 'polite'}>{inlineStatus || (expanded ? `找到 ${snapshot.items.length} 个结果` : '')}</span>
    </section>
    {#if previewItem}
      {#key `${requestId}:${previewItem.resultId}`}<FilePreviewPanel item={previewItem} {requestId} onHeight={(height) => { previewHeight = height; }} close={closePreview} open={() => { const item = previewItem; closePreview(); if (item) void execute(item); }} />{/key}
    {:else if qrQuery}
      <QrScanPanel bind:this={qrPanel} enabled={settings?.qrScanEnabled ?? false} {openExtensions} />
    {:else if conversionQuery}
      <ConversionPanel bind:this={conversionPanel} enabled={settings?.conversionEnabled ?? false} initialSources={conversionSources} {openExtensions} />
    {:else if reminderQuery}
      <ReminderPanel bind:this={reminderPanel} query={reminderQuery} enabled={settings?.remindersEnabled ?? false} active={!hidden && documentVisible} {openExtensions} />
    {:else if translationQuery}
      {#key translationQuery.mode}<TranslationPanel bind:this={translationPanel} query={translationQuery} enabled={settings?.translationEnabled ?? false} active={!hidden && documentVisible} {openExtensions} />{/key}
    {:else if expanded}
      <section class="result-panel" aria-label="搜索结果">
        <div class="scope-bar">
          {#if directoryScope}<div class="directory-scope" title={`搜索范围：${directoryScope.path} · ${directoryScope.detail}`}><strong>{directoryScope.path}</strong><span class="scope-separator">/</span><span class="directory-detail">{directoryScope.detail}</span></div>{:else}<span class="result-summary">找到 {snapshot.items.length} 个结果</span>{/if}
          <div class="scope-tabs" role="tablist" aria-label="搜索范围">
            {#each scopes as value}<button role="tab" aria-selected={scope === value} class:active={scope === value} onclick={() => changeScope(value)}>{scopeLabels[value]}</button>{/each}
          </div>
        </div>
        <div bind:this={results} class="results-list" style:height={`${layout.rows * 76}px`} id="search-results" role="listbox" aria-label="搜索结果列表" aria-busy={pending} onscroll={() => { menu = null; }}>
          {#each folderCompletions as candidate (candidate.path)}
            <div class="result-row completion-row"><button class="result-main" role="option" aria-selected="false" title={`${candidate.path} · 补全文件夹路径`} onclick={() => completeFolder(candidate.query)}>
              <ResultIcon item={{ kind: 'folder', title: candidate.title }} /><span class="result-text"><span class="result-title">{candidate.title}<span class="completion-label">补全文件夹</span></span><span class="result-subtitle completion-path">{candidate.path}</span></span><span class="completion-hint"><kbd>Tab</kbd><span>补全</span></span>
            </button></div>
          {/each}
          {#each snapshot.items as item (item.resultId)}
            {@const path = item.path ? { parent: item.subtitle, tail: '' } : subtitleParts(item.subtitle, item.title)}
            <div class="result-wrap">
              <div class="result-row" class:selected={selected === item.entityId} class:unavailable={!item.available}>
                <button id={`result-${item.resultId}`} class="result-main" role="option" aria-selected={selected === item.entityId} aria-disabled={!item.available} tabindex="-1"
                  onmousemove={(event) => { if (!benchmarkRunning && (event.movementX || event.movementY) && !pending && !menu) { selected = item.entityId; selectionEstablished = true; } }}
                  onclick={() => { selected = item.entityId; void execute(item); }} title={item.path || item.subtitle}>
                  <ResultIcon {item} />
                  <span class="result-text"><span class="result-title">{#each highlightSegments(item.title, query, snapshot.directory, snapshot.terms) as segment}<span class:keyword-match={segment.match}>{segment.text}</span>{/each}{#if !item.available}<span class="unavailable-label">不可用</span>{/if}</span><span class="result-subtitle">{#if path.parent}<strong class="parent-path">{path.parent}</strong>{/if}{path.tail}</span></span>
                  {#if item.pinned}<Pin size={13} class="pinned-icon" />{/if}
                  <span class="enter-indicator"><kbd>Enter</kbd><span>打开</span></span>
                </button>
                <button class="result-menu-button icon-button" aria-label={`${item.title} 的操作`} aria-expanded={menu === item.resultId} disabled={pending || busy} onclick={(event) => openResultMenu(item, event.currentTarget)}><MoreHorizontal size={18} /></button>
              </div>
              {#if menu === item.resultId}<div class="action-menu" style:left={`${menuLocation.left}px`} style:top={`${menuLocation.top}px`} style:visibility={menuPlaced ? 'visible' : 'hidden'} role="menu" aria-label={`${item.title} 的操作`}>
                {#if item.actions.includes('open')}<button role="menuitem" disabled={!item.available} onclick={() => execute(item)}><ArrowUpRight size={15} /><span>打开</span><kbd>↵</kbd></button>{/if}
                {#if item.actions.includes('reveal')}<button role="menuitem" disabled={!item.available} onclick={() => execute(item, 'reveal')}><Folder size={15} /><span>在资源管理器中显示</span><kbd>Ctrl ↵</kbd></button>{/if}
                {#if item.actions.includes('copyPath')}<button role="menuitem" disabled={!item.available} onclick={() => execute(item, 'copyPath')}><Copy size={15} /><span>复制路径</span><kbd>Ctrl ⇧ C</kbd></button>{/if}
                {#if settings?.filePreviewEnabled && item.kind === 'file'}<button role="menuitem" disabled={!item.available} onclick={() => openPreview(item)}><FileSearch size={15} /><span>快速预览</span><kbd>空格</kbd></button>{/if}
                {#if settings?.conversionEnabled && item.kind === 'file'}<button role="menuitem" disabled={!item.available} onclick={() => openConversion(item)}><ArrowRightLeft size={15} /><span>格式转换</span></button>{/if}
                <div class="menu-divider"></div><button role="menuitem" onclick={() => togglePin(item)}><Pin size={15} /><span>{item.pinned ? '取消固定' : '固定快捷方式'}</span></button>
                {#if item.kind === 'application'}<button role="menuitem" onclick={() => openModal('aliases', item)}><Keyboard size={15} /><span>设置别名</span></button>{/if}
              </div>{/if}
            </div>
          {/each}
        </div>
        <footer class="search-footer">
          {#if footerStatus}<span class="footer-status" class:error-status={!!error} role={error ? 'alert' : 'status'} title={footerStatus}>{footerStatus}</span>{:else}<div class="keyboard-hints"><span><kbd>↑ ↓</kbd>选择</span><span><kbd>Enter</kbd>打开</span><span title="Tab 补全文件夹；Shift+Tab 返回"><kbd>Tab</kbd>补全</span></div>{/if}
          {#if snapshot.hasMore}<button class="text-button load-more" disabled={busy || pending} onclick={loadMore}>{busy ? '加载中…' : '加载更多'}</button>{/if}
          <div class="footer-tools"><button class="provider-indicator" title={preview ? '浏览器预览仅展示样例，未连接本地搜索。' : `找到 ${snapshot.items.length} 个结果；打开索引与数据设置`} onclick={() => { settingsTab = 'advanced'; void openSettings(); }}><span class="status-dot" class:warning={preview || !!unavailableProvider || !!error}></span>{preview ? '预览样例' : unavailableProvider ? '检查服务' : '本地搜索'}</button><button class="footer-actions" title="结果操作 · Shift+F10" aria-label="当前结果的更多操作" disabled={!activeItem || pending || busy} onclick={(event) => { if (activeItem) void openResultMenu(activeItem, event.currentTarget); }}><span>更多操作</span><MoreHorizontal size={18} /></button></div>
        </footer>
      </section>
    {/if}
  {:else}
    <header class="settings-header" data-tauri-drag-region><button class="icon-button" aria-label="返回搜索" onclick={closeSettings}><ArrowLeft size={19} /></button><h1 data-tauri-drag-region>设置</h1><span class="settings-brand" data-tauri-drag-region><BrandMark size={25} />SuperSearch</span></header>
    <div class="settings-layout"><nav class="settings-nav" aria-label="设置分类"><button class:active={settingsTab === 'general'} aria-current={settingsTab === 'general' ? 'page' : undefined} onclick={() => settingsTab = 'general'}><SettingsIcon size={19} />通用</button><button class:active={settingsTab === 'search'} aria-current={settingsTab === 'search' ? 'page' : undefined} onclick={() => settingsTab = 'search'}><Search size={19} />搜索</button><button class:active={settingsTab === 'extensions'} aria-current={settingsTab === 'extensions' ? 'page' : undefined} onclick={openExtensions}><Puzzle size={19} />扩展</button><button class:active={settingsTab === 'advanced'} aria-current={settingsTab === 'advanced' ? 'page' : undefined} onclick={() => settingsTab = 'advanced'}><Database size={19} />索引与数据</button></nav>
      <section class="settings-content">
        {#if !draft}<div class="empty-state"><RefreshCw size={24} /><p>{error ? '无法加载设置。' : '正在加载设置…'}</p></div>
        {:else if settingsTab === 'general'}
          <h2>通用</h2>
          <div class="shortcut-setting">
            <div class="setting-row"><div><label for="shortcut">启动快捷键</label><p>聚焦右侧输入框后，直接按组合键。</p></div><div class="shortcut-field" class:invalid-field={!!shortcutError}>
              <input id="shortcut" bind:this={shortcutInput} value={draft.shortcut} readonly autocomplete="off" spellcheck="false" aria-describedby={`shortcut-help shortcut-status${shortcutError ? ' shortcut-error' : ''}`} aria-invalid={!!shortcutError} onkeydown={recordShortcut} onkeyup={(event) => event.stopPropagation()} onfocus={() => shortcutRecording = true} onblur={() => shortcutRecording = false} />
            </div></div>
            <p id="shortcut-help" class="field-help">Ctrl / Alt / Shift / Win + 按键；Tab 切换焦点，Esc 结束录入。</p>
            <p id="shortcut-status" class="field-help" role="status">{shortcutRecording ? '正在录入；' : ''}{preview ? '预览不检测系统占用；桌面版保存时验证。' : providers.some(provider => provider.id === 'shortcut' && provider.status === 'ready') ? `当前 ${settings?.shortcut ?? 'Alt+Space'} 仍会呼出或隐藏窗口；保存时验证新快捷键。` : '新快捷键保存时由系统验证；如已占用，请更换组合键。'}</p>
            {#if shortcutError}<p id="shortcut-error" class="field-error" role="alert">{shortcutError}</p>{/if}
          </div>
          <div class="setting-row"><div><label for="autostart">登录时启动</label><p>开机后自动运行 SuperSearch。</p></div><input id="autostart" type="checkbox" role="switch" class="switch" bind:checked={draft.autostart} /></div>
          <div class="setting-row"><div><label for="history">记录使用历史</label><p>根据本地记录，让常用项目更靠前。</p></div><input id="history" type="checkbox" role="switch" class="switch" bind:checked={draft.recordHistory} /></div>
          <div class="setting-row placement-setting"><div><label for="window-placement">显示位置</label><p id="window-placement-help">选择再次呼出时使用的位置；展开结果时保持当前位置。</p></div><select id="window-placement" aria-describedby="window-placement-help" bind:value={draft.windowPlacement}><option value="remember">记住上次位置</option><option value="cursor">跟随鼠标所在显示器</option></select></div>
          <fieldset class="appearance-field"><legend>外观</legend><p>选择搜索窗口的主题。</p><div class="theme-options">
            <label class="theme-choice theme-light"><input type="radio" name="theme" value="light" bind:group={draft.theme} /><Sun size={24} strokeWidth={1.5} /><span>浅色</span></label>
            <label class="theme-choice theme-dark"><input type="radio" name="theme" value="dark" bind:group={draft.theme} /><Moon size={24} strokeWidth={1.5} /><span>深色</span></label>
            <label class="theme-choice theme-system"><input type="radio" name="theme" value="system" bind:group={draft.theme} /><Monitor size={24} strokeWidth={1.5} /><span>跟随系统</span></label>
          </div></fieldset>
          <details class="settings-tip"><summary><Keyboard size={16} />键盘快捷操作</summary><p>↑ ↓ 选择结果；Enter 打开；Ctrl + Tab 切换范围；Tab 补全文件夹；Shift + F10 打开结果操作；Ctrl + Shift + C 复制路径；Shift + Enter 搜索网页。</p></details>
        {:else if settingsTab === 'search'}
          <h2>搜索范围</h2><p class="settings-intro">选择参与搜索的应用和目录。</p>
          <div class="setting-row"><div><label for="desktop">包含桌面应用</label><p>搜索桌面上的应用快捷方式。</p></div><input id="desktop" type="checkbox" role="switch" class="switch" bind:checked={draft.includeDesktop} /></div>
          <div class="setting-row"><div><label for="hidden">排除隐藏文件</label><p>不显示隐藏文件和文件夹。</p></div><input id="hidden" type="checkbox" role="switch" class="switch" bind:checked={draft.hideHidden} /></div>
          <div class="setting-row"><div><label for="system">排除系统文件</label><p>不显示操作系统文件。</p></div><input id="system" type="checkbox" role="switch" class="switch" bind:checked={draft.hideSystem} /></div>
          <label class="field-label spaced" for="include-dirs">仅搜索以下目录</label><textarea id="include-dirs" bind:value={includeText} placeholder="C:\Users\you\Documents" rows="2"></textarea><p class="field-help">每行一个绝对路径；留空时使用 Everything 的完整索引。</p>
          <label class="field-label spaced" for="exclude-dirs">排除以下目录</label><textarea id="exclude-dirs" bind:value={excludeText} placeholder="C:\Users\you\Downloads\Archive" rows="2"></textarea><p class="field-help">每行一个绝对路径；排除规则也适用于已固定文件。</p>
          <div class="button-row"><button class="secondary" disabled={busy} onclick={() => recovery('refresh_apps')}><RefreshCw size={14} />刷新应用</button><button class="secondary" disabled={busy} onclick={() => openModal('add')}><Plus size={14} />添加应用</button></div>
          <p class="field-help">直接输入 <code>项目/报告</code> 搜索文件夹内的文件；同名文件夹会一起搜索。输入 <code>D:\项目\报告</code> 指定完整路径，输入 <code>D:\项目\</code> 列出文件。包含子文件夹，也支持 <code>D:\项目 报告</code> 和 <code>ext:pdf</code>。</p>
        {:else if settingsTab === 'extensions'}
          <h2>扩展</h2><p class="settings-intro">按需开启，让搜索框多一些实用能力。</p>
          <div class="provider-card extension-card">
            <details class="extension-details" bind:this={qrDetails}>
              <summary class="extension-card-title" aria-label="屏幕扫码说明"><ScanLine size={23} strokeWidth={1.6} /><span><strong>屏幕扫码</strong><span id="qr-description" class="extension-description">识别屏幕网址与二维码，复制或打开链接</span></span></summary>
              <p>输入 <code>扫码</code> 或 <code>scan</code>，按回车识别鼠标所在显示器上显示的 HTTP/HTTPS 网址文字与二维码；也支持 <code>qr</code>、<code>扫一扫</code>。网页链接显示在二维码上方，分别复制或打开；不会自动打开。首次识别网址会准备本地文字识别组件，请保留完整包中的 <code>components</code> 文件夹。</p>
              <p>在本机识别，不联网、不保存截图或扫码历史。每次最多显示 32 个二维码，单条内容最多 16 KiB；过大的屏幕或受保护内容可能无法扫描。</p>
              <label class="field-label" for="qr-shortcut">扫码快捷键（可选）</label>
              <div class="qr-shortcut-row"><input id="qr-shortcut" class="shortcut-field" bind:this={qrShortcutInput} value={draft.qrScanShortcut} onkeydown={recordQrShortcut} readonly placeholder="点击后按下组合键" aria-describedby={`qr-shortcut-help${qrShortcutError ? ' qr-shortcut-error' : ''}`} aria-invalid={!!qrShortcutError} /><button class="secondary" disabled={!draft.qrScanShortcut} onclick={() => { if (draft) draft.qrScanShortcut = ''; qrShortcutError = ''; }}>清除</button></div>
              <p id="qr-shortcut-help" class="field-help">留空时仅通过搜索命令扫描；保存后生效，需与搜索快捷键不同。</p>
              {#if qrShortcutError}<p id="qr-shortcut-error" class="field-error" role="alert">{qrShortcutError}</p>{/if}
            </details>
            <input id="qr-scan-enabled" type="checkbox" role="switch" class="switch" bind:checked={draft.qrScanEnabled} aria-label="屏幕扫码" aria-describedby="qr-description" />
          </div>
          <div class="provider-card extension-card">
            <details class="extension-details">
              <summary class="extension-card-title" aria-label="翻译与词典说明"><Languages size={23} strokeWidth={1.6} /><span><strong>翻译与词典</strong><span id="translation-description" class="extension-description">中英短文本翻译与英文单词查询</span></span></summary>
              <div class="extension-commands"><p><code>翻译 hello / translate hello</code><span>英语译为中文</span></p><p><code>翻译 开发计划 / translate 开发计划</code><span>中文译为英语</span></p><p><code>查词 hello / dictionary hello</code><span>音标、英文释义与例句</span></p></div>
              <p>也支持简写 <code>fy</code>、<code>dict</code>。命令后用空格分隔内容；英文命令不区分大小写。</p>
              <p>使用 MyMemory 与 Free Dictionary API，无需密钥。按回车或点击提交后才发送文本；不会自动上传普通搜索词，也不保存翻译历史。</p>
              <p>翻译每次最多 500 个 UTF-8 字节（常见汉字约 166 个）；公共服务有额度与可用性限制。词典提供英文释义，中文含义请使用翻译。</p>
            </details>
            <input id="translation-enabled" type="checkbox" role="switch" class="switch" bind:checked={draft.translationEnabled} aria-label="翻译与词典" aria-describedby="translation-description" />
          </div>
          <div class="provider-card extension-card">
            <details class="extension-details"><summary class="extension-card-title" aria-label="文件快速预览说明"><FileSearch size={23} strokeWidth={1.6} /><span><strong>文件快速预览</strong><span id="preview-description" class="extension-description">空格查看图片、文档与代码</span></span></summary><p>用方向键选中文件后按空格，或从文件操作菜单选择“快速预览”。Esc 返回搜索结果。</p><p>仅在本机读取常规文件：PDF 上限 100 MiB，图片与 DOCX / XLSX / PPTX 上限 50 MiB，文本上限 1 GiB（分段读取）。支持 TIFF、ICO 和常见代码格式，HTML / SVG 仅展示源码。Office 仅预览文字与表格，不完整保留原排版；不支持旧版 Office 和网络文件。</p></details>
            <input id="file-preview-enabled" type="checkbox" role="switch" class="switch" bind:checked={draft.filePreviewEnabled} aria-label="文件快速预览" aria-describedby="preview-description" />
          </div>
          <div class="provider-card extension-card">
            <details class="extension-details"><summary class="extension-card-title" aria-label="倒计时提醒说明"><Bell size={23} strokeWidth={1.6} /><span><strong>倒计时提醒</strong><span id="reminder-description" class="extension-description">后台计时，到时发送桌面通知</span></span></summary><p>输入 <code>提醒 20m 休息</code> / <code>remind 20m 休息</code>，或 <code>倒计时 30s</code> / <code>timer 30s</code>，回车创建。也支持 <code>reminder</code>、<code>countdown</code>，英文不区分大小写。支持 s 秒、m 分钟、h 小时、d 天，可组合 <code>1h30m</code>；也支持中文单位。输入 <code>提醒</code> / <code>remind</code> 查看和取消。</p><p>支持 1 秒到 7 天，最多 20 个待执行提醒。隐藏窗口继续计时；退出后重新打开可恢复。请允许 SuperSearch 的 Windows 通知。</p></details>
            <input id="reminders-enabled" type="checkbox" role="switch" class="switch" bind:checked={draft.remindersEnabled} aria-label="倒计时提醒" aria-describedby="reminder-description" />
          </div>
          {#if settings?.remindersEnabled && !draft.remindersEnabled}<p class="field-help" role="status">保存后将取消所有待执行提醒；点击“取消”则保留。</p>{/if}
          <div class="provider-card extension-card">
            <details class="extension-details"><summary class="extension-card-title" aria-label="格式转换说明"><ArrowRightLeft size={23} strokeWidth={1.6} /><span><strong>格式转换</strong><span id="conversion-description" class="extension-description">本地转换图片与文档，组件按需安装</span></span></summary><p>输入 <code>转换</code> 或 <code>convert</code> 选择文件，也可从文件操作菜单进入。支持批量处理，生成新文件并保留原文件。</p><p>常见图片格式直接转换；Office、Markdown、PDF 与文字识别按用途安装本地组件，安装后离线使用。PDF 转 Word 为文字提取，转 PPT 为图片式幻灯片。</p><p>隐藏窗口后继续转换；关闭插件只阻止新任务，已开始的任务可在转换面板取消。</p></details>
            <input id="conversion-enabled" type="checkbox" role="switch" class="switch" bind:checked={draft.conversionEnabled} aria-label="格式转换" aria-describedby="conversion-description" />
          </div>
        {:else}
          <h2>文件搜索引擎</h2><p class="settings-intro">内置引擎由 SuperSearch 自动在后台管理；安装包包含专属高速索引服务。</p>
          <div class="provider-card"><div class="provider-card-heading"><span class="provider-logo"><Search size={19} /></span><div><strong>内置 Everything</strong><span>{preview ? '预览模式未连接本地引擎' : fileProvider?.status === 'ready' ? '搜索引擎已就绪' : fileProvider?.status === 'indexLoading' || fileProvider?.status === 'loading' ? '索引正在加载' : fileProvider?.status === 'notRunning' ? '搜索引擎未运行' : '搜索引擎暂不可用'}</span></div><span class="status-dot" class:warning={preview || fileProvider?.status !== 'ready'}></span></div><p>{fileProvider ? providerMessage(fileProvider) : '正在检查内置文件搜索引擎…'}</p><p>完成一次管理员安装后，NTFS 固定磁盘使用高速卷索引；服务不可用或其它文件系统使用目录扫描。在「搜索」中设置目录，可缩小返回结果范围。</p><div class="button-row"><button class="secondary" disabled={busy} onclick={() => recovery('start_everything')}><RefreshCw size={14} />重新启动搜索引擎</button></div></div>
          <div class="setting-row"><div><strong>使用记录</strong><p>清除此设备上的最近打开记录与使用次数。</p></div><button class="secondary" onclick={() => openModal('clear')}>清除记录</button></div>
          <div class="setting-row"><div><strong>诊断信息</strong><p>导出前先查看本地诊断报告。</p></div><button class="secondary" onclick={() => openModal('diagnostics')}>查看报告</button></div>
          {#each providers.filter(provider => provider.id !== 'everything') as provider}<div class="provider-note"><span class="status-dot" class:warning={provider.status !== 'ready'}></span><span>{providerMessage(provider)}</span></div>{/each}
        {/if}
      </section>
    </div>
    {#if error && !modal}<div class="message error-message" role="alert"><AlertCircle size={16} /><span>{error}</span></div>{/if}
    <footer class="settings-footer"><span>{preview ? '预览设置仅当前会话有效。' : '保存后应用更改。'}</span><div class="button-row"><button class="secondary" onclick={closeSettings}>取消</button><button class="primary" disabled={busy || !draft} onclick={saveSettings}>{busy ? '保存中…' : '保存更改'}</button></div></footer>
  {/if}

  <dialog bind:this={dialogElement} oncancel={(event) => { event.preventDefault(); closeModal(); }} class="dialog">
    {#if modal}<div class="dialog-header"><h2>{modal === 'add' ? '添加应用' : modal === 'aliases' ? '应用别名' : modal === 'clear' ? '清除使用记录？' : '诊断报告'}</h2><button class="icon-button" aria-label="关闭对话框" onclick={closeModal}><X size={18} /></button></div>
      {#if modal === 'add' || modal === 'aliases'}
        <p class="dialog-description">{modal === 'add' ? '填写信息后，选择本地应用程序或快捷方式。' : `为 ${dialogName} 设置完整的别名列表，保存后替换现有别名。`}</p>
        {#if modal === 'add'}<label class="field-label" for="app-name">显示名称</label><input id="app-name" bind:value={dialogName} placeholder="应用名称" />{/if}
        <label class="field-label spaced" for="aliases">别名</label><input id="aliases" bind:this={aliasesInput} bind:value={dialogAliases} placeholder="code, editor, vscode" aria-describedby={`aliases-help${aliasError ? ' aliases-error' : ''}`} aria-invalid={!!aliasError} /><p id="aliases-help" class="field-help">用中文或英文逗号分隔；最多 16 个，每个 128 字符，重复项自动合并。</p>
        {#if aliasError}<p id="aliases-error" class="field-error" role="alert">{aliasError}</p>{/if}
      {:else if modal === 'clear'}<p class="dialog-description">将清除最近打开记录和使用次数，保留已固定快捷方式与别名。</p>
      {:else}<p class="dialog-description">分享前请检查报告，其中可能包含本地路径；程序不会主动记录搜索词。</p><pre class="diagnostic-preview">{diagnostics}</pre>{/if}
      {#if error}<div class="message error-message" role="alert"><AlertCircle size={15} /><span>{error}</span></div>{/if}
      <div class="dialog-footer"><button class="secondary" onclick={closeModal}>{modal === 'diagnostics' ? '关闭' : '取消'}</button>{#if modal === 'diagnostics'}<button class="primary" onclick={downloadDiagnostics}><Download size={15} />导出报告</button>{:else}<button class="primary" disabled={busy || (modal === 'add' && !dialogName.trim())} onclick={submitDialog}>{busy ? '处理中…' : modal === 'clear' ? '清除记录' : modal === 'add' ? '选择应用' : '保存别名'}</button>{/if}</div>
    {/if}
  </dialog>
</main>
