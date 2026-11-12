<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		createGroup,
		deleteGroup,
		patchSession,
		placeSession,
		renameGroup,
		type SessionSummary
	} from '$lib/api';
	import {
		CheckIcon,
		DesktopTowerIcon,
		DotsThreeIcon,
		FolderIcon,
		FolderMinusIcon,
		FolderOpenIcon,
		FolderPlusIcon,
		GearSixIcon,
		MonitorIcon,
		PencilSimpleIcon,
		PlusIcon,
		SidebarSimpleIcon,
		TrashIcon
	} from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import { sessions } from '$lib/sessions.svelte';
	import { cubicOut } from 'svelte/easing';
	import { SvelteSet } from 'svelte/reactivity';
	import { slide } from 'svelte/transition';
	import Menu from './Menu.svelte';
	import MenuItem from './MenuItem.svelte';
	import Spinner from './Spinner.svelte';
	import Submenu from './Submenu.svelte';
	import Wordmark from './Wordmark.svelte';

	type Editing = { kind: 'session' | 'folder'; id: string } | { kind: 'new' };

	const FOLDED_KEY = 'tsoup.folded';
	const DRAG_TYPE = 'application/x-tsoup-session';

	let { open = $bindable() }: { open: boolean } = $props();

	const menus = $state<Record<string, boolean>>({});
	const selected = new SvelteSet<string>();
	const folded = new SvelteSet<string>(stored());
	const action =
		'button h-6 px-1.5 text-label text-secondary enabled:hover:bg-window-text/8 enabled:hover:text-window-text';

	let selecting = $state(false);
	let editing = $state<Editing | null>(null);
	let name = $state('');
	let over = $state<{ group: string | null } | null>(null);

	const folders = $derived(
		sessions.groups.map((group) => ({
			group,
			members: sessions.items.filter((session) => session.group === group.id)
		}))
	);
	const loose = $derived(
		sessions.items.filter(
			(session) => !sessions.groups.some((group) => group.id === session.group)
		)
	);

	function stored(): string[] {
		try {
			const ids: unknown = JSON.parse(localStorage.getItem(FOLDED_KEY) ?? '[]');
			return Array.isArray(ids) ? ids.filter((id) => typeof id === 'string') : [];
		} catch {
			return [];
		}
	}

	function fold(id: string) {
		if (!folded.delete(id)) folded.add(id);
		localStorage.setItem(FOLDED_KEY, JSON.stringify([...folded]));
	}

	function select(active: boolean) {
		selecting = active;
		selected.clear();
	}

	function toggle(id: string) {
		if (!selected.delete(id)) selected.add(id);
	}

	function all() {
		const every = selected.size === sessions.items.length;
		selected.clear();
		if (!every) for (const session of sessions.items) selected.add(session.id);
	}

	function edit(target: Editing, value = '') {
		editing = target;
		name = value;
	}

	function renaming(kind: 'session' | 'folder', id: string): boolean {
		return editing?.kind === kind && editing.id === id;
	}

	async function commit() {
		const edited = editing;
		const value = name.trim();
		editing = null;
		if (!edited || !value) return;
		if (edited.kind === 'new') await createGroup(value);
		else if (edited.kind === 'folder') await renameGroup(edited.id, value);
		else await patchSession(edited.id, { title: value });
	}

	async function place(ids: string[], group: string | null) {
		await Promise.all(ids.map((id) => placeSession(id, group)));
	}

	async function remove(ids: string[]) {
		await Promise.all(ids.map((id) => sessions.remove(id)));
		for (const id of ids) selected.delete(id);
		if (page.params.id && ids.includes(page.params.id)) {
			await goto('/');
		}
	}

	function dropzone(group: string | null) {
		return {
			ondragover(event: DragEvent) {
				if (!event.dataTransfer?.types.includes(DRAG_TYPE)) return;
				event.preventDefault();
				over = { group };
			},
			ondragleave(event: DragEvent & { currentTarget: HTMLElement }) {
				if (!event.currentTarget.contains(event.relatedTarget as Node | null)) over = null;
			},
			ondrop(event: DragEvent) {
				const id = event.dataTransfer?.getData(DRAG_TYPE);
				over = null;
				if (!id) return;
				event.preventDefault();
				place([id], group);
			}
		};
	}
</script>

