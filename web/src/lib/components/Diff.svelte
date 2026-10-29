<script lang="ts">
	import type { FileDiff } from '$lib/api';

	let { diff }: { diff: FileDiff } = $props();

	const lines = $derived(
		diff.patch
			.split('\n')
			.filter((line) => line && !line.startsWith('---') && !line.startsWith('+++'))
	);

	function kind(line: string): string {
		if (line.startsWith('@@')) return 'hunk';
		if (line.startsWith('+')) return 'added';
		if (line.startsWith('-')) return 'removed';
		return 'context';
	}
</script>

<div class="diff">
	{#each lines as line, index (index)}
		<div class={kind(line)}>{line}</div>
	{/each}
</div>

<style>
	.diff {
		padding: 6px 0;
		font-family: var(--font-mono);
		font-size: var(--font-label);
		line-height: 1.6;
		overflow-x: auto;
	}

	.diff div {
		padding: 0 12px;
		white-space: pre;
		width: max-content;
		min-width: 100%;
	}

	.hunk {
		color: var(--text-secondary);
	}

	.added {
		background: var(--positive-tint);
		color: var(--positive);
	}

	.removed {
		background: var(--negative-tint);
		color: var(--negative);
	}
</style>
