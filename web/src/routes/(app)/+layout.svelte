<script lang="ts">
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { sessions } from '$lib/sessions.svelte';
	import { onMount } from 'svelte';

	let { children } = $props();

	let open = $state(false);

	onMount(() => sessions.connect());
</script>

<Sidebar bind:open />
<main class="flex h-screen">
	{@render children()}
</main>
