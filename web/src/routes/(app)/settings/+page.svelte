<script lang="ts">
	import { getSettings, listModels, saveSettings, type Settings } from '$lib/api';
	import { onMount } from 'svelte';

	let saved = $state<Settings | null>(null);
	let url = $state('');
	let key = $state('');
	let model = $state('');
	let models = $state<string[]>([]);
	let modelsFailure = $state<string | null>(null);
	let status = $state<string | null>(null);
	let saving = $state(false);

	onMount(async () => {
		saved = await getSettings();
		url = saved.llm_url;
		model = saved.model ?? '';
		await refreshModels();
	});

	async function refreshModels() {
		modelsFailure = null;
		try {
			models = await listModels();
		} catch (error) {
			models = [];
			modelsFailure = error instanceof Error ? error.message : String(error);
		}
	}

	async function save(llmKey?: string) {
		saving = true;
		status = null;
		try {
			saved = await saveSettings({ llm_url: url, llm_key: llmKey, model: model || null });
			key = '';
			status = 'Saved';
			await refreshModels();
		} catch (error) {
			status = error instanceof Error ? error.message : String(error);
		} finally {
			saving = false;
		}
	}

	function submit(event: SubmitEvent) {
		event.preventDefault();
		save(key || undefined);
	}
</script>

<div class="flex min-w-0 flex-1 flex-col">
	<header class="toolbar pr-6 pl-11 text-title font-medium">Settings</header>

	{#if saved}
		<form class="mx-auto w-full max-w-160 p-6" onsubmit={submit}>
			<div class="px-1 pb-2 label">Model provider</div>
			<div class="rounded-surface surface p-1">
				<label class="field">
					<span class="w-24 shrink-0">Endpoint</span>
					<input
						class="input"
						bind:value={url}
						placeholder="https://api.openai.com/v1"
						spellcheck="false"
					/>
				</label>
				<label class="field">
					<span class="w-24 shrink-0">API key</span>
					<input
						class="input"
						bind:value={key}
						type="password"
						autocomplete="off"
						placeholder={saved.has_llm_key
							? 'Saved, leave empty to keep it'
							: 'Not set'}
					/>
					{#if saved.has_llm_key}
						<button
							type="button"
							class="button enabled:hover:bg-window-text/8"
							disabled={saving}
							onclick={() => save('')}
						>
							Remove
						</button>
					{/if}
				</label>
				<label class="field">
					<span class="w-24 shrink-0">Model</span>
					<input
						class="input"
						bind:value={model}
						list="models"
						placeholder="Choose a model"
						spellcheck="false"
					/>
					<datalist id="models">
						{#each models as option (option)}
							<option value={option}></option>
						{/each}
					</datalist>
				</label>
			</div>

			{#if modelsFailure}
				<p class="mx-1 mt-2 text-label text-danger">
					Could not list models: {modelsFailure}
				</p>
			{:else if models.length}
				<p class="mx-1 mt-2 text-label text-secondary">
					{models.length} models available from this endpoint
				</p>
			{/if}

			<div class="mt-4 flex items-center justify-end gap-3">
				{#if status}
					<span class="text-secondary">{status}</span>
				{/if}
				<button
					type="submit"
					class="button bg-window-text font-medium text-window enabled:hover:bg-window-text/90"
					disabled={saving}
				>
					Save
				</button>
			</div>
		</form>
	{/if}
</div>
