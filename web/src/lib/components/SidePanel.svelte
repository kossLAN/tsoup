<script lang="ts">
	import { stopAgent } from '$lib/api';
	import { ArrowLeftIcon, QuestionIcon, StopIcon, XIcon } from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import type { Session } from '$lib/session.svelte';
	import AgentTree from './AgentTree.svelte';
	import AgentView from './AgentView.svelte';
	import QuestionView from './QuestionView.svelte';

	type View = { kind: 'agent' | 'question'; id: string };

	let { session, thinking, onhide }: { session: Session; thinking: boolean; onhide: () => void } =
		$props();

	let view = $state<View | null>(null);
	let known = 0;

	const agent = $derived(
		view?.kind === 'agent' ? session.agents.find((agent) => agent.id === view?.id) : undefined
	);
	const position = $derived(
		view?.kind === 'question'
			? session.questions.findIndex((question) => question.id === view?.id)
			: -1
	);
	const question = $derived(session.questions[position]);

	$effect(() => {
		const count = session.questions.length;
		if (count > known && view === null) {
			view = { kind: 'question', id: session.questions[known].id };
		}
		known = count;
	});

	function asker(id: string): string {
		return session.agents.find((agent) => agent.id === id)?.name ?? 'Main agent';
	}

	function advance() {
		const next = session.questions[position + 1] ?? session.questions[position - 1];
		view = next ? { kind: 'question', id: next.id } : null;
	}
</script>

<section class="flex h-full flex-col border-l border-light bg-window">
	<header class="toolbar px-1.5">
		{#if agent || question}
			<button
				class="icon-button"
				onclick={() => (view = null)}
				{@attach tooltip('Back', 'bottom-start')}
			>
				<ArrowLeftIcon size={14} />
			</button>
		{/if}
		{#if agent}
			<span class="min-w-0 flex-1 truncate font-medium">{agent.name}</span>
			{#if agent.state === 'running'}
				<button
					class="icon-button"
					onclick={() => stopAgent(session.id, agent.id)}
					{@attach tooltip('Stop agent')}
				>
					<StopIcon size={14} />
				</button>
			{/if}
		{:else if question}
			<div class="flex min-w-0 flex-1 overflow-x-auto" role="tablist">
				<div class="flex gap-0.5 rounded-control bg-base p-0.5">
					{#each session.questions as other, index (other.id)}
						<button
							class={[
								'h-5.5 min-w-5.5 shrink-0 rounded-input px-1.5 text-label transition-colors',
								other.id === question.id
									? 'bg-alternate-base'
									: 'text-secondary hover:text-window-text'
							]}
							role="tab"
							aria-selected={other.id === question.id}
							onclick={() => (view = { kind: 'question', id: other.id })}
						>
							{index + 1}
						</button>
					{/each}
				</div>
			</div>
		{:else}
			<span class="min-w-0 flex-1 truncate pl-1 font-medium">Activity</span>
		{/if}
		<button class="icon-button" onclick={onhide} {@attach tooltip('Hide', 'bottom-end')}>
			<XIcon size={14} />
		</button>
	</header>

	{#if agent}
		<AgentView {session} {agent} {thinking} />
	{:else if question}
		{#key question.id}
			<QuestionView
				session={session.id}
				{question}
				asker={asker(question.agent)}
				onanswered={advance}
			/>
		{/key}
	{:else}
		<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-1 pb-2">
			{#if session.questions.length}
				<div class="flex flex-col">
					<h3 class="px-1 pb-1 label">Questions</h3>
					{#each session.questions as pending (pending.id)}
						<button
							class="nav-item h-6.5 w-full shrink-0 gap-1.5 px-1 text-left"
							onclick={() => (view = { kind: 'question', id: pending.id })}
						>
							<span class="status text-accent"><QuestionIcon size={12} /></span>
							<span class="truncate">{pending.text}</span>
						</button>
					{/each}
				</div>
			{/if}

			{#if session.agents.length}
				<div class="flex flex-col">
					{#if session.questions.length}
						<h3 class="px-1 pb-1 label">Agents</h3>
					{/if}
					<AgentTree
						root={session.id}
						busy={session.busy}
						agents={session.agents}
						onselect={(agent) => (view = { kind: 'agent', id: agent.id })}
					/>
				</div>
			{/if}
		</div>
	{/if}
</section>
