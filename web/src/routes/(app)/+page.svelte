<script lang="ts">
	import { goto } from '$app/navigation';
	import { sessions } from '$lib/sessions.svelte';

	async function create() {
		const session = await sessions.create();
		await goto(`/sessions/${session.id}`);
	}
</script>

<div class="empty">
	<button onclick={create}>New session</button>
</div>

<style>
	.empty {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	button {
		height: 32px;
		padding: 0 14px;
		background: var(--button);
		border: 1px solid var(--light);
		font-size: var(--font-title);
	}
</style>
