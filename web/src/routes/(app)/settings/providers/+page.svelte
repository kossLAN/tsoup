<script lang="ts">
	import {
		getClaudeCodeAccount,
		getSettings,
		patchSettings,
		type ClaudeCodeAccount,
		type Provider,
		type ProviderKind,
		type ProviderUpdate,
		type Settings
	} from '$lib/api';
	import ClaudeCodeLogin from '$lib/components/ClaudeCodeLogin.svelte';
	import ClaudeCodeUsage from '$lib/components/ClaudeCodeUsage.svelte';
	import Menu from '$lib/components/Menu.svelte';
	import MenuItem from '$lib/components/MenuItem.svelte';
	import ProviderIcon from '$lib/components/ProviderIcon.svelte';
	import Submenu from '$lib/components/Submenu.svelte';
	import { CaretRightIcon, PlugsConnectedIcon, PlusIcon, XIcon } from '$lib/icons';
	import {
		brand,
		catalog,
		examples,
		labels,
		order,
		type ApiKind,
		type Brand
	} from '$lib/providers';
	import { onMount, tick } from 'svelte';

	type Draft = {
		id: string | null;
		brand: Brand | undefined;
		name: string;
		kind: ProviderKind;
		url: string | null;
		key: string;
		hasKey: boolean;
	};

	let settings = $state<Settings | null>(null);
	let account = $state<ClaudeCodeAccount | null>(null);
	let accountFailure = $state<string | null>(null);
	let draft = $state<Draft | null>(null);
	let status = $state<string | null>(null);
	let saving = $state(false);
	let dialog = $state<HTMLDialogElement>();
	let keyInput = $state<HTMLInputElement>();
	let urlInput = $state<HTMLInputElement>();

	onMount(async () => {
		settings = await getSettings();
		if (settings.providers.some((provider) => provider.kind === 'claude_code')) {
			await loadAccount();
		}
	});

	async function loadAccount() {
		accountFailure = null;
		try {
			account = await getClaudeCodeAccount();
		} catch (error) {
			accountFailure = error instanceof Error ? error.message : String(error);
		}
	}

	function signIn(): string {
		if (accountFailure) return 'Could not check the Claude Code sign-in';
		if (!account) return 'Checking the Claude Code sign-in';
		return account.signed_in ? `Signed in as ${account.email}` : 'Not signed in to Claude Code';
	}

	async function open(next: Draft) {
		draft = next;
		status = null;
		if (next.kind === 'claude_code' && !account) loadAccount();
		await tick();
		dialog?.showModal();
		(next.brand ? keyInput : urlInput)?.focus();
	}

	function edit(provider: Provider) {
		open({
			id: provider.id,
			brand: brand(provider),
			name: provider.name,
			kind: provider.kind,
			url: provider.url,
			key: '',
			hasKey: provider.has_key
		});
	}

	function addBrand(entry: Brand) {
		open({
			id: null,
			brand: entry,
			name: entry.label,
			kind: entry.kind,
			url: entry.url,
			key: '',
			hasKey: false
		});
	}

	function addCustom(kind: ApiKind) {
		open({ id: null, brand: undefined, name: '', kind, url: '', key: '', hasKey: false });
	}

	function unchanged(provider: Provider): ProviderUpdate {
		return { id: provider.id, name: provider.name, kind: provider.kind, url: provider.url };
	}

	function changed(current: Draft): ProviderUpdate {
		return {
			id: current.id,
			name: current.name,
			kind: current.kind,
			url: current.url,
			key: current.key || undefined
		};
	}

	async function persist(providers: ProviderUpdate[]): Promise<boolean> {
		saving = true;
		status = null;
		try {
			settings = await patchSettings({ providers });
			return true;
		} catch (error) {
			status = error instanceof Error ? error.message : String(error);
			return false;
		} finally {
			saving = false;
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!draft || !settings) return;
		const current = draft;
		const providers = current.id
			? settings.providers.map((provider) =>
					provider.id === current.id ? changed(current) : unchanged(provider)
				)
			: [...settings.providers.map(unchanged), changed(current)];
		if (await persist(providers)) dialog?.close();
	}

	async function remove() {
		if (!draft || !settings) return;
		const id = draft.id;
		const providers = settings.providers
			.filter((provider) => provider.id !== id)
			.map(unchanged);
		if (await persist(providers)) dialog?.close();
	}

	async function clearKey() {
		if (!draft || !settings) return;
		const current = draft;
		const providers = settings.providers.map((provider) =>
			provider.id === current.id ? { ...unchanged(provider), key: '' } : unchanged(provider)
		);
		if (await persist(providers)) current.hasKey = false;
	}
</script>

