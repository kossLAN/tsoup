<script lang="ts">
	import { PlugsConnectedIcon } from '$lib/icons';
	import type { Brand } from '$lib/providers';

	let { brand, size = 16 }: { brand?: Brand; size?: number } = $props();
</script>

{#if brand}
	<span
		class="inline-block shrink-0 bg-current mask-contain mask-center mask-no-repeat"
		style:width="{size}px"
		style:height="{size}px"
		style:mask-image="url({brand.icon})"
		aria-hidden="true"
	></span>
{:else}
	<PlugsConnectedIcon {size} />
{/if}
