<script lang="ts">
  let { size = 32, class: className = '' } = $props<{ size?: number; class?: string }>();
</script>

<!-- 界面与安装包共用同一份银白品牌图标。 -->
<img src="/supersearch.png" width={size} height={size} class={`brand-mark ${className}`} alt="" aria-hidden="true" />