{#if settings}
	<div class="mx-auto w-full max-w-170 px-6 py-6">
		<div class="flex items-end justify-between gap-4 pb-3 pl-1">
			<div>
				<h2 class="text-title font-medium">Providers</h2>
				<p class="text-secondary">Where the agent sends its requests</p>
			</div>
			<Menu placement="bottom-end">
				{#snippet trigger(attributes)}
					<button
						type="button"
						class="button h-6.5 shrink-0 gap-1.5 px-2 text-label text-secondary hover:bg-window-text/8 hover:text-window-text"
						{...attributes}
					>
						<PlusIcon size={12} />
						Add provider
					</button>
				{/snippet}
				{#each catalog as entry (entry.label)}
					<MenuItem onselect={() => addBrand(entry)}>
						<ProviderIcon brand={entry} size={14} />
						{entry.label}
					</MenuItem>
				{/each}
				<div class="mx-1 my-1 h-px bg-light" role="separator"></div>
				<Submenu>
					{#snippet label()}
						<PlugsConnectedIcon size={14} />
						Custom endpoint
					{/snippet}
					{#each order as kind (kind)}
						<MenuItem onselect={() => addCustom(kind)}>{labels[kind]}</MenuItem>
					{/each}
				</Submenu>
			</Menu>
		</div>

		{#if settings.providers.length}
			<div class="flex flex-col rounded-surface surface p-1">
				{#each settings.providers as provider (provider.id)}
					<button
						type="button"
						class="flex items-center gap-3 rounded-input px-2 py-2 text-left transition-colors hover:bg-window-text/5"
						onclick={() => edit(provider)}
					>
						<span
							class="inline-flex size-8 shrink-0 items-center justify-center rounded-control bg-alternate-base"
						>
							<ProviderIcon brand={brand(provider)} size={16} />
						</span>
						<span class="min-w-0 flex-1">
							<span class="block truncate text-title font-medium"
								>{provider.name}</span
							>
							<span class="block truncate text-label text-secondary"
								>{provider.url ?? signIn()}</span
							>
						</span>
						{#if provider.kind !== 'claude_code' && !provider.has_key}
							<span class="shrink-0 text-label text-secondary">No API key</span>
						{/if}
						<span class="inline-flex shrink-0 text-secondary"
							><CaretRightIcon size={12} /></span
						>
					</button>
				{/each}
			</div>
			{#if settings.providers.some((provider) => provider.kind === 'claude_code')}
				<div class="mt-4"><ClaudeCodeUsage /></div>
			{/if}
		{:else}
			<div class="grid grid-cols-2 gap-2">
				{#each catalog as entry (entry.label)}
					<button
						type="button"
						class="flex items-center gap-3 rounded-surface surface px-3 py-2.5 text-left transition-colors hover:bg-alternate-base"
						onclick={() => addBrand(entry)}
					>
						<span
							class="inline-flex size-8 shrink-0 items-center justify-center rounded-control bg-alternate-base"
						>
							<ProviderIcon brand={entry} size={16} />
						</span>
						<span class="min-w-0">
							<span class="block truncate text-title font-medium">{entry.label}</span>
							<span class="block truncate text-label text-secondary"
								>{labels[entry.kind]}</span
							>
						</span>
					</button>
				{/each}
			</div>
		{/if}
	</div>
{/if}

<dialog
	bind:this={dialog}
	class="m-auto w-full max-w-md rounded-surface surface text-window-text shadow-overlay backdrop:bg-black/50"
	onclose={() => (draft = null)}
>
	{#if draft}
		<form onsubmit={submit}>
			<div class="flex items-center gap-3 border-b border-light px-4 py-3">
				<span
					class="inline-flex size-8 shrink-0 items-center justify-center rounded-control bg-alternate-base"
				>
					<ProviderIcon brand={draft.brand} size={16} />
				</span>
				<div class="min-w-0 flex-1">
					<div class="truncate text-title font-medium">
						{draft.brand?.label ?? `Custom ${labels[draft.kind]} endpoint`}
					</div>
					<div class="truncate text-label text-secondary">
						{draft.brand?.url ?? labels[draft.kind]}
					</div>
				</div>
				<button
					type="button"
					class="icon-button text-secondary"
					aria-label="Close"
					onclick={() => dialog?.close()}
				>
					<XIcon size={14} />
				</button>
			</div>
			<div class="p-2">
				<label class="field">
					<span class="w-20 shrink-0 text-secondary">Name</span>
					<input
						class="input"
						bind:value={draft.name}
						placeholder={draft.brand?.label ?? labels[draft.kind]}
						spellcheck="false"
					/>
				</label>
				{#if !draft.brand && draft.kind !== 'claude_code'}
					<label class="field">
						<span class="w-20 shrink-0 text-secondary">Endpoint</span>
						<input
							bind:this={urlInput}
							class="input"
							bind:value={draft.url}
							placeholder={examples[draft.kind]}
							spellcheck="false"
						/>
					</label>
				{/if}
				{#if draft.kind !== 'claude_code'}
					<label class="field">
						<span class="w-20 shrink-0 text-secondary">API key</span>
						<input
							bind:this={keyInput}
							class="input"
							bind:value={draft.key}
							type="password"
							autocomplete="off"
							placeholder={draft.hasKey ? 'Saved, leave empty to keep it' : 'Not set'}
						/>
						{#if draft.hasKey}
							<button
								type="button"
								class="button enabled:hover:bg-window-text/8"
								disabled={saving}
								onclick={clearKey}
							>
								Clear
							</button>
						{/if}
					</label>
				{:else}
					<ClaudeCodeLogin bind:account />
				{/if}
			</div>
			<div class="flex items-center gap-2 border-t border-light px-4 py-3">
				{#if draft.id}
					<button
						type="button"
						class="-ml-2 button text-danger enabled:hover:bg-danger/10"
						disabled={saving}
						onclick={remove}
					>
						Remove
					</button>
				{/if}
				{#if status}
					<span class="min-w-0 truncate text-label text-danger">{status}</span>
				{/if}
				<button
					type="button"
					class="ml-auto button enabled:hover:bg-window-text/8"
					onclick={() => dialog?.close()}
				>
					Cancel
				</button>
				<button
					type="submit"
					class="button bg-window-text font-medium text-window enabled:hover:bg-window-text/90"
					disabled={saving || draft.url?.trim() === ''}
				>
					{draft.id ? 'Save' : 'Add'}
				</button>
			</div>
		</form>
	{/if}
</dialog>
