<script lang="ts">
  import { AppWindow, File, FileText, FileSpreadsheet, FileType, Folder, Figma, FileImage, FileArchive, FileCode } from 'lucide-svelte';
  import type { ResultItem } from './types';

  let { item }: { item: Pick<ResultItem, 'kind' | 'title'> } = $props();
  // 按类型和扩展名展示图标；不读取或猜测本地文件内容。
  const extension = $derived(item.title.split('.').at(-1)?.toLocaleLowerCase());
  const type = $derived(item.kind === 'application' ? 'application' : item.kind === 'folder' ? 'folder' : ['xlsx', 'xls', 'csv', 'ods'].includes(extension ?? '') ? 'spreadsheet' : extension === 'pdf' ? 'pdf' : ['doc', 'docx', 'odt', 'md', 'txt', 'rtf'].includes(extension ?? '') ? 'document' : 'file');
</script>

<span class="result-icon" class:application-icon={type === 'application'} class:folder-icon={type === 'folder'} class:spreadsheet-icon={type === 'spreadsheet'} class:pdf-icon={type === 'pdf'} class:document-icon={type === 'document'} class:file-icon={type === 'file'} aria-hidden="true">
  {#if type === 'application'}<AppWindow size={40} strokeWidth={1.7} />
  {:else if type === 'folder'}<Folder size={42} strokeWidth={1.6} />
  {:else if type === 'spreadsheet'}<FileSpreadsheet size={40} strokeWidth={1.7} />
  {:else if type === 'pdf'}<FileType size={40} strokeWidth={1.7} />
  {:else if type === 'document'}<FileText size={40} strokeWidth={1.7} />
  {:else if extension === 'fig'}<Figma size={34} strokeWidth={1.7} />
  {:else if ['png', 'jpg', 'jpeg', 'webp', 'svg'].includes(extension ?? '')}<FileImage size={34} strokeWidth={1.7} />
  {:else if ['zip', 'rar', '7z'].includes(extension ?? '')}<FileArchive size={34} strokeWidth={1.7} />
  {:else if ['ts', 'js', 'rs', 'py', 'json', 'html', 'css'].includes(extension ?? '')}<FileCode size={34} strokeWidth={1.7} />
  {:else}<File size={34} strokeWidth={1.7} />{/if}
</span>
