<script lang="ts">
	import { pick, verbs } from '$lib/soup';
	import { fade } from 'svelte/transition';
	import Spinner from './Spinner.svelte';

	let verb = $state(pick(verbs));

	$effect(() => {
		const timer = setInterval(() => (verb = pick(verbs, verb)), 6000);
		return () => clearInterval(timer);
	});
</script>

<div class="flex items-center gap-2 text-secondary">
	<Spinner size={14} />
	{#key verb}
		<span in:fade={{ duration: 300 }}>{verb}…</span>
	{/key}
</div>
