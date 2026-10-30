<script lang="ts">
	import { goto } from '$app/navigation';
	import Composer from '$lib/components/Composer.svelte';
	import ModelNotice from '$lib/components/ModelNotice.svelte';
	import { WarningCircleIcon } from '$lib/icons';
	import { sessions } from '$lib/sessions.svelte';

	let failure = $state<string | null>(null);

	async function send(text: string): Promise<boolean> {
		failure = null;
		try {
			const session = await sessions.create();
			sessions.pending.set(session.id, text);
			await goto(`/sessions/${session.id}`);
			return true;
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
			return false;
		}
	}
</script>

<div class="flex flex-1 flex-col justify-center pb-20">
	{#if failure}
		<div class="mx-auto w-full max-w-chat px-6 pb-3">
			<div class="alert whitespace-pre-wrap"><WarningCircleIcon size={14} />{failure}</div>
		</div>
	{/if}
	<ModelNotice />
	<Composer onsend={send} />
</div>
