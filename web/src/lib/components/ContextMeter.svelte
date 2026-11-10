<script lang="ts">
	import type { Context } from '$lib/api';
	import { tooltip } from '$lib/popup';

	let {
		context,
		disabled,
		oncompact
	}: { context: Context | null; disabled: boolean; oncompact: () => void } = $props();

	const radius = 6;
	const length = 2 * Math.PI * radius;

	const used = $derived(context ? Math.min(context.used / context.window, 1) : 0);
	const hint = $derived(
		context
			? `${Math.round(used * 100)}% of the context used, click to compact`
			: 'Compact the conversation'
	);
</script>

<button
	class="icon-button"
	{disabled}
	onclick={oncompact}
	aria-label={hint}
	{@attach tooltip(hint)}
>
	<svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke-width="2">
		<circle class="stroke-light" cx="8" cy="8" r={radius} />
		{#if used > 0}
			<circle
				class={used >= 0.8 ? 'stroke-danger' : 'stroke-accent'}
				cx="8"
				cy="8"
				r={radius}
				stroke-linecap="round"
				stroke-dasharray="{used * length} {length}"
				transform="rotate(-90 8 8)"
			/>
		{/if}
	</svg>
</button>
