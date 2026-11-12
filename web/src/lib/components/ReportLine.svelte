<script lang="ts">
	import type { AgentSummary, ReportEntry } from '$lib/api';
	import { CaretRightIcon, WarningCircleIcon } from '$lib/icons';
	import Markdown from './Markdown.svelte';

	let { report, agents }: { report: ReportEntry; agents: AgentSummary[] } = $props();

	let open = $state(false);

	const name = $derived(agents.find(({ id }) => id === report.agent)?.name ?? report.agent);

	const label = $derived(
		{
			running: 'agent reported',
			finished: 'agent reported',
			failed: 'agent failed',
			stopped: 'agent stopped'
		}[report.state]
	);

	const failed = $derived(report.state === 'failed');
</script>

<div class="flex min-w-0 flex-col gap-1">
	<button
		class="group flex h-5.5 max-w-full items-center gap-1.5 text-secondary transition-colors hover:text-window-text disabled:pointer-events-none"
		disabled={!report.text}
		onclick={() => (open = !open)}
	>
		<span class="shrink-0">{label}</span>
		<span class="truncate font-mono text-label">{name}</span>
		{#if failed}
			<span class="inline-flex shrink-0"><WarningCircleIcon size={12} /></span>
		{/if}
		{#if report.text}
			<span
				class={[
					'inline-flex shrink-0 transition',
					open ? 'rotate-90' : 'opacity-0 group-hover:opacity-100'
				]}
			>
				<CaretRightIcon size={10} />
			</span>
		{/if}
	</button>

	{#if open && report.text}
		<div class="max-h-105 overflow-y-auto rounded-control surface px-3 py-2">
			{#if failed}
				<pre
					class="text-label/normal wrap-break-word whitespace-pre-wrap text-danger">{report.text}</pre>
			{:else}
				<Markdown text={report.text} muted />
			{/if}
		</div>
	{/if}
</div>
