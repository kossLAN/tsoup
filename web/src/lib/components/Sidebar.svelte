<script lang="ts">
	import { afterNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { patchSession } from '$lib/api';
	import {
		CheckIcon,
		DesktopTowerIcon,
		DotsThreeIcon,
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
	import { fade, fly } from 'svelte/transition';
	import Menu from './Menu.svelte';
	import MenuItem from './MenuItem.svelte';
	import Spinner from './Spinner.svelte';
	import Wordmark from './Wordmark.svelte';

	let { open = $bindable() }: { open: boolean } = $props();

	const menus = $state<Record<string, boolean>>({});
	const selected = new SvelteSet<string>();
	const action =
		'button h-6 px-1.5 text-label text-secondary enabled:hover:bg-window-text/8 enabled:hover:text-window-text';

	let selecting = $state(false);
	let renaming = $state<string | null>(null);
	let name = $state('');

	afterNavigate(() => {
		open = false;
	});

	function keydown(event: KeyboardEvent) {
		if (open && event.key === 'Escape') {
			open = false;
		}
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

	function rename(id: string, title: string) {
		renaming = id;
		name = title;
	}

	async function commit() {
		const id = renaming;
		const title = name.trim();
		renaming = null;
		if (id && title) await patchSession(id, { title });
	}

	async function remove(ids: string[]) {
		await Promise.all(ids.map((id) => sessions.remove(id)));
		for (const id of ids) selected.delete(id);
		if (page.params.id && ids.includes(page.params.id)) {
			await goto('/');
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<button
	class="fixed top-2.25 left-2.25 z-10 icon-button"
	onclick={() => (open = true)}
	{@attach tooltip('Open sidebar', 'right')}
>
	<SidebarSimpleIcon size={16} />
</button>

{#if open}
	<div
		class="fixed inset-0 z-20 bg-black/40"
		role="presentation"
		onclick={() => (open = false)}
		transition:fade={{ duration: 150 }}
	></div>

	<aside
		class="fixed inset-y-0 left-0 z-30 flex w-65 flex-col border-r border-light bg-base shadow-overlay"
		transition:fly={{ x: -260, opacity: 1, duration: 200, easing: cubicOut }}
	>
		<header class="toolbar gap-1.5 px-2.25">
			<button
				class="icon-button"
				onclick={() => (open = false)}
				{@attach tooltip('Close sidebar')}
			>
				<SidebarSimpleIcon size={16} />
			</button>
			<span class="flex-1 text-xl font-semibold tracking-tight"><Wordmark /></span>
			<a
				class="icon-button"
				href="/"
				onclick={() => (open = false)}
				{@attach tooltip('New session')}
			>
				<PlusIcon size={16} />
			</a>
		</header>

		<div class="flex h-8 items-center gap-0.5 pt-1.5 pr-1.5 pl-4">
			<span class="flex-1 label">Sessions</span>
			{#if selecting}
				<button class={action} onclick={all}>
					{selected.size === sessions.items.length ? 'None' : 'All'}
				</button>
				<button
					class={[action, 'enabled:text-danger']}
					disabled={selected.size === 0}
					onclick={() => remove([...selected])}
				>
					Delete {selected.size || ''}
				</button>
				<button class={action} onclick={() => select(false)}>Done</button>
			{:else if sessions.items.length}
				<button class={action} onclick={() => select(true)}>Select</button>
			{/if}
		</div>
		<nav class="flex flex-1 flex-col gap-0.5 overflow-y-auto px-1.5 pb-3">
			{#each sessions.items as session (session.id)}
				{@const active = menus[session.id] ?? false}
				{@const status = session.busy
					? 'Generating'
					: sessions.unseen.has(session.id)
						? 'Finished'
						: null}
				<div
					class={[
						'group nav-item pr-1',
						page.params.id === session.id
							? 'bg-alternate-base'
							: active && 'bg-window-text/8'
					]}
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
								{#if selected.has(session.id)}<CheckIcon
										size={10}
										weight="bold"
									/>{/if}
							</span>
							<span class="flex-1 truncate">{session.title || 'New session'}</span>
						</button>
					{:else if renaming === session.id}
						<input
							class="mx-1 input"
							bind:value={name}
							onkeydown={(event) => {
								if (event.key === 'Enter') commit();
								if (event.key === 'Escape') renaming = null;
								event.stopPropagation();
							}}
							onblur={commit}
							spellcheck="false"
							{@attach (input) => input.select()}
						/>
					{:else}
						<a
							class="flex min-w-0 flex-1 items-center gap-1.5 pr-1 pl-2.5 leading-8"
							href="/sessions/{session.id}"
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
							<MenuItem onselect={() => rename(session.id, session.title)}>
								<PencilSimpleIcon size={14} />
								Rename
							</MenuItem>
							<MenuItem danger onselect={() => remove([session.id])}>
								<TrashIcon size={14} />
								Delete
							</MenuItem>
						</Menu>
					{/if}
				</div>
			{/each}
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
{/if}
