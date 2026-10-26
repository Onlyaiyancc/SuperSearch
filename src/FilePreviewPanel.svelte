<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { ArrowLeft, ArrowUpRight, ChevronLeft, ChevronRight, FileSearch, RefreshCw, ZoomIn, ZoomOut, Maximize } from 'lucide-svelte';
  import DOMPurify from 'dompurify';
  import { call, preview } from './bridge';
  import { markdownHtml, imagePreviewHeight, createPreviewSession, parsePreviewPage, previewZoomStep, previewReadingKey, formatPreviewBytes, type FilePreview, type PreviewPosition } from './file-preview';
  import OfficePreviewContent from './OfficePreviewContent.svelte';
  import type { ResultItem } from './types';

  let { item, requestId, close, open, onHeight }: { item: ResultItem; requestId: string; close: () => void; open: () => void; onHeight: (height: number) => void } = $props();
  let region: HTMLDivElement;
  let dimensions = $state({ width: 0, height: 0 });
  let content = $state<FilePreview | null>(null);
  let loading = $state(true);
  let error = $state('');
  let offsets = $state([0]);
  let segment = $state(0);
  let imageRevision = $state(0);
  let zoom = $state<number | null>(null);
  let pageInput = $state('1');
  let pageError = $state('');
  let disposed = false;
  let lastPosition: PreviewPosition = {};
  const previewId = crypto.randomUUID();
  const session = createPreviewSession(position => call<FilePreview>('preview_file', { requestId, resultId: item.resultId, previewId, ...position }), () => { void call('close_file_preview', { previewId }).catch(() => {}); }, () => {
    content = session.state.content; loading = session.state.loading; error = session.state.error;
    if (content && content.kind !== 'image') onHeight(620);
  });
  const html = $derived(content?.kind === 'markdown' ? DOMPurify.sanitize(markdownHtml(content.text), {
    ALLOWED_TAGS: ['p', 'br', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'strong', 'em', 'del', 'pre', 'code', 'ul', 'ol', 'li', 'blockquote', 'hr', 'table', 'thead', 'tbody', 'tr', 'th', 'td'], ALLOWED_ATTR: [],
  }) : '');
  const imageSource = $derived((content?.kind === 'image' || content?.kind === 'pdf') && /^image\/(png|jpeg|gif|webp|bmp)$/u.test(content.mime) && /^[A-Za-z0-9+/=]+$/u.test(content.data) ? `data:${content.mime};base64,${content.data}` : '');
  const contentType = $derived(({ image: '图片', pdf: 'PDF 文档', office: 'Office 文档', markdown: 'Markdown', text: '文本' })[content?.kind ?? ''] ?? '文件');

  async function load(position: PreviewPosition = {}) {
    lastPosition = position;
    const loaded = await session.load(position);
    if (loaded) {
      imageRevision++;
      pageInput = String((content?.pageIndex ?? 0) + 1); pageError = '';
      if (position.offset != null) {
        // 重试也在成功后提交片段位置，避免正文已翻段但返回按钮仍停在旧段。
        const known = offsets.indexOf(position.offset);
        if (known >= 0) segment = known;
        else { offsets = [...offsets.slice(0, segment + 1), position.offset]; segment++; }
      }
      await tick();
      if (!disposed && region) { region.scrollTop = 0; region.scrollLeft = 0; }
    }
    else if (!disposed && content?.kind === 'pdf' && !session.state.loading) pageInput = String((content.pageIndex ?? 0) + 1);
    return loaded;
  }

  async function moveText(direction: -1 | 1) {
    const target = segment + direction;
    const offset = direction < 0 ? offsets[target] : content?.nextOffset;
    if (offset == null) return;
    await load({ offset });
  }

  async function jumpPage(event: SubmitEvent) {
    event.preventDefault();
    const page = parsePreviewPage(pageInput, content?.pageCount ?? 0);
    if (page == null) { pageError = `请输入 1 到 ${content?.pageCount ?? 0} 的整数页码。`; return; }
    if (page === content?.pageIndex) { pageError = ''; region.focus({ preventScroll: true }); return; }
    if (await load({ page })) region?.focus({ preventScroll: true });
  }

  async function setZoom(value: number | null) {
    const previous = zoom;
    const horizontal = (region.scrollLeft + region.clientWidth / 2) / Math.max(1, region.scrollWidth);
    const vertical = (region.scrollTop + region.clientHeight / 2) / Math.max(1, region.scrollHeight);
    zoom = value;
    if (zoom !== null) onHeight(620);
    await tick();
    if (disposed) return;
    if (zoom === null) { region.scrollTop = 0; region.scrollLeft = 0; resizeImage(); }
    else if (previous !== null) {
      region.scrollLeft = horizontal * region.scrollWidth - region.clientWidth / 2;
      region.scrollTop = vertical * region.scrollHeight - region.clientHeight / 2;
    }
  }

  function stepZoom(direction: -1 | 1) {
    const image = region.querySelector('img');
    const current = zoom ?? (image && dimensions.width ? image.getBoundingClientRect().width / dimensions.width : 1);
    void setZoom(previewZoomStep(current, direction));
  }

  function readingKey(event: KeyboardEvent) {
    if (!content || loading || disposed || !region?.closest('.file-preview-panel')?.contains(document.activeElement)) return;
    const target = event.target instanceof Element ? event.target : null;
    const action = previewReadingKey(event, {
      kind: content.kind, fit: zoom === null,
      blocked: !!target?.closest('input, textarea, select, button, [contenteditable]:not([contenteditable="false"])'),
      previous: content.kind === 'pdf' ? (content.pageIndex ?? 0) > 0 : segment > 0,
      next: content.kind === 'pdf' ? (content.pageIndex ?? 0) + 1 < (content.pageCount ?? 0) : content.nextOffset != null,
    });
    if (!action) return;
    event.preventDefault();
    if (action === 'zoomIn' || action === 'zoomOut') stepZoom(action === 'zoomIn' ? 1 : -1);
    else if (action === 'fit') void setZoom(null);
    else if (content.kind === 'pdf') void load({ page: (content.pageIndex ?? 0) + (action === 'next' ? 1 : -1) });
    else void moveText(action === 'next' ? 1 : -1);
  }

  function resizeImage() {
    if (zoom !== null || content?.kind !== 'image' || !dimensions.width || !region || error) return;
    const shell = region.closest('.app-shell');
    if (!shell) return;
    const style = getComputedStyle(region);
    const vertical = parseFloat(style.paddingTop) + parseFloat(style.paddingBottom) + parseFloat(style.borderTopWidth) + parseFloat(style.borderBottomWidth);
    const horizontal = parseFloat(style.paddingLeft) + parseFloat(style.paddingRight);
    // 高度只由原图比例、内容宽度和固定控件决定，不使用当前显示图高，避免反复收缩。
    const chrome = shell.getBoundingClientRect().height - region.getBoundingClientRect().height + vertical;
    onHeight(imagePreviewHeight(dimensions.width, dimensions.height, region.clientWidth - horizontal, chrome));
  }

  onMount(() => {
    const observer = new ResizeObserver(resizeImage);
    observer.observe(region);
    void load().then(() => { if (!disposed) region?.focus({ preventScroll: true }); });
    return () => { disposed = true; observer.disconnect(); session.dispose(); };
  });
