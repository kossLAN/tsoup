<script lang="ts">
	import type { Entry } from '$lib/api';
	import { WarningCircleIcon } from '$lib/icons';
	import { tick } from 'svelte';
	import Markdown from './Markdown.svelte';
	import Spinner from './Spinner.svelte';
	import ToolCallLine from './ToolCallLine.svelte';
	import Working from './Working.svelte';

	let {
		entries,
		start,
		busy,
		failure,
		onolder
	}: {
		entries: Entry[];
		start: number;
		busy: boolean;
		failure: string | null;
		onolder: () => Promise<void>;
	} = $props();

	const threshold = 400;

	let scroller: HTMLElement;
	let pinned = true;
	let anchor: number | null = null;
	let end = 0;
	let loading = false;

	const results = $derived(
		new Map(
			entries.flatMap((entry) =>
				entry.kind === 'tool' ? [[entry.call_id, entry] as const] : []
			)
		)
	);

	$effect.pre(() => {
		void start;
		if (scroller && !pinned) {
			anchor = scroller.scrollHeight - scroller.scrollTop;
		}
	});

	$effect(() => {
		void busy;
		void failure;
		const total = start + entries.length;
		if (total > end && entries.at(-1)?.kind === 'user') {
			pinned = true;
		}
		end = total;

		if (pinned) {
			scroller.scrollTop = scroller.scrollHeight;
		} else if (anchor !== null) {
			scroller.scrollTop = scroller.scrollHeight - anchor;
		}
		anchor = null;
		older();
	});

	async function older() {
		if (loading) return;
		loading = true;
		try {
			while (start > 0 && scroller.scrollTop < threshold) {
				const before = start;
				await onolder();
				await tick();
				if (start === before) break;
			}
		} finally {
			loading = false;
		}
	}

	function scrolled() {
		pinned = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40;
		older();
	}
</script>

<div class="flex-1 overflow-y-auto" bind:this={scroller} onscroll={scrolled}>
	<div class="mx-auto flex max-w-chat flex-col gap-2.5 px-6 pt-6 pb-3">
		{#if start > 0}
			<div class="flex justify-center"><Spinner size={14} kind="simmer" /></div>
		{/if}

		{#each entries as entry, index (start + index)}
			{#if entry.kind === 'user'}
				<div
					class="max-w-4/5 self-end bubble text-title/relaxed wrap-break-word whitespace-pre-wrap"
				>
					{entry.text}
				</div>
			{:else if entry.kind === 'assistant'}
				{#if entry.text}
					<Markdown text={entry.text} />
				{/if}
				{#if entry.tool_calls.length}
					<div class="flex flex-col gap-0.5">
						{#each entry.tool_calls as call (call.id)}
							<ToolCallLine {call} result={results.get(call.id)} pending={busy} />
						{/each}
					</div>
				{/if}
			{/if}
		{/each}

		{#if busy}
			<Working />
		{/if}

		{#if failure}
			<div class="alert whitespace-pre-wrap"><WarningCircleIcon size={14} />{failure}</div>
		{/if}
	</div>
</div>
