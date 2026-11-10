<script lang="ts">
	import type { FileDiff } from '$lib/api';
	import { escape, highlightLines, languageOf } from '$lib/highlight';

	type Line = { sign: string; code: string };

	let { diff }: { diff: FileDiff } = $props();

	const lines = $derived.by((): Line[] => {
		const raw = diff.patch
			.split('\n')
			.filter((line) => line && !line.startsWith('---') && !line.startsWith('+++'));
		const code = highlightLines(
			raw.map((line) => (line.startsWith('@@') ? '' : line.slice(1))).join('\n'),
			languageOf(diff.path)
		);
		return raw.map((line, index) =>
			line.startsWith('@@')
				? { sign: '@', code: escape(line) }
				: { sign: line[0], code: code[index] }
		);
	});

	const styles: Record<string, string> = {
		'@': 'text-secondary',
		'+': 'bg-positive/12',
		'-': 'bg-negative/12'
	};

	const signs: Record<string, string> = {
		'+': 'text-positive',
		'-': 'text-negative'
	};
</script>

<div class="overflow-x-auto py-1.5 font-mono text-label/relaxed">
	{#each lines as line, index (index)}
		<div class={['w-max min-w-full px-3 whitespace-pre', styles[line.sign]]}>
			{#if line.sign !== '@'}<span class={['select-none', signs[line.sign]]}>{line.sign}</span
				>{/if}{@html line.code}
		</div>
	{/each}
</div>
