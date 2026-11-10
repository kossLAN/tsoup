<script lang="ts">
	import type { AgentSummary } from '$lib/api';
	import { CheckIcon, RobotIcon, StopIcon, WarningCircleIcon } from '$lib/icons';
	import Spinner from './Spinner.svelte';

	let {
		root,
		busy,
		agents,
		onselect
	}: {
		root: string;
		busy: boolean;
		agents: AgentSummary[];
		onselect: (agent: AgentSummary) => void;
	} = $props();
</script>

{#snippet branch(parent: string, depth: number)}
	{#each agents.filter((agent) => agent.parent === parent) as agent (agent.id)}
		<button
			class="nav-item h-6.5 w-full shrink-0 gap-1.5 pr-1 text-left"
			style:padding-left="{4 + depth * 10}px"
			onclick={() => onselect(agent)}
		>
			<span class="status text-secondary">
				{#if agent.state === 'running'}
					<Spinner size={12} kind="stir" />
				{:else if agent.state === 'finished'}
					<CheckIcon size={12} />
				{:else if agent.state === 'failed'}
					<WarningCircleIcon size={12} />
				{:else}
					<StopIcon size={12} />
				{/if}
			</span>
			<span class={['truncate', agent.state !== 'running' && 'text-secondary']}>
				{agent.name}
			</span>
			{#if agent.model}
				<span class="ml-auto max-w-2/5 shrink-0 truncate pl-1 text-label text-secondary">
					{agent.model.name}
				</span>
			{/if}
		</button>
		{@render branch(agent.id, depth + 1)}
	{/each}
{/snippet}

<div class="flex flex-col">
	<div class="flex h-6.5 shrink-0 items-center gap-1.5 px-1">
		<span class="status text-secondary">
			{#if busy}
				<Spinner size={12} kind="stir" />
			{:else}
				<RobotIcon size={12} />
			{/if}
		</span>
		<span class="truncate">Main agent</span>
	</div>
	{@render branch(root, 1)}
</div>
