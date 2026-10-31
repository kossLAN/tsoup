<script lang="ts">
	import { goto } from '$app/navigation';
	import Composer from '$lib/components/Composer.svelte';
	import ModelNotice from '$lib/components/ModelNotice.svelte';
	import Wordmark from '$lib/components/Wordmark.svelte';
	import { WarningCircleIcon } from '$lib/icons';
	import { sessions } from '$lib/sessions.svelte';
	import { pick, prompts } from '$lib/soup';

	const prompt = pick(prompts);

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
	<div class="flex flex-col items-center gap-3 px-6 pb-10 text-center">
		<h1 class="text-7xl font-semibold tracking-tight"><Wordmark /></h1>
		<p class="text-title text-secondary">{prompt}</p>
	</div>
	{#if failure}
		<div class="mx-auto w-full max-w-chat px-6 pb-3">
			<div class="alert whitespace-pre-wrap"><WarningCircleIcon size={14} />{failure}</div>
		</div>
	{/if}
	<ModelNotice />
	<Composer onsend={send} />
</div>