</script>

<svelte:window onkeydown={readingKey} />

<section class="utility-panel file-preview-panel" aria-label="文件快速预览" aria-busy={loading}>
  <header class="utility-heading">
    <button class="icon-button" aria-label="返回搜索结果" onclick={close}><ArrowLeft size={20} /></button>
    <div class="utility-title"><h2>{item.title}</h2><p>{preview ? '预览样例 · 不读取本地文件' : '本地文件预览 · Esc 返回'}</p></div>
    <button class="secondary" onclick={open}>打开文件 <ArrowUpRight size={14} /></button>
  </header>
  {#if content}
    <div class="preview-metadata"><span>{contentType}{formatPreviewBytes(content.totalBytes) ? ` · ${formatPreviewBytes(content.totalBytes)}` : ''}</span><span class="preview-read-status" role="status">{loading ? '正在读取…' : ''}</span></div>
  {/if}
  {#if imageSource}
    <div class="preview-toolbar" role="group" aria-label="预览缩放">
      <span class="preview-zoom-label" aria-live="polite">{zoom === null ? '适应窗口' : `${Math.round(zoom * 100)}%`}</span>
      <button class="icon-button" aria-label="缩小预览" title="缩小 · Ctrl+-" disabled={loading || !dimensions.width || zoom === 0.1} onclick={() => stepZoom(-1)}><ZoomOut size={17} /></button>
      <button class="icon-button" aria-label="放大预览" title="放大 · Ctrl++" disabled={loading || !dimensions.width || zoom === 4} onclick={() => stepZoom(1)}><ZoomIn size={17} /></button>
      <button class="preview-tool-button" disabled={loading || !dimensions.width} onclick={() => setZoom(1)}>100%</button>
      <button class="preview-tool-button" aria-label="适应窗口" title="适应窗口 · Ctrl+0" aria-pressed={zoom === null} disabled={loading} onclick={() => setZoom(null)}><Maximize size={15} /><span>适应窗口</span></button>
    </div>
  {/if}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (滚动正文需要键盘焦点以支持方向键和翻页键) -->
  <div bind:this={region} class="preview-content" class:image-preview-content={!!imageSource && zoom === null && !error} class:zoomed-preview-content={!!imageSource && zoom !== null} tabindex="0" role="region" aria-label="文件内容">
    {#if loading && !content}<div class="utility-empty"><RefreshCw class="loading-icon" size={25} /><p>正在读取文件…</p></div>
    {:else if error && !content}<div class="utility-empty" role="alert"><FileSearch size={30} /><p>{error}</p><button class="secondary" onclick={() => load(lastPosition)}>重试</button></div>
    {:else if content?.kind === 'image' || content?.kind === 'pdf'}
      {#if imageSource}<div class="preview-raster-stage">{#key imageRevision}<img class="preview-image" style:width={zoom === null ? undefined : `${dimensions.width * zoom}px`} style:height={zoom === null ? undefined : `${dimensions.height * zoom}px`} src={imageSource} alt={item.title} onload={(event) => { const image = event.currentTarget as HTMLImageElement; dimensions = { width: image.naturalWidth, height: image.naturalHeight }; resizeImage(); }} onerror={() => { error = '图片无法解码，请使用关联应用打开。'; onHeight(320); }} />{/key}</div>{:else}<p>图片格式无法预览。</p>{/if}
    {:else if content?.kind === 'office'}<OfficePreviewContent sections={content.sections ?? []} />
    {:else if content?.kind === 'markdown'}<article class="markdown-preview">{@html html}</article>
    {:else}<pre class="text-preview">{content?.text || '（空文件）'}</pre>{/if}
  </div>
  {#if content?.kind === 'pdf'}
    <nav class="preview-pagination" aria-label="PDF 翻页">
      <button class="secondary" aria-label="上一页" title="上一页 · PageUp" disabled={loading || !content.pageIndex} onclick={() => load({ page: (content?.pageIndex ?? 0) - 1 })}><ChevronLeft size={16} />上一页</button>
      <form class="preview-page-jump" onsubmit={jumpPage}>
        <label>第 <input aria-label="跳转页码" type="text" inputmode="numeric" maxlength="10" bind:value={pageInput} aria-invalid={!!pageError} aria-describedby={pageError ? 'preview-page-error' : undefined} disabled={loading} /> / {content.pageCount} 页</label>
        <button class="preview-tool-button" disabled={loading}>{loading ? '读取中…' : '跳转'}</button>
      </form>
      <button class="secondary" aria-label="下一页" title="下一页 · PageDown" disabled={loading || (content.pageIndex ?? 0) + 1 >= (content.pageCount ?? 0)} onclick={() => load({ page: (content?.pageIndex ?? 0) + 1 })}>下一页<ChevronRight size={16} /></button>
    </nav>
    {#if pageError}<p id="preview-page-error" class="preview-note" role="alert">{pageError}</p>{/if}
  {:else if content && (content.nextOffset != null || segment > 0)}
    <nav class="preview-pagination" aria-label="文本分段">
      <button class="secondary" disabled={loading || segment === 0} onclick={() => moveText(-1)}><ChevronLeft size={16} />上一段</button>
      <span>{`第 ${segment + 1} 段`}</span>
      <button class="secondary" disabled={loading || content.nextOffset == null} onclick={() => moveText(1)}>下一段<ChevronRight size={16} /></button>
    </nav>
  {:else if content?.truncated}<p class="preview-note">内容较多，仅显示有界预览；请打开文件查看完整内容。</p>{/if}
  {#if error && content}<p class="preview-note" role="alert">{error}<button class="text-button" disabled={loading} onclick={() => load(lastPosition)}>重试</button></p>{/if}
</section>
