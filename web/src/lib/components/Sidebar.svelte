<script lang="ts">
	import { afterNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		DesktopTowerIcon,
		DotsThreeIcon,
		GearSixIcon,
		MonitorIcon,
		PlusIcon,
		SidebarSimpleIcon,
		TrashIcon
	} from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import { sessions } from '$lib/sessions.svelte';
	import { cubicOut } from 'svelte/easing';
	import { fade, fly } from 'svelte/transition';
	import Menu from './Menu.svelte';
	import MenuItem from './MenuItem.svelte';
	import Spinner from './Spinner.svelte';
	import Wordmark from './Wordmark.svelte';

	let { open = $bindable() }: { open: boolean } = $props();

	const menus = $state<Record<string, boolean>>({});

	afterNavigate(() => {
		open = false;
	});

	function keydown(event: KeyboardEvent) {
		if (open && event.key === 'Escape') {
			open = false;
		}
	}

	async function remove(id: string) {
		await sessions.remove(id);
		if (page.params.id === id) {
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

		<div class="px-4 pt-3 pb-1.5 label">Sessions</div>
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
						<MenuItem danger onselect={() => remove(session.id)}>
							<TrashIcon size={14} />
							Delete
						</MenuItem>
					</Menu>
				</div>
			{/each}
		</nav>

		<footer class="mt-auto p-1.5">
			<a
				class={[
					'nav-item gap-2.5 px-1.75',
					page.url.pathname === '/settings' && 'bg-alternate-base'
				]}
				href="/settings"
			>
				<GearSixIcon size={16} />
				Settings
			</a>
		</footer>
	</aside>
{/if}
