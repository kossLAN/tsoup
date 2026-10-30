<script lang="ts">
	import { page } from '$app/state';
	import { vncUrl } from '$lib/api';
	import RFB from '@novnc/novnc';

	let screen: HTMLElement;
	let connected = $state(false);

	$effect(() => {
		const rfb = new RFB(screen, vncUrl(page.params.id ?? '', page.params.surface ?? ''));
		rfb.scaleViewport = true;
		rfb.resizeSession = true;
		rfb.background = '#000';
		rfb.addEventListener('connect', () => (connected = true));
		rfb.addEventListener('disconnect', () => (connected = false));
		return () => rfb.disconnect();
	});
</script>

<div class="fixed inset-0 bg-black" bind:this={screen}></div>
{#if !connected}
	<div class="pointer-events-none fixed inset-0 flex items-center justify-center text-secondary">
		Connecting to the VM display
	</div>
{/if}
