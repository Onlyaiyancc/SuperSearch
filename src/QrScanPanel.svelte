<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { ScanLine, Copy, ExternalLink, RefreshCw, AlertCircle, ArrowRight, Link } from 'lucide-svelte';
  import { call, preview } from './bridge';
  import { createQrScanSession, screenLinkWarning, type QrScanResult, type QrScanState } from './qr-scan';

  let { enabled, openExtensions }: { enabled: boolean; openExtensions: () => void } = $props();
  let scanState = $state<QrScanState>({ pending: false, acting: false, extracting: null, links: {}, result: null, selected: 0, error: '', notice: '' });
  let list = $state<HTMLDivElement>();
  const webLinks = $derived(scanState.result?.webLinks ?? []);
  const resultCount = $derived(webLinks.length + (scanState.result?.items.length ?? 0));
  const warning = $derived(screenLinkWarning(scanState.result?.webLinkWarning));
  const session = createQrScanSession({
    prepare: () => call<{ sessionId: string }>('prepare_qr_scan'),
    scan: sessionId => call<QrScanResult>('scan_qr_screen', { sessionId }),
    close: sessionId => call('close_qr_scan', { sessionId }),
    action: (sessionId, resultId, action) => call('qr_scan_action', { sessionId, resultId, action }),
    screenAction: (sessionId, linkId, action) => call('screen_web_link_action', { sessionId, linkId, action }),
  }, value => { scanState = value; });

  export function cancel() { session.cancel(); }
  export async function scan() { if (enabled) await session.scan(); }
  export async function submit() { if (enabled) await session.submit(); }
  export function move(direction: number) {
    session.move(direction);
    void tick().then(() => list?.querySelector('[aria-current="true"]')?.scrollIntoView({ block: 'nearest' }));
  }
  onDestroy(cancel);
</script>

