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

{#snippet branch(parent: string, trail: boolean[])}
	{@const children = agents.filter((agent) => agent.parent === parent)}
	{#each children as agent, index (agent.id)}
		{@const last = index === children.length - 1}
		<button
			class="nav-item h-6.5 w-full shrink-0 gap-1.5 px-1 text-left"
			onclick={() => onselect(agent)}
		>
			{#each trail as open, level (level)}
				<span
					class={[
						'ml-1.75 w-1.75 shrink-0 self-stretch',
						open && 'border-l border-light'
					]}
				></span>
			{/each}
			<span
				class={['ml-1.75 w-1.75 shrink-0 self-stretch', !last && 'border-l border-light']}
			>
				<span
					class={[
						'block h-1/2 border-b border-light',
						last && 'rounded-bl-small border-l'
					]}
				></span>
			</span>
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
		{@render branch(agent.id, [...trail, !last])}
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
	{@render branch(root, [])}
</div>
