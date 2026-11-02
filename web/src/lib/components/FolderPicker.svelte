<script lang="ts">
	import { listFolders } from '$lib/api';
	import { CaretDownIcon, FolderIcon, FolderPlusIcon } from '$lib/icons';
	import { follow, tooltip } from '$lib/popup';

	let { node, value = $bindable() }: { node: string; value: string } = $props();

	const id = $props.id();

	let button: HTMLButtonElement;
	let popup: HTMLElement;
	let input: HTMLInputElement;
	let folders = $state<string[]>([]);
	let failure = $state<string | null>(null);
	let query = $state('');
	let active = $state(0);
	let stop: (() => void) | undefined;

	const name = $derived(query.trim());
	const valid = $derived(name !== '' && !name.startsWith('.') && !name.includes('/'));
	const options = $derived([
		...folders
			.filter((folder) => folder.toLowerCase().includes(name.toLowerCase()))
			.map((folder) => ({ folder, create: false })),
		...(valid && !folders.includes(name) ? [{ folder: name, create: true }] : [])
	]);

	async function beforetoggle(event: ToggleEvent) {
		stop?.();
		stop = undefined;
		if (event.newState !== 'open') return;

		query = '';
		active = 0;
		failure = null;
		requestAnimationFrame(() => {
			if (!popup.matches(':popover-open')) return;
			stop = follow(button, popup, 'top-end');
			input.focus();
		});

		try {
			folders = await listFolders(node);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
	}

	function choose(folder: string) {
		value = folder;
		popup.hidePopover();
	}

	function keydown(event: KeyboardEvent) {
		const count = options.length;
		if (event.key === 'ArrowDown' && count) {
			active = (active + 1) % count;
		} else if (event.key === 'ArrowUp' && count) {
			active = (active - 1 + count) % count;
		} else if (event.key === 'Enter' && options[active]) {
			choose(options[active].folder);
		} else {
			return;
		}
		event.preventDefault();
	}
</script>

<button
	bind:this={button}
	class="button h-6.5 shrink-0 gap-1.5 px-2 text-label text-secondary hover:bg-window-text/8 hover:text-window-text"
	popovertarget={id}
	{@attach tooltip('The folder the session works in')}
>
	<FolderIcon size={14} />
	{value || 'Folder'}
	<CaretDownIcon size={10} />
</button>

<div bind:this={popup} {id} class="popup w-64" popover="auto" onbeforetoggle={beforetoggle}>
	<input
		bind:this={input}
		bind:value={query}
		class="mb-1 input w-full"
		placeholder="Find or create a folder"
		role="combobox"
		aria-expanded="true"
		aria-controls="{id}-options"
		aria-activedescendant={options[active] ? `${id}-${active}` : undefined}
		oninput={() => (active = 0)}
		onkeydown={keydown}
	/>
	<div id="{id}-options" class="max-h-60 overflow-y-auto" role="listbox">
		{#each options as option, index (option.folder)}
			<button
				id="{id}-{index}"
				class={['menu-item', index === active && 'bg-window-text/8']}
				role="option"
				aria-selected={index === active}
				tabindex="-1"
				onclick={() => choose(option.folder)}
				onpointermove={() => (active = index)}
			>
				{#if option.create}
					<FolderPlusIcon size={14} />
					<span class="truncate">Create {option.folder}</span>
				{:else}
					<FolderIcon size={14} />
					<span class="truncate">{option.folder}</span>
				{/if}
			</button>
		{/each}
	</div>
	{#if failure}
		<p class="px-2 py-1 text-label text-danger">{failure}</p>
	{:else if !options.length}
		<p class="px-2 py-1 text-label text-secondary">
			{name
				? "Folder names can't start with . or contain /"
				: 'Type a name to create a folder'}
		</p>
	{/if}
</div>
