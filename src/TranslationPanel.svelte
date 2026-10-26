<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import { Languages, BookOpen, Copy, RefreshCw, AlertCircle, ArrowRight } from 'lucide-svelte';
  import { call, preview } from './bridge';
  import { createTranslationSession, translationError } from './translation';
  import type { TranslationQuery, TranslationRequest, TranslationResult, TranslationState } from './translation';

  let { query, enabled, active = true, openExtensions }: { query: TranslationQuery; enabled: boolean; active?: boolean; openExtensions: () => void } = $props();
  let manualDirection = $state<TranslationRequest['direction'] | null>(null);
  const direction = $derived(manualDirection ?? (/\p{Script=Han}/u.test(query.text) ? 'zh-en' : 'en-zh'));
  let lookupState = $state<TranslationState>({ pending: false, result: null, error: '' });
  let copying = $state(false);
  let notice = $state('');
  let disposed = false;
  let copyEpoch = 0;
  const session = createTranslationSession(request => call<TranslationResult>('lookup_translation', request), value => { lookupState = value; });
  const dictionary = $derived(query.mode === 'dictionary');
  const bytes = $derived(new TextEncoder().encode(query.text).length);
  const invalid = $derived(!query.text ? '请在命令后输入内容。' : dictionary
    ? !/^(?=.*[a-z])[a-z'-]{1,80}$/iu.test(query.text) ? '请输入单个英文单词，可包含连字符或英文撇号。' : ''
    : bytes > 500 ? '文本超过 500 个 UTF-8 字节，请缩短后再试。' : '');

  function reset() {
    session.reset();
    copyEpoch++;
    copying = false;
    notice = '';
  }

  $effect(() => { if (!active) manualDirection = null; });
  $effect(() => {
    // 编辑只失效旧结果，不重建面板或自动发送文本；手动方向在本次使用中保留。
    query.text; query.mode; direction; enabled; active;
    untrack(reset);
  });

  async function lookup() {
    if (!active || !enabled || invalid || lookupState.pending || copying) return;
    notice = '';
    await session.submit({ ...query, direction });
  }

  async function copyResult() {
    const result = lookupState.result;
    if (!result || !active || !enabled || copying) return;
    copying = true;
    const epoch = ++copyEpoch;
    const text = dictionary
      ? [result.text, result.phonetic, ...result.meanings.map(item => `${item.partOfSpeech} ${item.definition}${item.example ? `\n例句：${item.example}` : ''}`), `来源：${result.source}`].filter(Boolean).join('\n')
      : result.text;
    try {
      await call('copy_translation', { text });
      if (!disposed && epoch === copyEpoch) notice = preview ? '预览样例不会写入系统剪贴板' : '已复制到剪贴板';
    } catch (value) {
      if (!disposed && epoch === copyEpoch) lookupState = { ...lookupState, error: translationError(value) };
    } finally {
      if (!disposed && epoch === copyEpoch) copying = false;
    }
  }

  export async function submit() {
    if (lookupState.result) await copyResult(); else await lookup();
  }

  onDestroy(() => { disposed = true; reset(); });
</script>

<section class="translation-panel" aria-label={dictionary ? '英文词典' : '中英翻译'} aria-busy={lookupState.pending}>
  <header class="translation-heading">
    <span class="extension-mark">{#if dictionary}<BookOpen size={22} />{:else}<Languages size={22} />{/if}</span>
    <div><h2>{dictionary ? '英文词典' : '中英翻译'}</h2><p>{dictionary ? '音标、英文释义与例句' : '短文本翻译，按回车提交'}</p></div>
    <span class="extension-badge">{preview ? '样例预览' : '在线扩展'}</span>
  </header>

  {#if !enabled}
    <div class="translation-empty"><Languages size={30} strokeWidth={1.4} /><h3>翻译与词典尚未启用</h3><p>开启后，可在这里翻译中英短文本或查询英文单词。</p><button class="primary" onclick={openExtensions}>前往扩展设置 <ArrowRight size={15} /></button></div>
  {:else}
    <div class="translation-toolbar">
      {#if dictionary}<span class="translation-direction">英语 → 英文释义</span>
      {:else}<label class="sr-only" for="translation-direction">翻译方向</label><select id="translation-direction" value={direction} onchange={(event) => { manualDirection = event.currentTarget.value as TranslationRequest['direction']; reset(); }}><option value="en-zh">英语 → 简体中文</option><option value="zh-en">简体中文 → 英语</option></select>{/if}
      <button class="primary" disabled={!!invalid || lookupState.pending || copying} onclick={lookup}>{#if lookupState.pending}<RefreshCw size={15} class="loading-icon" />{/if}{lookupState.pending ? '查询中…' : lookupState.error ? '重试' : dictionary ? '查词' : '翻译'}</button>
    </div>
    <div class="translation-body">
      {#if lookupState.pending}
        <div class="translation-empty" role="status"><RefreshCw size={26} class="loading-icon" /><h3>正在{dictionary ? '查询单词' : '翻译'}…</h3><p>公共服务可能需要几秒钟，请稍候。</p></div>
      {:else if lookupState.result}
        <article class="translation-result">
          <div class="translation-result-heading"><h3>{dictionary ? lookupState.result.text : '译文'}</h3><button class="secondary" disabled={copying} onclick={copyResult}><Copy size={14} />{copying ? '复制中…' : '复制'}</button></div>
          {#if dictionary}
            {#if lookupState.result.phonetic}<p class="translation-phonetic">{lookupState.result.phonetic}</p>{/if}
            <ol class="dictionary-meanings">{#each lookupState.result.meanings as meaning}<li><span class="dictionary-pos">{meaning.partOfSpeech}</span><p>{meaning.definition}</p>{#if meaning.example}<blockquote>{meaning.example}</blockquote>{/if}</li>{/each}</ol>
          {:else}<p class="translated-text">{lookupState.result.text}</p>{/if}
          <p class="translation-source">来源：{lookupState.result.source}{preview ? ' · 合成样例' : ''}</p>
        </article>
      {:else if !lookupState.error}
        <div class="translation-empty"><span class="translation-key">↵</span><h3>{invalid || `按回车${dictionary ? '查询单词' : '翻译文本'}`}</h3><p>{dictionary ? '例如 查词 hello / dictionary hello；中文含义请用 翻译 hello。' : `例如 翻译 hello / translate hello · ${bytes} / 500 字节`}</p></div>
      {/if}
      {#if lookupState.error}<div class="translation-error" role="alert"><AlertCircle size={18} /><p>{lookupState.error}</p></div>{/if}
      {#if notice}<p class="translation-notice" role="status">{notice}</p>{/if}
    </div>
    <footer class="translation-footer"><p>{preview ? '预览仅展示合成样例，不连接服务或写入剪贴板。' : `按回车或点击按钮才向 ${dictionary ? 'Free Dictionary API' : 'MyMemory'} 发送文本；公共服务有额度与可用性限制。`}</p><span>{lookupState.result ? 'Enter 复制结果' : 'Enter 提交'} · Esc 关闭</span></footer>
  {/if}
</section>
