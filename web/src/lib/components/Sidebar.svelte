<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { GearSixIcon, PlusIcon, SidebarSimpleIcon, TrashIcon } from '$lib/icons';
	import { sessions } from '$lib/sessions.svelte';

	let { collapsed = $bindable() }: { collapsed: boolean } = $props();

	async function create() {
		const session = await sessions.create();
		await goto(`/sessions/${session.id}`);
	}

	async function remove(id: string) {
		await sessions.remove(id);
		if (page.params.id === id) {
			await goto('/');
		}
	}
</script>

<aside class:collapsed>
	<header>
		<button class="icon-button" title="Toggle sidebar" onclick={() => (collapsed = !collapsed)}>
			<SidebarSimpleIcon size={16} />
		</button>
		{#if !collapsed}
			<span class="name">tsoup</span>
			<button class="icon-button" title="New session" onclick={create}>
				<PlusIcon size={16} />
			</button>
		{/if}
	</header>

	{#if !collapsed}
		<div class="label section">Sessions</div>
		<nav>
			{#each sessions.items as session (session.id)}
				<div class="row" class:active={page.params.id === session.id}>
					<a href="/sessions/{session.id}">{session.title || 'New session'}</a>
					<button
						class="icon-button remove"
						title="Delete session"
						onclick={() => remove(session.id)}
					>
						<TrashIcon size={14} />
					</button>
				</div>
			{/each}
		</nav>
	{/if}

	<footer>
		<a
			class="settings"
			class:active={page.url.pathname === '/settings'}
			href="/settings"
			title="Settings"
		>
			<GearSixIcon size={16} />
			{#if !collapsed}<span>Settings</span>{/if}
		</a>
	</footer>
</aside>

<style>
	aside {
		display: flex;
		flex-direction: column;
		width: 260px;
		flex-shrink: 0;
		background: var(--base);
		border-right: 1px solid var(--light);
		transition: width var(--duration) var(--ease-out-cubic);
		overflow: hidden;
	}

	aside.collapsed {
		width: 44px;
	}

	header {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 44px;
		padding: 0 9px;
		flex-shrink: 0;
	}

	.name {
		flex: 1;
		font-size: var(--font-title);
		font-weight: 500;
	}

	.section {
		padding: 12px 16px 6px;
	}

	nav {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 0 6px 12px;
		overflow-y: auto;
	}

	.row {
		display: flex;
		align-items: center;
		height: 32px;
		padding-right: 4px;
		border-radius: var(--radius-control);
		transition: background var(--duration-fast) linear;
	}

	.row:hover {
		background: var(--hover);
	}

	.row.active {
		background: var(--alternate-base);
	}

	.row a {
		flex: 1;
		min-width: 0;
		padding: 0 10px;
		color: var(--window-text);
		text-decoration: none;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		line-height: 32px;
	}

	footer {
		margin-top: auto;
		padding: 6px;
		border-top: 1px solid var(--light);
	}

	.settings {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 32px;
		padding: 0 7px;
		border-radius: var(--radius-control);
		color: var(--window-text);
		text-decoration: none;
		white-space: nowrap;
		transition: background var(--duration-fast) linear;
	}

	.settings:hover {
		background: var(--hover);
	}

	.settings.active {
		background: var(--alternate-base);
	}

	.remove {
		opacity: 0;
		color: var(--text-secondary);
	}

	.row:hover .remove {
		opacity: 1;
	}

	.remove:hover {
		color: var(--danger);
		background: var(--danger-tint);
	}
</style>
