<script lang="ts">
	import { CaretRightIcon } from '$lib/icons';
	import Markdown from './Markdown.svelte';

	let { summary }: { summary: string } = $props();

	let open = $state(false);
</script>

<div class="flex flex-col gap-2">
	<div class="flex items-center gap-3 text-label text-secondary">
		<span class="h-px flex-1 bg-light"></span>
		<button
			class="flex items-center gap-1.5 transition-colors hover:text-window-text"
			onclick={() => (open = !open)}
		>
			Context compacted
			<span class={['inline-flex shrink-0 transition', open && 'rotate-90']}>
				<CaretRightIcon size={10} />
			</span>
		</button>
		<span class="h-px flex-1 bg-light"></span>
	</div>
	{#if open}
		<div class="max-h-105 overflow-y-auto rounded-control surface px-3 py-2">
			<Markdown text={summary} muted />
		</div>
	{/if}
</div>
