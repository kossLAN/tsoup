<script lang="ts">
	import {
		getSettings,
		listModels,
		patchSettings,
		type Effort,
		type ModelChoice,
		type Provider,
		type Selection,
		type Settings
	} from '$lib/api';
	import { BrainIcon, CaretDownIcon, CheckIcon, SparkleIcon, StarIcon } from '$lib/icons';
	import { follow, tooltip, type Placement } from '$lib/popup';
	import { brand } from '$lib/providers';
	import { onMount } from 'svelte';
	import ProviderIcon from './ProviderIcon.svelte';

	type Listing = { models: string[]; failure: string | null };
	type Option = { provider: Provider; model: string; custom: boolean; index: number };

	let {
		selection,
		onchange,
		placeholder,
		placement = 'top-end',
		effort = true,
		starrable = true
	}: {
		selection: Selection;
		onchange: (selection: Selection) => void;
		placeholder?: string;
		placement?: Placement;
		effort?: boolean;
		starrable?: boolean;
	} = $props();

	const efforts: { value: Effort | null; label: string }[] = [
		{ value: null, label: 'Default' },
		{ value: 'low', label: 'Low' },
		{ value: 'medium', label: 'Medium' },
		{ value: 'high', label: 'High' },
		{ value: 'xhigh', label: 'Extra high' }
	];

	const id = $props.id();
	const requested = new Set<string>();

	let button = $state<HTMLButtonElement>();
	let popup = $state<HTMLElement>();
	let input = $state<HTMLInputElement>();
	let settings = $state<Settings | null>(null);
	let listings = $state<Record<string, Listing>>({});
	let query = $state('');
	let active = $state(0);
	let failure = $state<string | null>(null);
	let stop: (() => void) | undefined;

	const name = $derived(query.trim());
	const model = $derived(
		selection.model ?? (placeholder === undefined ? (settings?.default_model ?? null) : null)
	);
	const current = $derived(
		settings?.providers.find((provider) => provider.id === model?.provider)
	);
	const groups = $derived.by(() => {
		let index = 0;
		return (settings?.providers ?? []).map((provider) => {
			const listing = listings[provider.id];
			const listed = listing?.models ?? [];
			const options: Option[] = listed
				.filter((model) => model.toLowerCase().includes(name.toLowerCase()))
				.map((model) => ({ provider, model, custom: false, index: index++ }));
			if (name && !listed.includes(name)) {
				options.push({ provider, model: name, custom: true, index: index++ });
			}
			return { provider, listing, options };
		});
	});
	const options = $derived(groups.flatMap((group) => group.options));

	onMount(async () => {
		settings = await getSettings();
	});

	function beforetoggle(event: ToggleEvent) {
		stop?.();
		stop = undefined;
		if (event.newState !== 'open') return;

		query = '';
		active = 0;
		failure = null;
		requestAnimationFrame(() => {
			if (!button || !popup?.matches(':popover-open')) return;
			stop = follow(button, popup, placement);
			input?.focus();
		});

		for (const provider of settings?.providers ?? []) {
			if (requested.has(provider.id)) continue;
			requested.add(provider.id);
			list(provider);
		}
	}

	async function list(provider: Provider) {
		try {
			listings[provider.id] = { models: await listModels(provider.id), failure: null };
		} catch (error) {
			listings[provider.id] = { models: [], failure: describe(error) };
		}
	}

	function describe(error: unknown): string {
		return error instanceof Error ? error.message : String(error);
	}

	function choice(option: Option): ModelChoice {
		return { provider: option.provider.id, name: option.model };
	}

	function matches(choice: ModelChoice | null, option: Option): boolean {
		return choice?.provider === option.provider.id && choice.name === option.model;
	}

	function choose(option: Option) {
		popup?.hidePopover();
		onchange({ model: choice(option), effort: selection.effort });
	}

	async function star(option: Option) {
		failure = null;
		try {
			settings = await patchSettings({ default_model: choice(option) });
		} catch (error) {
			failure = describe(error);
		}
	}

	function keydown(event: KeyboardEvent) {
		const count = options.length;
		if (event.key === 'ArrowDown' && count) {
			active = (active + 1) % count;
		} else if (event.key === 'ArrowUp' && count) {
			active = (active - 1 + count) % count;
		} else if (event.key === 'Enter' && options[active]) {
			choose(options[active]);
		} else {
			return;
		}
		event.preventDefault();
	}
</script>

