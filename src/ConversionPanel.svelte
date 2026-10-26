<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { ArrowDown, ArrowUp, ArrowRightLeft, Check, Download, FilePlus2, FolderOpen, RefreshCw, X } from 'lucide-svelte';
  import { call, preview, subscribe } from './bridge';
  import { clearConversionDraftSources, pendingConversionDraftClear, commonTargets, conversionError, conversionRetryRequest, createConversionRefresh, createCsvPreviewSession, csvPreviewKey, csvPreviewReady, formatBytes, loadConversionDraft, mergeConversionSources, optionsForConversionSources, refreshConversionEvent, requiredComponents, saveConversionDraft, targetLabel, validateConversionOptions } from './conversion';
  import type { ConversionComponent, ConversionComponentId, ConversionDirectory, ConversionJob, ConversionOptions, ConversionSource, CsvPreviewResult, CsvPreviewState } from './conversion';

  let { enabled, openExtensions, initialSources = [] }: { enabled: boolean; openExtensions: () => void; initialSources?: ConversionSource[] } = $props();
  const restored = untrack(() => loadConversionDraft(initialSources));
  const pendingClear = untrack(pendingConversionDraftClear);
  let sources = $state<ConversionSource[]>(restored.sources);
  let directory = $state<ConversionDirectory | null>(restored.directory);
  let options = $state<ConversionOptions>(restored.options);
  let components = $state<ConversionComponent[]>([]);
  let jobs = $state<ConversionJob[]>([]);
  let pending = $state(pendingClear !== null);
  let loading = $state(false);
  let componentActions = $state<string[]>([]);
  let error = $state('');
  let notice = $state('');
  let csvState = $state<CsvPreviewState>({ key: '', pending: false, completed: 0, total: 0, results: {}, error: '' });
  let csvSelectedId = $state('');
  let lastCsvKey = '';
  let current = true;
  const extensions = $derived(sources.map(source => source.extension));
  const targets = $derived(commonTargets(extensions));
  const required = $derived(requiredComponents(extensions, options));
  const missing = $derived(required.filter(id => !components.some(component => component.id === id && component.status === 'ready')));
  const csvSources = $derived(sources.filter(source => source.extension.toLowerCase() === 'csv'));
  const csvKey = $derived(csvPreviewKey(csvSources, options.csvDelimiter, options.csvEncoding));
  const csvReady = $derived(csvPreviewReady(csvState, csvSources, options.csvDelimiter, options.csvEncoding));
  const csvDisplayId = $derived(csvSources.some(source => source.id === csvSelectedId) ? csvSelectedId : csvSources[0]?.id ?? '');
  const csvPreviewResult = $derived(csvState.key === csvKey ? csvState.results[csvDisplayId] : undefined);
  const csvIssue = $derived(csvSources.some(source => source.bytes > 32 * 1024 * 1024) ? 'CSV 导入和预览仅支持不超过 32 MiB 的文件，请分割文件后重试。' : csvSources.length && !csvReady ? '请先预览所有 CSV 文件，确认分隔符和编码正确后再转换。' : '');
  const invalid = $derived(sources.length ? validateConversionOptions(extensions, options) || csvIssue : '请选择要转换的文件。');
  const pdfSource = $derived(extensions.some(extension => extension.toLowerCase() === 'pdf'));
  const imageSource = $derived(extensions.some(extension => /^(jpe?g|png|webp|bmp|tiff?|gif|ico)$/iu.test(extension)));
  const allImages = $derived(sources.length > 0 && extensions.every(extension => /^(jpe?g|png|webp|bmp|tiff?|gif|ico)$/iu.test(extension)));
  const sheetSource = $derived(extensions.some(extension => /^(xlsx?|ods)$/iu.test(extension)));
  const activeJobs = $derived(jobs.filter(job => ['queued', 'running'].includes(job.status)));
  const statusLabels: Record<string, string> = { queued: '排队中', pending: '等待处理', running: '正在处理', completed: '已完成', failed: '失败', cancelled: '已取消', missing: '未安装', downloading: '下载中', installing: '安装中', ready: '已就绪' };
  const componentNames: Record<ConversionComponentId, string> = { office: 'Office 文档', pandoc: '结构化文档', pdf: 'PDF 文档', ocr: '文字识别' };

  const jobRefresh = createConversionRefresh(() => call<ConversionJob[]>('list_conversions'), value => { jobs = value; }, value => { error = conversionError(value); });
  const componentRefresh = createConversionRefresh(() => call<ConversionComponent[]>('get_conversion_components'), value => { components = value; }, value => { error = conversionError(value); });
  const csvSession = createCsvPreviewSession(request => call<CsvPreviewResult>('preview_conversion_csv', request), value => { csvState = value; });

  $effect(() => { saveConversionDraft({ sources, directory, options }); });
  $effect(() => {
    void refresh();
  });
  $effect(() => {
    const key = enabled ? csvKey : '';
    if (key !== lastCsvKey) { lastCsvKey = key; csvSession.reset(); }
  });

  async function refresh() {
    if (!current) return;
    loading = true;
    await Promise.all([jobRefresh.run(), ...(enabled ? [componentRefresh.run()] : [])]);
    if (current) loading = false;
  }

  function setSources(next: ConversionSource[]) {
    const retained = next.slice(0, 100);
    sources = retained;
    options = optionsForConversionSources(retained.map(source => source.extension), options);
    error = '';
  }

  async function chooseFiles() {
    if (pending || !enabled) return;
    pending = true; error = ''; notice = '';
    try {
      const selected = await call<ConversionSource[]>('choose_conversion_files');
      if (!current || !selected?.length) return;
      const unique = mergeConversionSources(sources, selected);
      setSources(unique);
      if (unique.length > 100) {
        await call('release_conversion_sources', { ids: unique.slice(100).map(source => source.id) });
        if (current) error = '每批最多 100 个文件，请分批处理。';
      }
    } catch (value) { if (current) error = conversionError(value); }
    finally { if (current) pending = false; }
  }

  async function removeSource(id: string) {
    if (pending) return;
    pending = true;
    setSources(sources.filter(source => source.id !== id));
    try { await call('release_conversion_sources', { ids: [id] }); }
    catch (value) { if (current) error = conversionError(value); }
    finally { if (current) pending = false; }
  }

  async function clearSources() {
    if (!enabled || pending || !sources.length) return;
    pending = true; error = ''; notice = '';
    try {
      // 批量释放成功后再清空草稿；后台任务持有的凭据由原生任务继续保留。
      await clearConversionDraftSources(sources.map(source => source.id), ids => call('release_conversion_sources', { ids }));
      if (current) { setSources([]); csvSession.reset(); notice = '已清空文件选择，转换选项和后台任务保留。'; }
    } catch (value) { if (current) error = conversionError(value); }
    finally { if (current) pending = false; }
  }

  async function chooseDirectory() {
    if (pending || !enabled) return;
    pending = true; error = '';
    try {
      const selected = await call<ConversionDirectory | null>('choose_conversion_directory');
      if (current && selected) directory = selected;
    } catch (value) { if (current) error = conversionError(value); }
    finally { if (current) pending = false; }
  }

  function moveSource(index: number, direction: number) {
    const destination = index + direction;
    if (destination < 0 || destination >= sources.length || pending) return;
    const next = [...sources];
    [next[index], next[destination]] = [next[destination], next[index]];
    sources = next;
  }

  function changeTarget() {
    options.merge = false;
    options.ocr = false;
    error = '';
  }

  async function previewCsv() {
    if (!enabled || pending || csvState.pending || !csvSources.length) return;
    error = '';
    await csvSession.preview(csvSources, options.csvDelimiter, options.csvEncoding);
  }

  export async function submit() {
    if (!enabled || pending) return;
    if (!sources.length) { await chooseFiles(); return; }
    if (invalid) { error = invalid; return; }
    if (missing.length) { error = '请先安装下方标记为“本次需要”的组件。'; return; }
    pending = true; error = ''; notice = '';
    try {
      await call<ConversionJob>('start_conversion', { sourceIds: sources.map(source => source.id), outputId: directory?.id ?? null, options: { ...options } });
      if (!current) return;
      notice = preview ? '已创建会话样例，浏览器不会读取或写入文件。' : '任务已加入队列，隐藏窗口后继续转换。';
      await jobRefresh.run();
    } catch (value) { if (current) error = conversionError(value); }
    finally { if (current) pending = false; }
  }

  async function componentAction(id: ConversionComponentId, action: 'install' | 'import' | 'cancel' | 'remove') {
    const key = `${action}:${id}`;
    if (!enabled || componentActions.includes(key)) return;
    componentActions = [...componentActions, key]; error = ''; notice = '';
    try {
      const command = { install: 'install_conversion_component', import: 'import_conversion_component', cancel: 'cancel_component_install', remove: 'remove_conversion_component' }[action];
      await call(command, { id });
      if (!current) return;
      await componentRefresh.run();
      if (action === 'install' || action === 'import') notice = '组件状态已更新；文件选择和转换选项已保留，请确认就绪后开始转换。';
    } catch (value) { if (current) error = conversionError(value); }
    finally { if (current) componentActions = componentActions.filter(item => item !== key); }
  }

  async function cancelJob(id: string) {
    error = '';
    try { await call('cancel_conversion', { id }); if (current) await jobRefresh.run(); }
    catch (value) { if (current) error = conversionError(value); }
  }

  async function openOutput(id: string, reveal: boolean) {
    try { await call('open_conversion_output', { id, reveal }); }
    catch (value) { if (current) error = conversionError(value); }
  }

  async function retryJob(job: ConversionJob, sourceId: string | null) {
    const request = conversionRetryRequest(job, sourceId);
    if (!enabled || !request || pending) return;
    pending = true; error = '';
    try {
      await call('retry_conversion', request);
      if (current) { notice = sourceId === null ? '已按原顺序重新提交合并任务，输出目录和选项保留。' : '已按原输出目录和选项重新提交失败项，其他结果保留。'; await jobRefresh.run(); }
    } catch (value) { if (current) error = conversionError(value); }
    finally { if (current) pending = false; }
  }

  onMount(() => {
    current = true;
    // 切换扩展不解除批量清空的互斥；回包后以最新草稿恢复，失败则继续保留选择。
    if (pendingClear) void pendingClear.then(() => {
      if (current) setSources(loadConversionDraft().sources);
    }).catch(value => { if (current) error = conversionError(value); }).finally(() => { if (current) pending = false; });
    const unsubscribers: (() => void)[] = [];
    for (const event of ['conversions_changed', 'conversion_components_changed'] as const) {
      void subscribe(event, () => { void refreshConversionEvent(event, jobRefresh, componentRefresh, enabled); }).then(unsubscribe => { if (current) unsubscribers.push(unsubscribe); else unsubscribe(); });
    }
    return () => { current = false; jobRefresh.dispose(); componentRefresh.dispose(); csvSession.dispose(); for (const unsubscribe of unsubscribers) unsubscribe(); };
  });