<section class="translation-panel qr-panel" aria-label="屏幕扫码" aria-busy={scanState.pending}>
  <header class="translation-heading"><span class="extension-mark"><ScanLine size={22} /></span><div><h2>屏幕扫码</h2><p>识别屏幕网址与二维码</p></div><span class="extension-badge">{preview ? '合成样例' : '本地扩展'}</span></header>
  {#if !enabled}
    <div class="translation-empty"><ScanLine size={30} strokeWidth={1.4} /><h3>屏幕扫码尚未启用</h3><p>开启后，按回车识别当前屏幕上的网址和二维码。</p><button class="primary" onclick={openExtensions}>前往扩展设置 <ArrowRight size={15} /></button></div>
  {:else}
    <div class="translation-toolbar"><span class="translation-direction">{scanState.result ? `${webLinks.length} 个网页链接 · ${scanState.result.items.length} 个二维码` : '按回车或点击按钮开始扫描'}</span><button class="primary" disabled={scanState.pending || scanState.acting} onclick={() => session.scan()}>{#if scanState.pending}<RefreshCw size={15} class="loading-icon" />{:else}<ScanLine size={15} />{/if}{scanState.pending ? '扫描中…' : scanState.result || scanState.error ? '重新扫描' : '扫描当前屏幕'}</button></div>
    <div class="translation-body">
      {#if warning}<div class="screen-link-warning" role="status"><AlertCircle size={17} /><p>{warning}</p></div>{/if}
      {#if scanState.pending}<div class="translation-empty" role="status"><RefreshCw size={26} class="loading-icon" /><h3>正在扫描当前屏幕…</h3><p>搜索窗口将暂时隐藏。首次使用会准备本地文字识别组件，请稍候。</p></div>
      {:else if resultCount && scanState.result}
        <div bind:this={list} class="qr-results" aria-label="屏幕识别结果">
          {#if webLinks.length}
            <section class="screen-web-group" aria-label="网页链接">
              <h3 class="qr-group-heading">网页链接 <span>{webLinks.length} 个 · 来自屏幕文字</span></h3>
              <div class="qr-group-items">{#each webLinks as link, index (link.id)}
                <article class="qr-result screen-web-result" class:qr-selected={scanState.selected === index} aria-current={scanState.selected === index ? 'true' : undefined}>
                  <button class="screen-web-summary" onclick={() => session.select(index)} aria-label={`选择第 ${index + 1} 个网页链接`} aria-pressed={scanState.selected === index}><Link size={18} /><span><strong>{link.domain}</strong><span class="qr-link-url">{link.url}</span></span></button>
                  <div class="qr-actions"><button class="secondary" disabled={scanState.acting} onclick={() => session.action('copy', index)}><Copy size={14} />复制链接</button><button class="secondary" disabled={scanState.acting} onclick={() => session.action('open', index)}><ExternalLink size={14} />打开链接</button></div>
                </article>
              {/each}</div>
              {#if scanState.result.webLinksLimited}<p class="qr-limit" role="status">屏幕网页链接数量已达上限，部分链接未显示；请减少屏幕内容后重新扫描。</p>{/if}
            </section>
          {/if}
          {#if scanState.result.items.length}
            <section class="qr-code-group" aria-label="二维码">
              <h3 class="qr-group-heading">二维码 <span>{scanState.result.items.length} 个</span></h3>
              <div class="qr-group-items">{#each scanState.result.items as item, index (item.id)}
                <article class="qr-result" class:qr-selected={scanState.selected === webLinks.length + index} aria-current={scanState.selected === webLinks.length + index ? 'true' : undefined}>
                  <button class="qr-summary" onclick={() => session.select(webLinks.length + index)} aria-label={`选择第 ${index + 1} 个二维码`} aria-pressed={scanState.selected === webLinks.length + index}>
                    {#if item.thumbnail}<img src={item.thumbnail} alt={`第 ${index + 1} 个二维码缩略图`} width="72" height="72" />{:else}<span class="qr-placeholder" aria-hidden="true"><ScanLine size={28} strokeWidth={1.3} /></span>{/if}
                    <span><strong>{item.domain || `二维码 ${index + 1}`}</strong><span class="qr-text">{item.text}</span></span>
                  </button>
                  <div class="qr-actions"><button class="secondary" disabled={scanState.acting} onclick={() => session.action('copy', webLinks.length + index)}><Copy size={14} />复制</button>{#if item.domain}<button class="secondary" disabled={scanState.acting} onclick={() => session.action('open', webLinks.length + index)}><ExternalLink size={14} />打开链接</button>{/if}</div>
                </article>
              {/each}</div>
            </section>
          {/if}
        </div>
      {:else if scanState.result}<div class="translation-empty" role="status"><ScanLine size={30} /><h3>当前屏幕未找到网址或二维码</h3><p>请清晰显示 HTTP/HTTPS 网址文字或完整二维码后重新扫描。</p></div>
      {:else if !scanState.error}<div class="translation-empty"><ScanLine size={30} strokeWidth={1.4} /><h3>准备扫描当前屏幕</h3><p>输入 <code>扫码</code> / <code>scan</code>，按回车识别屏幕上显示的网址与二维码。网页链接优先展示，识别后可复制或打开。</p></div>{/if}
      {#if scanState.result?.limited}<p class="qr-limit" role="status">已达到数量或内容上限，部分二维码未显示。请减少屏幕上的二维码后重新扫描。</p>{/if}
    </div>
    {#if scanState.error || scanState.notice}<div class="qr-feedback">
      {#if scanState.error}<div class="translation-error" role="alert"><AlertCircle size={18} /><p>{scanState.error}</p></div>{/if}
      {#if scanState.notice}<p class="translation-notice" role="status">{preview ? '合成样例：未写入剪贴板或打开链接' : scanState.notice}</p>{/if}
    </div>{/if}
    <footer class="translation-footer"><p>{preview ? '浏览器仅展示合成样例，不捕获屏幕、联网、复制或打开链接。' : '同一画面在本机识别网址文字与二维码，不保存截图或历史。链接需点击按钮才打开。'}</p><span>{resultCount ? '↑ ↓ 选择 · Enter 复制所选内容' : 'Enter 扫描'} · Esc 关闭</span></footer>
  {/if}
</section>
