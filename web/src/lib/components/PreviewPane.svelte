<script lang="ts">
	import { surfaceUrl, type Surface, type VmStatus } from '$lib/api';
	import {
		ArrowClockwiseIcon,
		ArrowSquareOutIcon,
		PowerIcon,
		WarningCircleIcon,
		XIcon
	} from '$lib/icons';
	import Spinner from './Spinner.svelte';

	let {
		session,
		status,
		boots,
		onhide,
		ondestroy
	}: {
		session: string;
		status: VmStatus;
		boots: number;
		onhide: () => void;
		ondestroy: () => void;
	} = $props();

	const label = {
		stopped: 'Stopped',
		building: 'Building image',
		booting: 'Booting',
		running: 'Running',
		failed: 'Failed'
	};

	const surfaces = $derived(status.state === 'running' ? status.surfaces : []);

	let view = $derived(
		(
			surfaces.find((surface) => surface.target !== null) ??
			surfaces.find((surface) => surface.protocol === 'vnc') ??
			surfaces[0]
		)?.name
	);
	const current = $derived(surfaces.find((surface) => surface.name === view));
	const reloads = $state<Record<string, number>>({});

	function source(surface: Surface): string {
		return surface.protocol === 'vnc'
			? `/vnc/${session}/${surface.name}`
			: surfaceUrl(session, surface.name);
	}

	function reload(surface: Surface) {
		reloads[surface.name] = (reloads[surface.name] ?? 0) + 1;
	}
</script>

<section class="flex h-full flex-col border-l border-light bg-window">
	<header class="toolbar pr-2.25 pl-3.5">
		<span class="text-title font-medium">VM</span>
		<span class="flex-1 text-label text-secondary">{label[status.state]}</span>
		{#if current?.protocol === 'http'}
			<button class="icon-button" title="Reload" onclick={() => reload(current)}>
				<ArrowClockwiseIcon size={14} />
			</button>
			<a
				class="icon-button"
				title="Open in a new tab"
				href={surfaceUrl(session, current.name)}
				target="_blank"
			>
				<ArrowSquareOutIcon size={14} />
			</a>
		{/if}
		{#if surfaces.length > 1}
			<div class="flex gap-0.5 rounded-control bg-base p-0.5" role="tablist">
				{#each surfaces as surface (surface.name)}
					<button
						class={[
							'h-5.5 rounded-input px-2 text-label',
							view === surface.name ? 'bg-alternate-base' : 'text-secondary'
						]}
						role="tab"
						aria-selected={view === surface.name}
						onclick={() => (view = surface.name)}
					>
						{surface.label}
					</button>
				{/each}
			</div>
		{/if}
		<button
			class="icon-button"
			title="Destroy VM"
			disabled={status.state === 'stopped'}
			onclick={ondestroy}
		>
			<PowerIcon size={14} />
		</button>
		<button class="icon-button" title="Hide" onclick={onhide}>
			<XIcon size={14} />
		</button>
	</header>

	<div class="flex min-h-0 flex-1 bg-black">
		{#if status.state === 'running'}
			{#each surfaces as surface (surface.name)}
				{#key `${boots} ${surface.target} ${reloads[surface.name]}`}
					<iframe
						class={['flex-1', view !== surface.name && 'hidden']}
						src={source(surface)}
						title={surface.label}
						allow="clipboard-read; clipboard-write"
					></iframe>
				{/key}
			{/each}
		{:else if status.state === 'failed'}
			<div class="m-auto flex max-w-9/10 items-start gap-2.5">
				<WarningCircleIcon size={18} />
				<pre
					class="max-h-[60vh] overflow-y-auto text-label wrap-break-word whitespace-pre-wrap">{status.message}</pre>
			</div>
		{:else if status.state === 'stopped'}
			<div class="m-auto text-secondary">The VM is not running</div>
		{:else}
			<div class="m-auto flex items-center gap-2.5 text-secondary">
				<Spinner size={18} />
				{label[status.state]}
			</div>
		{/if}
	</div>
</section>
