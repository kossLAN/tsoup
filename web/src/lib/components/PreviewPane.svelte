<script lang="ts">
	import { previewUrl, type Action, type Preview, type VmStatus } from '$lib/api';
	import {
		ArrowClockwiseIcon,
		ArrowSquareOutIcon,
		PlayIcon,
		PowerIcon,
		WarningCircleIcon,
		XIcon
	} from '$lib/icons';
	import Spinner from './Spinner.svelte';

	let {
		session,
		status,
		boots,
		actions,
		onaction,
		onhide,
		ondestroy
	}: {
		session: string;
		status: VmStatus;
		boots: number;
		actions: Action[];
		onaction: (label: string) => void;
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

	const previews = $derived(status.state === 'running' ? status.previews : []);

	let view = $derived(previews[0]?.name);
	const current = $derived(previews.find((preview) => preview.name === view));
	const reloads = $state<Record<string, number>>({});

	function reload(preview: Preview) {
		reloads[preview.name] = (reloads[preview.name] ?? 0) + 1;
	}
</script>

<section class="flex h-full flex-col border-l border-light bg-window">
	<header class="toolbar pr-2.25 pl-3.5">
		<span class="text-title font-medium">VM</span>
		<span class="flex-1 text-label text-secondary">{label[status.state]}</span>
		{#each actions as action (action.label)}
			<button
				class="button gap-1.5 px-2.5 text-label enabled:hover:bg-window-text/8"
				title={action.command}
				disabled={status.state !== 'running'}
				onclick={() => onaction(action.label)}
			>
				<PlayIcon size={12} />
				{action.label}
			</button>
		{/each}
		{#if current}
			<button class="icon-button" title="Reload" onclick={() => reload(current)}>
				<ArrowClockwiseIcon size={14} />
			</button>
			<a
				class="icon-button"
				title="Open in a new tab"
				href={previewUrl(session, current)}
				target="_blank"
			>
				<ArrowSquareOutIcon size={14} />
			</a>
		{/if}
		{#if previews.length > 1}
			<div class="flex gap-0.5 rounded-control bg-base p-0.5" role="tablist">
				{#each previews as preview (preview.name)}
					<button
						class={[
							'h-5.5 rounded-input px-2 text-label',
							view === preview.name ? 'bg-alternate-base' : 'text-secondary'
						]}
						role="tab"
						aria-selected={view === preview.name}
						onclick={() => (view = preview.name)}
					>
						{preview.label}
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
			{#each previews as preview (preview.name)}
				{#key `${boots} ${preview.target} ${reloads[preview.name]}`}
					<iframe
						class={['flex-1', view !== preview.name && 'hidden']}
						src={previewUrl(session, preview)}
						title={preview.label}
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