</script>

<section class="utility-panel conversion-panel" aria-label="格式转换">
  <header class="utility-heading"><ArrowRightLeft size={24} aria-hidden="true" /><div class="utility-title"><h2>格式转换</h2><p>{preview ? '会话样例 · 不读取或写入本地文件' : '本地处理 · 保留原文件'}</p></div><button class="icon-button" aria-label="刷新组件与任务状态" disabled={loading} onclick={refresh}><RefreshCw size={16} class={loading ? 'loading-icon' : ''} /></button></header>

  {#if !enabled}
    <div class="utility-empty"><ArrowRightLeft size={32} strokeWidth={1.5} aria-hidden="true" /><h3>格式转换尚未启用</h3><p>转换常见图片与文档，需要时安装本地组件。</p><button class="primary" onclick={openExtensions}>前往扩展设置</button></div>
  {:else}
    <section class="conversion-files" aria-labelledby="conversion-files-title">
      <div class="section-heading"><h3 id="conversion-files-title">待转换文件 <span>{sources.length ? `(${sources.length})` : ''}</span></h3><div class="button-row">{#if sources.length}<button class="text-button" disabled={pending} onclick={clearSources}>清空文件</button>{/if}<button class="secondary" disabled={pending} onclick={chooseFiles}><FilePlus2 size={15} aria-hidden="true" />选择文件</button></div></div>
      {#if sources.length}
        <ol class="source-list">{#each sources as source, index (source.id)}<li><span class="source-number" aria-hidden="true">{index + 1}</span><div class="source-name"><strong title={source.name}>{source.name}</strong><small>{formatBytes(source.bytes)}</small></div><button class="icon-button" aria-label={`上移 ${source.name}`} disabled={pending || index === 0} onclick={() => moveSource(index, -1)}><ArrowUp size={14} aria-hidden="true" /></button><button class="icon-button" aria-label={`下移 ${source.name}`} disabled={pending || index === sources.length - 1} onclick={() => moveSource(index, 1)}><ArrowDown size={14} aria-hidden="true" /></button><button class="icon-button" aria-label={`移除 ${source.name}`} disabled={pending} onclick={() => removeSource(source.id)}><X size={15} aria-hidden="true" /></button></li>{/each}</ol>
      {:else}<p class="utility-help empty-source">选择本地文件，或从搜索结果菜单选择“格式转换”。单批最多 100 个文件。</p>{/if}
    </section>

    {#if sources.length}
      <div class="conversion-fields">
        <label class="conversion-field"><span>目标格式</span><select aria-label="目标格式" bind:value={options.target} disabled={pending || !targets.length} onchange={changeTarget}>{#if !targets.length}<option value="">没有共同目标格式</option>{/if}{#each targets as target}<option value={target}>{targetLabel(target, extensions)}</option>{/each}</select></label>
        <div class="conversion-field"><span>输出目录</span><button class="directory-button" disabled={pending} onclick={chooseDirectory}><FolderOpen size={15} aria-hidden="true" /><span title={directory?.label}>{directory?.label ?? '各源文件所在目录'}</span></button>{#if directory}<button class="text-button" disabled={pending} onclick={() => directory = null}>恢复原目录</button>{/if}</div>
        {#if options.target === 'jpg'}<label class="conversion-field"><span>JPG 质量（1–100）</span><input aria-label="JPG 质量" type="number" min="1" max="100" bind:value={options.quality} disabled={pending} /></label>{/if}
        {#if options.target === 'ico'}<label class="conversion-field"><span>ICO 尺寸</span><select aria-label="ICO 尺寸" bind:value={options.icoSize} disabled={pending}>{#each [16, 32, 48, 64, 128, 256] as size}<option value={size}>{size} × {size}</option>{/each}</select></label>{/if}
        {#if pdfSource}<label class="conversion-field"><span>页面分辨率</span><select aria-label="页面分辨率" bind:value={options.dpi} disabled={pending}><option value={150}>150 DPI</option><option value={300}>300 DPI</option></select></label>{/if}
        {#if pdfSource || imageSource}<label class="conversion-field"><span>{pdfSource ? '页码范围' : '帧号 / 页码'}</span><input aria-label={pdfSource ? '页码范围' : '帧号或页码'} placeholder={pdfSource ? '留空为全部，如 1-3,5' : '多帧文件需选择，如 1'} bind:value={options.pages} disabled={pending} /></label>{/if}
        {#if options.target === 'csv' && sheetSource}<label class="conversion-field"><span>工作表序号（从 1 开始）</span><input aria-label="工作表序号" type="number" min="1" max="1000" bind:value={options.sheet} disabled={pending} /></label>{/if}
        {#if csvSources.length}
          <label class="conversion-field"><span>CSV 分隔符</span><select aria-label="CSV 分隔符" bind:value={options.csvDelimiter} disabled={pending}><option value=",">逗号 (,)</option><option value=";">分号 (;)</option><option value={'\t'}>制表符 (Tab)</option></select></label>
          <label class="conversion-field"><span>CSV 编码</span><select aria-label="CSV 编码" bind:value={options.csvEncoding} disabled={pending}><option value="utf-8">UTF-8</option><option value="gb18030">GB18030（简体中文）</option><option value="utf-16le">UTF-16 LE</option><option value="utf-16be">UTF-16 BE</option></select></label>
        {/if}
      </div>
      {#if csvSources.length}
        <section class="csv-preview" aria-labelledby="csv-preview-title">
          <div class="section-heading"><h3 id="csv-preview-title">CSV 导入预览</h3><button class="secondary" disabled={pending || csvState.pending || csvSources.some(source => source.bytes > 32 * 1024 * 1024)} onclick={previewCsv}>{csvState.pending ? `预览中 ${csvState.completed} / ${csvState.total}` : '预览 CSV'}</button></div>
          <p class="utility-help">每个 CSV 上限 32 MiB。当前编码与分隔符应用于全部 CSV；预览全部文件后才能转换。单元格按文本导入，不执行公式。</p>
          {#if csvSources.length > 1}<label class="conversion-field"><span>查看文件</span><select aria-label="查看 CSV 预览文件" value={csvDisplayId} onchange={(event) => csvSelectedId = event.currentTarget.value}>{#each csvSources as source (source.id)}<option value={source.id}>{source.name}</option>{/each}</select></label>{/if}
          {#if csvState.pending}<p class="utility-help" role="status">正在读取第 {Math.min(csvState.completed + 1, csvState.total)} 个 CSV…</p>{/if}
          {#if csvState.error}<p class="field-error" role="alert">{csvState.error}</p>{/if}
          {#if csvPreviewResult}
            <!-- svelte-ignore a11y_no_noninteractive_tabindex (滚动表格区域需要支持键盘聚焦后滚动。) -->
            <div class="csv-table-scroll" tabindex="0" role="region" aria-label="CSV 文本预览表格"><table><caption class="sr-only">{csvSources.find(source => source.id === csvDisplayId)?.name}，最多显示前 20 行、20 列</caption><tbody>{#each csvPreviewResult.rows as row, rowIndex}<tr><th scope="row">{rowIndex + 1}</th>{#each row as cell}<td>{cell}</td>{/each}</tr>{/each}</tbody></table></div>
            <p class="utility-help">{csvPreviewResult.truncated ? '内容已截断，仅显示前 20 行、20 列及每格前 2048 个字符。' : `已显示 ${csvPreviewResult.rows.length} 行。`}{csvReady ? ' 全部 CSV 已预览，可以转换。' : ''}</p>
          {:else if !csvState.pending}<p class="utility-help">请点击“预览 CSV”，检查中文、列分隔和引号是否正确。</p>{/if}
        </section>
      {/if}
      {#if options.target === 'pdf' && allImages}<label class="check-option"><input type="checkbox" bind:checked={options.merge} disabled={pending} /><span>按上方顺序合并为一个 PDF</span></label>{/if}
      {#if (pdfSource || allImages) && ['txt', 'docx', 'pdf'].includes(options.target)}<label class="check-option"><input type="checkbox" bind:checked={options.ocr} disabled={pending} /><span>使用本地文字识别（简体中文与英语）</span></label>{/if}
      <div class="conversion-caveats">
        {#if options.target === 'jpg'}<p>透明区域将填充白色，默认移除位置等隐私元数据；JPG 为有损格式。</p>{/if}
        {#if options.target === 'gif'}<p>GIF 颜色将量化，画质可能变化；动画、多页文件需先明确选择帧或页。</p>{/if}
        {#if options.target === 'csv'}<p>{sheetSource ? '只导出所选工作表，保留数据结果，不保留公式、样式和其他工作表。' : '每张检测到的表格单独保存；没有表格时会提示。'}</p>{/if}
        {#if pdfSource && options.target === 'docx'}<p>提取文字并重建可编辑段落，复杂排版不会完整还原。扫描件需要文字识别。</p>{/if}
        {#if pdfSource && options.target === 'xlsx'}<p>提取可检测的表格，每张表放入独立工作表；扫描表格可能无法恢复结构。</p>{/if}
        {#if pdfSource && options.target === 'pptx'}<p>每页作为一张图片放入幻灯片，页面中的文字和图形不能分别编辑。</p>{/if}
        {#if options.ocr && options.target === 'pdf'}<p>生成保留页面外观的可搜索 PDF；原书签、表单、附件及数字签名不保留。</p>{/if}
        <p>生成新文件，重名时自动追加后缀。Office 字体、动画和复杂样式可能发生变化。</p>
      </div>
      {#if invalid}<p class="field-help">{invalid}</p>{/if}
      <div class="conversion-submit"><span class="utility-help">{missing.length ? `本次需要：${missing.map(id => componentNames[id]).join('、')}` : '组件已就绪，可开始转换'}{activeJobs.length ? ` · ${activeJobs.length} 批处理中` : ''}</span><button class="primary" disabled={pending || !!invalid || missing.length > 0} onclick={submit}>{pending ? '正在提交…' : '开始转换'}</button></div>
    {/if}

    {#if error}<p class="field-error feedback" role="alert">{error}</p>{/if}
    {#if notice}<p class="feedback utility-help" role="status">{notice}</p>{/if}

    <details class="components-section" open={missing.length > 0 || !sources.length}>
      <summary>本地组件 <span>按需安装，安装后离线使用</span></summary>
      <div class="component-list" aria-busy={loading}>
        {#each components as component (component.id)}
          <article class="component-card">
            <div class="component-title"><h4>{component.name}</h4>{#if required.includes(component.id)}<span class="needed-badge">本次需要</span>{/if}<span class="component-state">{component.status === 'ready' ? '✓ ' : ''}{statusLabels[component.status] ?? '不可用'}</span></div>
            <p>{component.description}</p>
            <small>{component.version ? `版本 ${component.version} · ` : ''}{component.downloadBytes > 0 ? `${component.installationSource === 'download' ? '下载' : '组件包'} ${formatBytes(component.downloadBytes)} · ` : ''}{component.installedBytes > 0 ? `占用 ${formatBytes(component.installedBytes)} · ` : ''}{component.license}</small>
            {#if component.source}<small class="component-source">来源：{component.source}</small>{/if}
            {#if component.error}<p class="field-error">{conversionError(component.error)}</p>{/if}
            <div class="component-actions">
              {#if component.status === 'downloading' || component.status === 'installing' || componentActions.includes(`install:${component.id}`) || componentActions.includes(`import:${component.id}`)}<span class="utility-help" role="status">正在{component.status === 'downloading' ? '下载' : '处理'}，请稍候…</span><button class="secondary" disabled={componentActions.includes(`cancel:${component.id}`)} onclick={() => componentAction(component.id, 'cancel')}>取消安装</button>
              {:else if component.status === 'ready'}<span class="ready-label"><Check size={13} aria-hidden="true" />本机可用</span>{#if component.managed !== false}<button class="text-button" disabled={activeJobs.length > 0 || componentActions.includes(`remove:${component.id}`)} onclick={() => componentAction(component.id, 'remove')}>卸载组件</button>{:else}<span class="utility-help">在 Windows 中管理</span>{/if}
              {:else}{#if component.canInstall}<button class="secondary" onclick={() => componentAction(component.id, 'install')}><Download size={14} aria-hidden="true" />安装组件</button>{/if}<button class="text-button" onclick={() => componentAction(component.id, 'import')}>导入本地组件包</button>{/if}
            </div>
            {#if component.status !== 'ready'}<small>{component.installationSource === 'adjacent' ? '已找到随附组件包，点击安装即可，无需联网。' : component.installationSource === 'download' ? '点击安装将下载固定版本组件；离线时可导入同版本组件包。' : preview ? '请在桌面应用中安装组件。' : '未找到随附组件包，此版本没有可用下载。请保留完整发行包的 components 文件夹，或导入匹配的组件包。'}</small>{/if}
          </article>
        {:else}<p class="utility-help">{loading ? '正在检测本地组件…' : '暂无组件状态，请刷新后重试。'}</p>{/each}
      </div>
    </details>
  {/if}
    {#if !enabled && error}<p class="field-error feedback" role="alert">{error}</p>{/if}
    {#if jobs.length}
      <section class="conversion-jobs" aria-labelledby="conversion-jobs-title"><div class="section-heading"><h3 id="conversion-jobs-title">转换任务</h3><span class="utility-help">隐藏窗口后继续处理</span></div>
        {#each jobs as job (job.id)}<article class="conversion-job"><div class="job-heading"><strong>{targetLabel(job.target)} <span>{statusLabels[job.status] ?? '处理中'}</span></strong><span class="utility-help">{job.completed} / {job.total}</span>{#if job.status === 'queued' || job.status === 'running'}<button class="text-button" onclick={() => cancelJob(job.id)}>取消任务</button>{/if}{#if conversionRetryRequest(job, null)}<button class="text-button" disabled={!enabled || pending} onclick={() => retryJob(job, null)}>重试合并任务</button>{/if}</div>
          {#if job.status === 'running' || job.status === 'queued'}<progress max={Math.max(job.total, 1)} value={job.completed} aria-label="已完成文件数"></progress>{/if}
          {#each job.items as item, itemIndex (`${job.id}:${itemIndex}`)}<div class="job-item"><div class="job-item-heading"><span title={item.name}>{item.name}</span><small>{statusLabels[item.status] ?? '处理中'}</small>{#if item.sourceId && conversionRetryRequest(job, item.sourceId)}<button class="text-button" disabled={!enabled || pending} onclick={() => retryJob(job, item.sourceId!)}>重试此项</button>{/if}</div>{#if item.error}<p class="field-error">{conversionError(item.error)}</p>{/if}{#each item.outputs as output (output.id)}<div class="output-row"><span title={output.path}>{output.name}</span><button class="text-button" onclick={() => openOutput(output.id, false)}>打开</button><button class="icon-button" aria-label={`打开 ${output.name} 所在文件夹`} onclick={() => openOutput(output.id, true)}><FolderOpen size={15} aria-hidden="true" /></button></div>{/each}</div>{/each}
        </article>{/each}
      </section>
    {/if}
</section>

<style>
  .conversion-panel { gap: 16px; }
  .conversion-panel .utility-heading { margin-bottom: 0; }
  .section-heading, .job-heading { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  h3 { margin: 0; font-size: 14px; font-weight: 600; }
  h3 span { font-weight: 400; color: var(--secondary); }
  .source-list { list-style: none; margin: 10px 0 0; padding: 0; max-height: 155px; overflow: auto; border: 1px solid var(--border); border-radius: 8px; }
  .source-list li { display: flex; align-items: center; gap: 4px; padding: 5px 7px; border-bottom: 1px solid var(--border); }
  .source-list li:last-child { border-bottom: 0; }
  .source-number { flex: 0 0 22px; text-align: center; font-size: 12px; color: var(--secondary); }
  .source-name { flex: 1; min-width: 0; padding: 4px; }
  .source-name strong { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 500; }
  .source-name small { color: var(--secondary); font-size: 11px; }
  .empty-source { margin: 12px 0 0; padding: 18px 12px; background: var(--surface); border-radius: 8px; }
  .conversion-fields { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .conversion-field { display: flex; flex-direction: column; gap: 6px; min-width: 0; font-size: 12px; }
  .conversion-field > span { color: var(--secondary); }
  .conversion-field input, .conversion-field select, .directory-button { min-width: 0; width: 100%; height: 37px; padding: 7px 10px; border: 1px solid var(--border-strong); border-radius: 7px; background: var(--raised); color: var(--text); font: inherit; }
  .directory-button { display: flex; align-items: center; gap: 7px; text-align: left; }
  .directory-button span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .directory-button :global(svg) { flex-shrink: 0; }
  .conversion-field .text-button { justify-content: flex-start; align-self: flex-start; }
  .csv-preview { padding: 12px; border: 1px solid var(--border); border-radius: 8px; }
  .csv-preview .utility-help { margin: 8px 0 0; }
  .csv-preview .conversion-field { margin-top: 10px; }
  .csv-table-scroll { max-height: 210px; overflow: auto; margin-top: 10px; border: 1px solid var(--border); border-radius: 5px; }
  .csv-table-scroll table { width: 100%; border-collapse: collapse; font-size: 12px; }
  .csv-table-scroll th, .csv-table-scroll td { padding: 6px 8px; border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); text-align: left; vertical-align: top; white-space: pre-wrap; overflow-wrap: anywhere; min-width: 70px; max-width: 240px; }
  .csv-table-scroll th { min-width: 30px; color: var(--secondary); background: var(--surface); font-weight: 400; }
  .check-option { display: flex; align-items: center; gap: 9px; min-height: 30px; font-size: 12px; line-height: 1.6; }
  .check-option input { accent-color: var(--text); }
  .conversion-caveats { padding: 10px 12px; border-radius: 7px; background: var(--surface); color: var(--secondary); font-size: 12px; line-height: 1.7; }
  .conversion-caveats p { margin: 0; }
  .conversion-caveats p + p { margin-top: 5px; }
  .conversion-submit { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .conversion-submit .primary { flex-shrink: 0; }
  .feedback { margin: 0; }
  .components-section { border-top: 1px solid var(--border); padding-top: 12px; }
  .components-section summary { cursor: pointer; font-size: 13px; font-weight: 600; line-height: 1.8; }
  .components-section summary span { margin-left: 6px; font-size: 11px; font-weight: 400; color: var(--secondary); }
  .component-list { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 10px; }
  .component-card { min-width: 0; padding: 12px; border: 1px solid var(--border); border-radius: 8px; }
  .component-title { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  h4 { margin: 0; flex: 1; font-size: 13px; font-weight: 600; }
  .component-card p { margin: 6px 0; font-size: 12px; line-height: 1.7; color: var(--secondary); }
  .component-card small { display: block; color: var(--secondary); font-size: 11px; line-height: 1.7; overflow-wrap: anywhere; }
  .component-state { color: var(--secondary); font-size: 11px; white-space: nowrap; }
  .needed-badge { font-size: 10px; padding: 2px 5px; border-radius: 4px; background: var(--selected); color: var(--text); }
  .component-actions { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 7px; margin-top: 8px; }
  .component-actions .secondary { padding: 7px 9px; font-size: 12px; }
  .ready-label { display: flex; align-items: center; gap: 4px; font-size: 12px; }
  .conversion-jobs { border-top: 1px solid var(--border); padding-top: 14px; }
  .conversion-job { margin-top: 10px; padding: 12px; border: 1px solid var(--border); border-radius: 8px; }
  .job-heading strong { flex: 1; font-size: 13px; }
  .job-heading strong span { margin-left: 6px; font-size: 12px; font-weight: 400; color: var(--secondary); }
  progress { display: block; width: 100%; height: 5px; margin: 10px 0; accent-color: var(--text); }
  .job-item { padding-top: 8px; }
  .job-item-heading { display: flex; gap: 8px; align-items: center; }
  .job-item-heading > span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .job-item-heading small { color: var(--secondary); font-size: 11px; }
  .job-item p { margin: 5px 0; }
  .output-row { display: flex; align-items: center; gap: 8px; margin-top: 4px; padding: 4px 8px; border-radius: 5px; background: var(--surface); }
  .output-row > span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  button:focus-visible, input:focus-visible, select:focus-visible, summary:focus-visible { outline: 2px solid var(--text); outline-offset: 2px; }
  @media (max-width: 460px) {
    .conversion-panel { padding: 16px 14px; }
    .conversion-fields, .component-list { grid-template-columns: 1fr; }
    .conversion-submit { align-items: stretch; flex-direction: column; }
    .components-section summary span { display: block; margin-left: 15px; }
  }
</style>
