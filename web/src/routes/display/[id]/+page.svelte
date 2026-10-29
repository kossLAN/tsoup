<script lang="ts">
	import { page } from '$app/state';
	import { displayUrl } from '$lib/api';
	import RFB from '@novnc/novnc';

	let screen: HTMLElement;
	let connected = $state(false);

	$effect(() => {
		const rfb = new RFB(screen, displayUrl(page.params.id ?? ''));
		rfb.scaleViewport = true;
		rfb.background = '#000';
		rfb.addEventListener('connect', () => (connected = true));
		rfb.addEventListener('disconnect', () => (connected = false));
		return () => rfb.disconnect();
	});
</script>

<div class="screen" bind:this={screen}></div>
{#if !connected}
	<div class="overlay">Connecting to the VM display</div>
{/if}

<style>
	:global(body) {
		background: #000;
		overflow: hidden;
	}

	.screen {
		position: fixed;
		inset: 0;
	}

	.overlay {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-secondary);
		pointer-events: none;
	}
</style>
