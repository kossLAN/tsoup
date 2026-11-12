<script lang="ts">
	import type { AgentSummary, Entry, UserEntry } from '$lib/api';
	import { WarningCircleIcon } from '$lib/icons';
	import type { Draft } from '$lib/session.svelte';
	import { tick } from 'svelte';
	import CompactionLine from './CompactionLine.svelte';
	import Markdown from './Markdown.svelte';
	import ReplyActions from './ReplyActions.svelte';
	import ReportLine from './ReportLine.svelte';
	import Spinner from './Spinner.svelte';
	import ToolCallLine from './ToolCallLine.svelte';
	import Working from './Working.svelte';

	let {
		entries,
		start,
		busy,
		draft,
		queued,
		agents,
		compacting = false,
		thinking,
		failure,
		onolder,
		onregenerate,
		dense = false
	}: {
		entries: Entry[];
		start: number;
		busy: boolean;
		draft: Draft | null;
		queued: UserEntry[];
		agents: AgentSummary[];
		compacting?: boolean;
		thinking: boolean;
		failure: string | null;
		onolder: () => Promise<void>;
		onregenerate?: () => void;
		dense?: boolean;
	} = $props();

	const awaiting = $derived.by(() => {
		if (!agents.some(({ state }) => state === 'running')) return false;
		const last = entries.at(-1);
		if (last?.kind === 'assistant' && !last.tool_calls.length) return true;
		const reply = entries.findLast((entry) => entry.kind === 'assistant');
		return (
			reply?.kind === 'assistant' &&
			reply.tool_calls.some(
				(call) => call.function.name === 'wait_agents' && !results.has(call.id)
			)
		);
	});

	const activity = $derived.by(() => {
		if (compacting) return 'Compacting';
		if (!draft) return awaiting ? 'Waiting for agents' : null;
		if (draft.tool) return `Preparing ${draft.tool}`;
		if (draft.text) return 'Writing';
		return 'Thinking';
	});

	const threshold = 400;

	let scroller: HTMLElement;
	let content: HTMLElement;
	let pinned = true;
	let anchor: number | null = null;
	let end = 0;
	let waiting = 0;
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
		const observer = new ResizeObserver(follow);
		observer.observe(scroller);
		observer.observe(content);
		return () => observer.disconnect();
	});

	$effect(() => {
		const total = start + entries.length;
		if ((total > end && entries.at(-1)?.kind === 'user') || queued.length > waiting) {
			pinned = true;
		}
		end = total;
		waiting = queued.length;

		if (pinned) {
			follow();
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

	function follow() {
		if (pinned) scroller.scrollTop = scroller.scrollHeight;
	}

	function scrolled() {
		pinned = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40;
		older();
	}
</script>

{#snippet message(entry: UserEntry)}
	{#if entry.images?.length}
		<div class="flex max-w-4/5 flex-wrap justify-end gap-1.5 self-end">
			{#each entry.images as image, position (position)}
				<img
					class="max-h-56 max-w-full rounded-control border border-light"
					src="data:{image.media_type};base64,{image.data}"
					alt="Attachment {position + 1}"
				/>
			{/each}
		</div>
	{/if}
	{#if entry.text}
		<div
			class="max-w-4/5 self-end bubble text-title/relaxed wrap-break-word whitespace-pre-wrap"
		>
			{entry.text}
		</div>
	{/if}
{/snippet}

<div class="flex-1 overflow-y-auto" bind:this={scroller} onscroll={scrolled}>
	<div
		class={['mx-auto flex max-w-chat flex-col gap-2.5 pb-3', dense ? 'px-2 pt-2' : 'px-6 pt-6']}
		bind:this={content}
	>
		{#if start > 0}
			<div class="flex justify-center"><Spinner size={14} kind="simmer" /></div>
		{/if}

		{#each entries as entry, index (start + index)}
			{#if entry.kind === 'user'}
				{@render message(entry)}
			{:else if entry.kind === 'assistant'}
				{#if thinking && entry.thinking}
					<Markdown text={entry.thinking} muted />
				{/if}
				{#if entry.text}
					<Markdown text={entry.text} />
				{/if}
				{#if entry.tool_calls.length}
					<div class="flex flex-col gap-0.5">
						{#each entry.tool_calls as call (call.id)}
							<ToolCallLine
								{call}
								result={results.get(call.id)}
								pending={busy}
								{agents}
							/>
						{/each}
					</div>
				{/if}
				{#if entry.text && !entry.tool_calls.length}
					<ReplyActions
						text={entry.text}
						onregenerate={!busy && index === entries.length - 1
							? onregenerate
							: undefined}
					/>
				{/if}
			{:else if entry.kind === 'compaction'}
				<CompactionLine summary={entry.summary} />
			{:else if entry.kind === 'report'}
				<ReportLine report={entry} {agents} />
			{/if}
		{/each}

		{#if thinking && draft?.thinking}
			<Markdown text={draft.thinking} muted />
		{/if}
		{#if draft?.text}
			<Markdown text={draft.text} />
		{/if}

		{#if busy}
			<Working label={activity} />
		{/if}

		{#if queued.length}
			<div class="flex flex-col gap-2.5 opacity-60">
				<span class="self-end label">Queued</span>
				{#each queued as entry, index (index)}
					{@render message(entry)}
				{/each}
			</div>
		{/if}

		{#if failure}
			<div class="alert whitespace-pre-wrap"><WarningCircleIcon size={14} />{failure}</div>
		{/if}
	</div>
</div>
