<script lang="ts">
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { sessions } from '$lib/sessions.svelte';
	import { onMount } from 'svelte';

	let { children } = $props();

	const storageKey = 'tsoup.sidebar.collapsed';
	let collapsed = $state(localStorage.getItem(storageKey) === 'true');

	$effect(() => {
		localStorage.setItem(storageKey, String(collapsed));
	});

	onMount(() => {
		sessions.refresh();
	});
</script>

<div class="app">
	<Sidebar bind:collapsed />
	<main>
		{@render children()}
	</main>
</div>

<style>
	.app {
		display: flex;
		height: 100vh;
	}

	main {
		flex: 1;
		min-width: 0;
		display: flex;
	}
</style>
