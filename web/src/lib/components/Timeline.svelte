<script lang="ts">
	import type { Entry } from '$lib/api';
	import { CircleNotchIcon, WarningCircleIcon } from '$lib/icons';
	import ToolCallLine from './ToolCallLine.svelte';

	let { entries, busy, failure }: { entries: Entry[]; busy: boolean; failure: string | null } =
		$props();

	let scroller: HTMLElement;

	const results = $derived(
		new Map(
			entries.flatMap((entry) =>
				entry.kind === 'tool' ? [[entry.call_id, entry] as const] : []
			)
		)
	);

	$effect(() => {
		void entries.length;
		void busy;
		if (scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 200) {
			scroller.scrollTo({ top: scroller.scrollHeight });
		}
	});
</script>

<div class="scroller" bind:this={scroller}>
	<div class="timeline">
		{#each entries as entry, index (index)}
			{#if entry.kind === 'user'}
				<div class="user">{entry.text}</div>
			{:else if entry.kind === 'assistant'}
				{#if entry.text}
					<div class="assistant">{entry.text}</div>
				{/if}
				{#if entry.tool_calls.length}
					<div class="calls">
						{#each entry.tool_calls as call (call.id)}
							<ToolCallLine {call} result={results.get(call.id)} pending={busy} />
						{/each}
					</div>
				{/if}
			{/if}
		{/each}

		{#if busy}
			<div class="working">
				<span class="spinner"><CircleNotchIcon size={14} /></span>Working
			</div>
		{/if}

		{#if failure}
			<div class="failure"><WarningCircleIcon size={14} />{failure}</div>
		{/if}
	</div>
</div>

<style>
	.scroller {
		flex: 1;
		overflow-y: auto;
	}

	.timeline {
		display: flex;
		flex-direction: column;
		gap: 10px;
		max-width: 820px;
		margin: 0 auto;
		padding: 24px 24px 12px;
	}

	.user,
	.assistant {
		font-size: var(--font-title);
		line-height: 1.6;
		white-space: pre-wrap;
		word-break: break-word;
	}

	.user {
		align-self: flex-end;
		max-width: 80%;
		padding: 8px 12px;
		background: var(--alternate-base);
		border: 1px solid var(--light);
		border-radius: var(--radius-surface);
	}

	.calls {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.working,
	.failure {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--text-secondary);
	}

	.failure {
		padding: 8px 12px;
		color: var(--danger);
		background: var(--danger-tint);
		border-radius: var(--radius-control);
		white-space: pre-wrap;
	}

	.spinner {
		display: inline-flex;
		color: var(--accent);
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
