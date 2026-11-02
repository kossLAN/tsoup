<script lang="ts">
	import type { NodeSummary } from '$lib/api';
	import { CaretDownIcon, CubeIcon, DesktopTowerIcon } from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import Menu from './Menu.svelte';
	import MenuItem from './MenuItem.svelte';

	let { nodes, value = $bindable() }: { nodes: NodeSummary[]; value: string | null } = $props();
</script>

<Menu placement="top-end">
	{#snippet trigger(attributes)}
		<button
			class="button h-6.5 shrink-0 gap-1.5 px-2 text-label text-secondary hover:bg-window-text/8 hover:text-window-text"
			{...attributes}
			{@attach tooltip('Where the session runs')}
		>
			{#if value === null}
				<CubeIcon size={14} />
				Sandbox
			{:else}
				<DesktopTowerIcon size={14} />
				{value}
			{/if}
			<CaretDownIcon size={10} />
		</button>
	{/snippet}
	<MenuItem onselect={() => (value = null)}>
		<CubeIcon size={14} />
		Sandbox
	</MenuItem>
	{#each nodes as node (node.name)}
		<MenuItem onselect={() => (value = node.name)}>
			<DesktopTowerIcon size={14} />
			{node.name}
			<span class="ml-auto max-w-60 truncate pl-3 font-mono text-label text-secondary"
				>{node.directory}</span
			>
		</MenuItem>
	{/each}
</Menu>
