<script lang="ts">
	import { afterNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { GearSixIcon, PlusIcon, SidebarSimpleIcon, TrashIcon } from '$lib/icons';
	import { sessions } from '$lib/sessions.svelte';
	import { cubicOut } from 'svelte/easing';
	import { fade, fly } from 'svelte/transition';

	let { open = $bindable() }: { open: boolean } = $props();

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
	title="Open sidebar"
	onclick={() => (open = true)}
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
			<button class="icon-button" title="Close sidebar" onclick={() => (open = false)}>
				<SidebarSimpleIcon size={16} />
			</button>
			<span class="flex-1 text-title font-medium">tsoup</span>
			<a class="icon-button" href="/" title="New session" onclick={() => (open = false)}>
				<PlusIcon size={16} />
			</a>
		</header>

		<div class="px-4 pt-3 pb-1.5 label">Sessions</div>
		<nav class="flex flex-1 flex-col gap-0.5 overflow-y-auto px-1.5 pb-3">
			{#each sessions.items as session (session.id)}
				<div
					class={[
						'group nav-item pr-1',
						page.params.id === session.id && 'bg-alternate-base'
					]}
				>
					<a class="flex-1 truncate px-2.5 leading-8" href="/sessions/{session.id}">
						{session.title || 'New session'}
					</a>
					<button
						class="icon-button text-secondary opacity-0 group-hover:opacity-100 hover:text-window-text"
						title="Delete session"
						onclick={() => remove(session.id)}
					>
						<TrashIcon size={14} />
					</button>
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
