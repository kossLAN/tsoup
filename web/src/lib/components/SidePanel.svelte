<script lang="ts">
	import { stopAgent } from '$lib/api';
	import { ArrowLeftIcon, StopIcon, XIcon } from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import type { Session } from '$lib/session.svelte';
	import AgentTree from './AgentTree.svelte';
	import AgentView from './AgentView.svelte';
	import QuestionCard from './QuestionCard.svelte';

	let { session, thinking, onhide }: { session: Session; thinking: boolean; onhide: () => void } =
		$props();

	let selected = $state<string | null>(null);

	const agent = $derived(session.agents.find((agent) => agent.id === selected));

	function asker(id: string): string {
		return session.agents.find((agent) => agent.id === id)?.name ?? 'Main agent';
	}
</script>

<section class="flex h-full flex-col border-l border-light bg-window">
	<header class="toolbar px-1.5">
		{#if agent}
			<button
				class="icon-button"
				onclick={() => (selected = null)}
				{@attach tooltip('Back', 'bottom-start')}
			>
				<ArrowLeftIcon size={14} />
			</button>
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
		{:else}
			<span class="min-w-0 flex-1 truncate pl-1 font-medium">Agents</span>
		{/if}
		<button class="icon-button" onclick={onhide} {@attach tooltip('Hide', 'bottom-end')}>
			<XIcon size={14} />
		</button>
	</header>

	{#if agent}
		<AgentView {session} {agent} {thinking} />
	{:else}
		<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-1 pb-2">
			{#if session.questions.length}
				<div class="flex flex-col gap-1">
					<h3 class="px-1 label">Questions</h3>
					{#each session.questions as question (question.id)}
						<QuestionCard
							session={session.id}
							{question}
							asker={asker(question.agent)}
						/>
					{/each}
				</div>
			{/if}

			{#if session.agents.length}
				<AgentTree
					root={session.id}
					busy={session.busy}
					agents={session.agents}
					onselect={(agent) => (selected = agent.id)}
				/>
			{/if}
		</div>
	{/if}
</section>
