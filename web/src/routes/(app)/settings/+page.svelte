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

<div class="page">
	<header>Settings</header>

	{#if saved}
		<form onsubmit={submit}>
			<div class="label section">Model provider</div>
			<div class="card">
				<label class="row">
					<span>Endpoint</span>
					<input
						bind:value={url}
						placeholder="https://api.openai.com/v1"
						spellcheck="false"
					/>
				</label>
				<div class="separator"></div>
				<label class="row">
					<span>API key</span>
					<input
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
							class="remove"
							disabled={saving}
							onclick={() => save('')}
						>
							Remove
						</button>
					{/if}
				</label>
				<div class="separator"></div>
				<label class="row">
					<span>Model</span>
					<input
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
				<p class="hint failure">Could not list models: {modelsFailure}</p>
			{:else if models.length}
				<p class="hint">{models.length} models available from this endpoint</p>
			{/if}

			<div class="actions">
				{#if status}
					<span class="status">{status}</span>
				{/if}
				<button type="submit" class="save" disabled={saving}>Save</button>
			</div>
		</form>
	{/if}
</div>

<style>
	.page {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	header {
		display: flex;
		align-items: center;
		height: 44px;
		padding: 0 24px;
		flex-shrink: 0;
		border-bottom: 1px solid var(--light);
		font-size: var(--font-title);
		font-weight: 500;
	}

	form {
		width: 100%;
		max-width: 640px;
		margin: 0 auto;
		padding: 24px;
	}

	.section {
		padding: 0 4px 8px;
	}

	.card {
		padding: 4px;
		background: var(--base);
		border: 1px solid var(--light);
		border-radius: var(--radius-surface);
	}

	.row {
		display: flex;
		align-items: center;
		gap: 10px;
		min-height: 38px;
		padding: 0 6px;
	}

	.row span {
		width: 96px;
		flex-shrink: 0;
	}

	.separator {
		height: 1px;
		margin: 0 6px;
		background: var(--light);
	}

	input {
		flex: 1;
		min-width: 0;
		height: 28px;
		padding: 0 8px;
		background: var(--alternate-base);
		border: 1px solid var(--light);
		border-radius: var(--radius-input);
		outline: none;
		transition: border-color var(--duration) var(--ease-out-cubic);
	}

	input:focus {
		border-color: var(--accent);
	}

	input::placeholder {
		color: var(--text-secondary);
	}

	.remove {
		height: 28px;
		padding: 0 10px;
		color: var(--danger);
	}

	.remove:hover {
		background: var(--danger-tint);
	}

	.hint {
		margin: 8px 4px 0;
		font-size: var(--font-label);
		color: var(--text-secondary);
	}

	.failure {
		color: var(--danger);
	}

	.actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 12px;
		margin-top: 16px;
	}

	.status {
		color: var(--text-secondary);
	}

	.save {
		height: 28px;
		padding: 0 16px;
		background: var(--accent);
		color: var(--highlighted-text);
		font-weight: 500;
	}

	.save:hover {
		background: var(--accent);
		filter: brightness(1.1);
	}
</style>
