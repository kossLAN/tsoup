<script lang="ts">
	import type { FileDiff } from '$lib/api';

	let { diff }: { diff: FileDiff } = $props();

	const lines = $derived(
		diff.patch
			.split('\n')
			.filter((line) => line && !line.startsWith('---') && !line.startsWith('+++'))
	);

	function style(line: string): string {
		if (line.startsWith('@@')) return 'text-secondary';
		if (line.startsWith('+')) return 'bg-positive/12 text-positive';
		if (line.startsWith('-')) return 'bg-negative/12 text-negative';
		return '';
	}
</script>

<div class="overflow-x-auto py-1.5 font-mono text-label/relaxed">
	{#each lines as line, index (index)}
		<div class="w-max min-w-full px-3 whitespace-pre {style(line)}">{line}</div>
	{/each}
</div>
