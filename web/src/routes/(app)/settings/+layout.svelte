<script lang="ts">
	import { page } from '$app/state';
	import { PlugsConnectedIcon } from '$lib/icons';

	let { children } = $props();

	const sections = [
		{ href: '/settings/providers', label: 'Providers', icon: PlugsConnectedIcon }
	];
</script>

<div class="flex min-w-0 flex-1 flex-col">
	<header class="toolbar pr-6 pl-11 text-title font-medium">Settings</header>
	<div class="flex min-h-0 flex-1">
		<nav class="flex w-52 shrink-0 flex-col gap-0.5 px-3 py-6">
			{#each sections as section (section.href)}
				<a
					class={[
						'nav-item gap-2.5 px-2',
						page.url.pathname === section.href && 'bg-alternate-base'
					]}
					href={section.href}
				>
					<section.icon size={16} />
					{section.label}
				</a>
			{/each}
		</nav>
		<div class="min-w-0 flex-1 overflow-y-auto">
			{@render children()}
		</div>
	</div>
</div>
