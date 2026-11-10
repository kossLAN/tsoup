<script lang="ts">
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { sessions } from '$lib/sessions.svelte';
	import { onMount } from 'svelte';

	const key = 'tsoup.sidebar';

	let { children } = $props();

	let open = $state(localStorage.getItem(key) === 'open');

	$effect(() => localStorage.setItem(key, open ? 'open' : 'closed'));

	onMount(() => sessions.connect());
</script>

<div class="group/shell flex h-screen" data-sidebar={open}>
	<Sidebar bind:open />
	<main class="flex min-w-0 flex-1">
		{@render children()}
	</main>
</div>
