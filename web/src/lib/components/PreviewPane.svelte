<script lang="ts">
	import { surfaceUrl, type Surface, type VmStatus } from '$lib/api';
	import {
		ArrowClockwiseIcon,
		ArrowSquareOutIcon,
		CircleNotchIcon,
		PowerIcon,
		WarningCircleIcon,
		XIcon
	} from '$lib/icons';

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

<section>
	<header>
		<span class="title">VM</span>
		<span class="state {status.state}">{label[status.state]}</span>
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
			<div class="views" role="tablist">
				{#each surfaces as surface (surface.name)}
					<button
						role="tab"
						aria-selected={view === surface.name}
						class:selected={view === surface.name}
						onclick={() => (view = surface.name)}
					>
						{surface.label}
					</button>
				{/each}
			</div>
		{/if}
		<button
			class="icon-button destroy"
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

	<div class="screen">
		{#if status.state === 'running'}
			{#each surfaces as surface (surface.name)}
				{#key `${boots} ${surface.target} ${reloads[surface.name]}`}
					<iframe
						class:hidden={view !== surface.name}
						src={source(surface)}
						title={surface.label}
						allow="clipboard-read; clipboard-write"
					></iframe>
				{/key}
			{/each}
		{:else if status.state === 'failed'}
			<div class="message failure">
				<WarningCircleIcon size={18} />
				<pre>{status.message}</pre>
			</div>
		{:else if status.state === 'stopped'}
			<div class="message">The VM is not running</div>
		{:else}
			<div class="message">
				<span class="spinner"><CircleNotchIcon size={18} /></span>
				{label[status.state]}
			</div>
		{/if}
	</div>
</section>

<style>
	section {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--window);
		border-left: 1px solid var(--light);
	}

	header {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 44px;
		padding: 0 9px 0 14px;
		flex-shrink: 0;
		border-bottom: 1px solid var(--light);
	}

	.title {
		font-size: var(--font-title);
		font-weight: 500;
	}

	.state {
		flex: 1;
		font-size: var(--font-label);
		color: var(--text-secondary);
	}

	.state.running {
		color: var(--positive);
	}

	.state.failed {
		color: var(--danger);
	}

	.destroy:hover {
		color: var(--danger);
		background: var(--danger-tint);
	}

	.screen {
		flex: 1;
		min-height: 0;
		display: flex;
		background: #000;
	}

	iframe {
		flex: 1;
		border: none;
	}

	iframe.hidden {
		display: none;
	}

	a.icon-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		color: inherit;
		transition: background var(--duration-fast) linear;
	}

	a.icon-button:hover {
		background: var(--hover);
	}

	.views {
		display: flex;
		padding: 2px;
		gap: 2px;
		border-radius: var(--radius-control);
		background: var(--base);
	}

	.views button {
		height: 22px;
		padding: 0 8px;
		border-radius: var(--radius-input);
		font-size: var(--font-label);
		color: var(--text-secondary);
	}

	.views button.selected {
		background: var(--alternate-base);
		color: var(--window-text);
	}

	.message {
		margin: auto;
		display: flex;
		align-items: center;
		gap: 10px;
		max-width: 90%;
		color: var(--text-secondary);
	}

	.failure {
		align-items: flex-start;
		color: var(--danger);
	}

	.failure pre {
		margin: 0;
		font-size: var(--font-label);
		white-space: pre-wrap;
		word-break: break-word;
		max-height: 60vh;
		overflow-y: auto;
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