{#snippet field(placeholder: string)}
	<input
		class="mx-1 input"
		bind:value={name}
		{placeholder}
		onkeydown={(event) => {
			if (event.key === 'Enter') commit();
			if (event.key === 'Escape') editing = null;
			event.stopPropagation();
		}}
		onblur={commit}
		spellcheck="false"
		{@attach (input) => {
			input.focus();
			input.select();
		}}
	/>
{/snippet}

{#snippet destinations(ids: string[], current: string | null)}
	{#each sessions.groups as group (group.id)}
		<MenuItem disabled={group.id === current} onselect={() => place(ids, group.id)}>
			<FolderIcon size={14} />
			<span class="max-w-48 truncate">{group.name}</span>
		</MenuItem>
	{/each}
	{#if current !== null || ids.length > 1}
		<MenuItem onselect={() => place(ids, null)}>
			<FolderMinusIcon size={14} />
			No folder
		</MenuItem>
	{/if}
{/snippet}

{#snippet row(session: SessionSummary)}
	{@const active = menus[session.id] ?? false}
	{@const status = session.busy
		? 'Generating'
		: sessions.unseen.has(session.id)
			? 'Finished'
			: null}
	<div
		class={[
			'group nav-item shrink-0 pr-1',
			page.params.id === session.id ? 'bg-alternate-base' : active && 'bg-window-text/8'
		]}
		role="listitem"
		draggable={!selecting && !renaming('session', session.id)}
		ondragstart={(event) => event.dataTransfer?.setData(DRAG_TYPE, session.id)}
	>
		{#if selecting}
			<button
				class="flex min-w-0 flex-1 items-center gap-2 pr-1 pl-2.5 text-left leading-8"
				role="checkbox"
				aria-checked={selected.has(session.id)}
				onclick={() => toggle(session.id)}
			>
				<span
					class={[
						'inline-flex size-3.5 shrink-0 items-center justify-center rounded-small border',
						selected.has(session.id)
							? 'border-accent bg-accent text-window'
							: 'border-light'
					]}
				>
					{#if selected.has(session.id)}<CheckIcon size={10} weight="bold" />{/if}
				</span>
				<span class="flex-1 truncate">{session.title || 'New session'}</span>
			</button>
		{:else if renaming('session', session.id)}
			{@render field('Session name')}
		{:else}
			<a
				class="flex min-w-0 flex-1 items-center gap-1.5 pr-1 pl-2.5 leading-8"
				href="/sessions/{session.id}"
				draggable="false"
			>
				<span class="flex-1 truncate">{session.title || 'New session'}</span>
				<span
					class={[
						'flex items-center gap-1.5 text-secondary',
						active ? 'hidden' : 'group-hover:hidden'
					]}
				>
					{#if session.environment.kind === 'node'}
						<span
							class="status"
							role="img"
							{@attach tooltip(
								`${session.environment.folder} on ${session.environment.name}`
							)}
						>
							<DesktopTowerIcon size={12} />
						</span>
					{:else if session.environment.vm.state !== 'stopped' && session.environment.vm.state !== 'failed'}
						<span class="status" role="img" {@attach tooltip('VM running')}>
							<MonitorIcon size={12} />
						</span>
					{/if}
					{#if status}
						<span class="status" role="img" {@attach tooltip(status)}>
							{#if session.busy}
								<Spinner size={12} />
							{:else}
								<span class="size-1.5 rounded-full bg-accent"></span>
							{/if}
						</span>
					{/if}
				</span>
			</a>
			<Menu
				placement="bottom-end"
				bind:open={() => active, (value) => (menus[session.id] = value)}
			>
				{#snippet trigger(attributes)}
					<span class={active ? 'flex' : 'hidden group-hover:flex'}>
						<button
							class="icon-button text-secondary hover:text-window-text"
							{...attributes}
							{@attach tooltip('More')}
						>
							<DotsThreeIcon size={16} weight="bold" />
						</button>
					</span>
				{/snippet}
				<MenuItem onselect={() => edit({ kind: 'session', id: session.id }, session.title)}>
					<PencilSimpleIcon size={14} />
					Rename
				</MenuItem>
				{#if sessions.groups.length}
					<Submenu>
						{#snippet label()}
							<FolderIcon size={14} />
							Move to
						{/snippet}
						{@render destinations([session.id], session.group)}
					</Submenu>
				{/if}
				<MenuItem danger onselect={() => remove([session.id])}>
					<TrashIcon size={14} />
					Delete
				</MenuItem>
			</Menu>
		{/if}
	</div>
{/snippet}

{#if !open}
	<button
		class="fixed top-2.25 left-2.25 z-10 icon-button"
		onclick={() => (open = true)}
		{@attach tooltip('Open sidebar', 'right')}
	>
		<SidebarSimpleIcon size={16} />
	</button>
{/if}

{#if open}
	<div
		class="h-screen shrink-0 overflow-hidden"
		transition:slide={{ axis: 'x', duration: 200, easing: cubicOut }}
	>
		<aside class="flex h-full w-65 flex-col border-r border-light bg-base">
			<header class="toolbar gap-1.5 px-2.25">
				<button
					class="icon-button"
					onclick={() => (open = false)}
					{@attach tooltip('Close sidebar')}
				>
					<SidebarSimpleIcon size={16} />
				</button>
				<span class="flex-1 text-xl font-semibold tracking-tight"><Wordmark /></span>
				<a class="icon-button" href="/" {@attach tooltip('New session')}>
					<PlusIcon size={16} />
				</a>
			</header>

			<div class="flex h-8 items-center gap-0.5 pt-1.5 pr-1.5 pl-4">
				<span class="flex-1 label">Sessions</span>
				{#if selecting}
					<button class={action} onclick={all}>
						{selected.size === sessions.items.length ? 'None' : 'All'}
					</button>
					{#if sessions.groups.length}
						<Menu placement="bottom-end">
							{#snippet trigger(attributes)}
								<button
									class={action}
									disabled={selected.size === 0}
									{...attributes}
								>
									Move
								</button>
							{/snippet}
							{@render destinations([...selected], null)}
						</Menu>
					{/if}
					<button
						class={[action, 'enabled:text-danger']}
						disabled={selected.size === 0}
						onclick={() => remove([...selected])}
					>
						Delete {selected.size || ''}
					</button>
					<button class={action} onclick={() => select(false)}>Done</button>
				{:else}
					{#if sessions.items.length}
						<button class={action} onclick={() => select(true)}>Select</button>
					{/if}
					<button
						class="icon-button size-6 text-secondary hover:text-window-text"
						onclick={() => edit({ kind: 'new' })}
						{@attach tooltip('New folder')}
					>
						<FolderPlusIcon size={14} />
					</button>
				{/if}
			</div>
			<nav class="flex flex-1 flex-col gap-0.5 overflow-y-auto px-1.5 pb-3">
				{#if editing?.kind === 'new'}
					<div class="nav-item shrink-0">{@render field('Folder name')}</div>
				{/if}
				{#each folders as { group, members } (group.id)}
					{@const active = menus[group.id] ?? false}
					{@const expanded = !folded.has(group.id)}
					{@const current = members.some((session) => session.id === page.params.id)}
					<div class="flex shrink-0 flex-col gap-0.5" {...dropzone(group.id)}>
						<div
							class={[
								'group nav-item pr-1',
								over?.group === group.id
									? 'bg-accent/15'
									: !expanded && current
										? 'bg-alternate-base'
										: active && 'bg-window-text/8'
							]}
						>
							{#if renaming('folder', group.id)}
								{@render field('Folder name')}
							{:else}
								<button
									class="flex min-w-0 flex-1 items-center gap-2 pr-1 pl-2.5 text-left leading-8"
									aria-expanded={expanded}
									onclick={() => fold(group.id)}
								>
									<span class="inline-flex shrink-0 text-secondary">
										{#if expanded}
											<FolderOpenIcon size={14} />
										{:else}
											<FolderIcon size={14} />
										{/if}
									</span>
									<span class="flex-1 truncate font-medium">{group.name}</span>
									<span
										class={[
											'pr-1 text-label text-secondary tabular-nums',
											active ? 'hidden' : 'group-hover:hidden'
										]}
									>
										{members.length}
									</span>
								</button>
								<Menu
									placement="bottom-end"
									bind:open={() => active, (value) => (menus[group.id] = value)}
								>
									{#snippet trigger(attributes)}
										<span class={active ? 'flex' : 'hidden group-hover:flex'}>
											<button
												class="icon-button text-secondary hover:text-window-text"
												{...attributes}
												{@attach tooltip('More')}
											>
												<DotsThreeIcon size={16} weight="bold" />
											</button>
										</span>
									{/snippet}
									<MenuItem onselect={() => goto(`/?folder=${group.id}`)}>
										<PlusIcon size={14} />
										New session
									</MenuItem>
									<MenuItem
										onselect={() =>
											edit({ kind: 'folder', id: group.id }, group.name)}
									>
										<PencilSimpleIcon size={14} />
										Rename
									</MenuItem>
									<MenuItem danger onselect={() => deleteGroup(group.id)}>
										<TrashIcon size={14} />
										Delete folder
									</MenuItem>
								</Menu>
							{/if}
						</div>
						{#if expanded}
							<div
								class="ml-4.25 flex flex-col gap-0.5 border-l border-light pl-1"
								role="list"
							>
								{#each members as session (session.id)}
									{@render row(session)}
								{:else}
									<span class="px-2.5 text-label leading-7 text-secondary">
										No sessions yet
									</span>
								{/each}
							</div>
						{/if}
					</div>
				{/each}
				<div
					class={[
						'flex flex-1 flex-col gap-0.5 rounded-input',
						over?.group === null && sessions.groups.length > 0 && 'bg-accent/8'
					]}
					role="list"
					{...dropzone(null)}
				>
					{#each loose as session (session.id)}
						{@render row(session)}
					{/each}
				</div>
			</nav>

			<footer class="mt-auto p-1.5">
				<a
					class={[
						'nav-item gap-2.5 px-1.75',
						page.url.pathname.startsWith('/settings') && 'bg-alternate-base'
					]}
					href="/settings"
				>
					<GearSixIcon size={16} />
					Settings
				</a>
			</footer>
		</aside>
	</div>
{/if}
