<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		x,
		y,
		side,
		children
	}: { x: number; y: number; side: 'left' | 'right'; children: Snippet } = $props();
</script>

<div
	class="pointer-events-none absolute z-10 w-max max-w-72 rounded-control border border-light bg-alternate-base px-2.5 py-2 shadow-overlay"
	style:top="{y}px"
	style:left={side === 'right' ? `${x}px` : undefined}
	style:right={side === 'left' ? `${x}px` : undefined}
	role="tooltip"
>
	{@render children()}
</div>
