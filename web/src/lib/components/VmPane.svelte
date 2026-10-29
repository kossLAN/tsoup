<script lang="ts">
	import type { VmStatus } from '$lib/api';
	import { CircleNotchIcon, PowerIcon, WarningCircleIcon, XIcon } from '$lib/icons';

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
</script>

<section>
	<header>
		<span class="title">VM</span>
		<span class="state {status.state}">{label[status.state]}</span>
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
			{#key boots}
				<iframe src="/display/{session}" title="VM display"></iframe>
			{/key}
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
