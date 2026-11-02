<script lang="ts">
	import { getSettings } from '$lib/api';
	import { onMount } from 'svelte';

	let missing = $state(false);

	onMount(async () => {
		missing = !(await getSettings()).providers.length;
	});
</script>

{#if missing}
	<div class="mx-auto w-full max-w-chat px-7 pb-2 text-secondary">
		No model provider is configured. <a class="text-accent underline" href="/settings"
			>Add one in settings</a
		>
	</div>
{/if}
