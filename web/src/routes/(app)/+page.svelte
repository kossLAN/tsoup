<script lang="ts">
	import { goto } from '$app/navigation';
	import { getSettings, type Image, type NewEnvironment, type Selection } from '$lib/api';
	import Composer from '$lib/components/Composer.svelte';
	import EnvironmentPicker from '$lib/components/EnvironmentPicker.svelte';
	import FolderPicker from '$lib/components/FolderPicker.svelte';
	import ModelNotice from '$lib/components/ModelNotice.svelte';
	import ModelPicker from '$lib/components/ModelPicker.svelte';
	import Wordmark from '$lib/components/Wordmark.svelte';
	import { WarningCircleIcon } from '$lib/icons';
	import { sessions } from '$lib/sessions.svelte';
	import { pick, prompts } from '$lib/soup';
	import { onMount } from 'svelte';

	const prompt = pick(prompts);

	let failure = $state<string | null>(null);
	let node = $state<string | null>(null);
	let folder = $state('');
	let selection = $state<Selection>({ model: null, effort: null });

	const connected = $derived(sessions.nodes.some(({ name }) => name === node) ? node : null);

	onMount(async () => {
		selection.effort = (await getSettings()).last_effort;
	});

	async function send(text: string, images: Image[]): Promise<boolean> {
		failure = null;
		if (connected !== null && !folder) {
			failure = 'Choose a folder for the session to work in';
			return false;
		}

		const environment: NewEnvironment =
			connected === null ? { kind: 'sandbox' } : { kind: 'node', name: connected, folder };
		try {
			const session = await sessions.create(environment, selection, text, images);
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
	<Composer onsend={send}>
		{#snippet accessory()}
			<ModelPicker {selection} onchange={(next) => (selection = next)} />
			{#if sessions.nodes.length}
				<EnvironmentPicker
					nodes={sessions.nodes}
					bind:value={
						() => connected,
						(value) => {
							node = value;
							folder = '';
						}
					}
				/>
				{#if connected !== null}
					<FolderPicker node={connected} bind:value={folder} />
				{/if}
			{/if}
		{/snippet}
	</Composer>
</div>
