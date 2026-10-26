<script lang="ts">
  import type { OfficeSection } from './file-preview';
  let { sections }: { sections: OfficeSection[] } = $props();
  let selected = $state(0);
  const current = $derived(sections[selected]);
</script>

<div class="office-preview">
  <div class="office-section-bar">
    {#if sections.length > 1}
      <label>章节 / 工作表 <select aria-label="章节或工作表" bind:value={selected}>{#each sections as section, index}<option value={index}>{section.title}</option>{/each}</select></label>
    {:else if current}<strong>{current.title}</strong>{/if}
    <span>内容预览，不完整保留原排版</span>
  </div>
  {#if current}
    {#if current.text}<pre class="text-preview">{current.text}</pre>{/if}
    {#if current.rows.length}
      <table class="office-table" aria-label={current.title}><tbody>{#each current.rows as row}<tr>{#each row as cell}<td>{cell}</td>{/each}</tr>{/each}</tbody></table>
    {:else if !current.text}<p class="preview-note">（无可预览内容）</p>{/if}
  {:else}<p class="preview-note">（空文档）</p>{/if}
</div>