{#if settings}
	<button
		bind:this={button}
		class="button h-6.5 max-w-64 min-w-0 shrink gap-1.5 px-2 text-label text-secondary hover:bg-window-text/8 hover:text-window-text"
		popovertarget={id}
		{@attach tooltip(effort ? 'Model and reasoning effort' : 'Model')}
	>
		{#if model && current}
			<ProviderIcon brand={brand(current)} size={14} />
			<span class="truncate">{model.name}</span>
		{:else}
			<SparkleIcon size={14} />
			<span class="truncate">{placeholder ?? 'Choose a model'}</span>
		{/if}
		<CaretDownIcon size={10} />
	</button>

	<div bind:this={popup} {id} class="popup w-80" popover="auto" onbeforetoggle={beforetoggle}>
		{#if settings.providers.length}
			<input
				bind:this={input}
				bind:value={query}
				class="mb-1 input w-full"
				placeholder="Find a model or type its id"
				spellcheck="false"
				role="combobox"
				aria-expanded="true"
				aria-controls="{id}-options"
				aria-activedescendant={options[active] ? `${id}-${active}` : undefined}
				oninput={() => (active = 0)}
				onkeydown={keydown}
			/>
			<div id="{id}-options" class="h-72 overflow-y-auto" role="listbox">
				{#each groups as group (group.provider.id)}
					<div class="flex items-center gap-2 px-2 pt-2 pb-1 label">
						<ProviderIcon brand={brand(group.provider)} size={12} />
						<span class="truncate">{group.provider.name}</span>
					</div>
					{#each group.options as option (option.model)}
						{@const starred = matches(settings.default_model, option)}
						<div
							class={[
								'flex items-center rounded-input',
								option.index === active && 'bg-window-text/8'
							]}
						>
							<button
								id="{id}-{option.index}"
								class="menu-item min-w-0 flex-1 focus:bg-transparent"
								role="option"
								aria-selected={option.index === active}
								tabindex="-1"
								onclick={() => choose(option)}
								onpointermove={() => (active = option.index)}
							>
								<span class="truncate"
									>{option.custom ? `Use ${option.model}` : option.model}</span
								>
								{#if matches(model, option)}
									<span class="ml-auto inline-flex pl-3 text-accent"
										><CheckIcon size={12} /></span
									>
								{/if}
							</button>
							{#if starrable}
								<button
									class={[
										'mr-0.5 icon-button size-6',
										starred ? 'text-accent' : 'text-secondary',
										!starred && option.index !== active && 'invisible'
									]}
									aria-label={starred
										? 'Default for new sessions'
										: 'Use for new sessions'}
									aria-pressed={starred}
									tabindex="-1"
									onclick={() => star(option)}
									onpointermove={() => (active = option.index)}
								>
									<StarIcon size={12} weight={starred ? 'fill' : 'regular'} />
								</button>
							{/if}
						</div>
					{:else}
						<p
							class={[
								'line-clamp-3 px-2 py-1 text-label break-words',
								group.listing?.failure ? 'text-danger' : 'text-secondary'
							]}
						>
							{#if !group.listing}
								Loading models
							{:else if group.listing.failure}
								Could not list models: {group.listing.failure}
							{:else}
								No models listed, type a model id
							{/if}
						</p>
					{/each}
				{/each}
			</div>
			{#if failure}
				<p class="px-2 pt-1 text-label text-danger">{failure}</p>
			{/if}
		{:else}
			<p class="px-2 py-1.5 text-secondary">
				Add a provider in <a class="text-accent underline" href="/settings">settings</a> to choose
				a model
			</p>
		{/if}
		{#if effort}
			<div class="mt-1 border-t border-light px-1 pt-2 pb-1">
				<div class="flex items-center gap-1.5 px-1 pb-1.5 label">
					<BrainIcon size={12} />
					Reasoning effort
				</div>
				<div class="grid grid-cols-5 gap-0.5 rounded-input bg-alternate-base p-0.5">
					{#each efforts as level (level.label)}
						<button
							class={[
								'h-6 rounded-small text-label transition-colors',
								level.value === selection.effort
									? 'bg-light text-window-text'
									: 'text-secondary hover:text-window-text'
							]}
							aria-pressed={level.value === selection.effort}
							onclick={() =>
								onchange({ model: selection.model, effort: level.value })}
						>
							{level.label}
						</button>
					{/each}
				</div>
			</div>
		{/if}
	</div>
{/if}
