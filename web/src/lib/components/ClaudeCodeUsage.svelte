<script lang="ts">
	import { getClaudeCodeUsage, type ClaudeCodeLimit, type ClaudeCodeUsage } from '$lib/api';
	import { ArrowClockwiseIcon } from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import { onMount } from 'svelte';
	import Spinner from './Spinner.svelte';

	const minute = 60_000;
	const hour = 60 * minute;
	const day = 24 * hour;

	let usage = $state<ClaudeCodeUsage | null>(null);
	let loading = $state(false);
	let failure = $state<string | null>(null);

	const limits = $derived(usage?.rate_limits?.limits ?? []);
	const extra = $derived(usage?.rate_limits?.extra_usage ?? null);

	onMount(load);

	async function load() {
		loading = true;
		failure = null;
		try {
			usage = await getClaudeCodeUsage();
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
		loading = false;
	}

	function label(limit: ClaudeCodeLimit): string {
		const scope = limit.scope?.model?.display_name ?? limit.scope?.surface?.display_name;
		if (limit.kind === 'session') return 'Current session';
		if (limit.kind === 'weekly_all') return 'This week, all models';
		if (limit.kind === 'weekly_scoped') return `This week, ${scope ?? 'scoped'}`;
		return limit.kind.replaceAll('_', ' ');
	}

	function resets(limit: ClaudeCodeLimit): string {
		if (!limit.resets_at) return '';
		const left = new Date(limit.resets_at).getTime() - Date.now();
		if (left <= 0) return 'Resets now';
		const days = Math.floor(left / day);
		const hours = Math.floor((left % day) / hour);
		const minutes = Math.floor((left % hour) / minute);
		if (days) return `Resets in ${days}d ${hours}h`;
		if (hours) return `Resets in ${hours}h ${minutes}m`;
		return `Resets in ${Math.max(minutes, 1)}m`;
	}

	function money(amount: number | null, currency: string | null): string {
		if (amount === null) return 'no limit';
		return new Intl.NumberFormat(undefined, {
			style: 'currency',
			currency: currency ?? 'USD'
		}).format(amount / 100);
	}
</script>

<div class="flex flex-col rounded-surface surface p-1">
	<div class="flex items-center gap-3 px-3 pt-2 pb-1">
		<span class="min-w-0 flex-1">
			<span class="block text-title font-medium">Claude Code usage</span>
			<span class="block text-label text-secondary"
				>How much of your plan's limits is used</span
			>
		</span>
		<button class="icon-button" disabled={loading} onclick={load} {@attach tooltip('Refresh')}>
			{#if loading}
				<Spinner size={14} kind="stir" />
			{:else}
				<ArrowClockwiseIcon size={14} />
			{/if}
		</button>
	</div>

	{#if failure}
		<p class="px-3 py-2 text-label text-danger">{failure}</p>
	{:else if usage}
		{#each limits as limit (limit.kind + label(limit))}
			<div class="flex flex-col gap-1.5 px-3 py-2">
				<div class="flex items-baseline gap-3">
					<span class="min-w-0 flex-1 truncate">{label(limit)}</span>
					<span class="shrink-0 text-label text-secondary">{resets(limit)}</span>
					<span class="w-9 shrink-0 text-right tabular-nums"
						>{Math.round(limit.percent)}%</span
					>
				</div>
				<div class="h-1.5 overflow-hidden rounded-full bg-light">
					<div
						class={[
							'h-full rounded-full',
							limit.severity === 'normal' ? 'bg-accent' : 'bg-danger'
						]}
						style:width="{Math.min(Math.max(limit.percent, 0), 100)}%"
					></div>
				</div>
			</div>
		{:else}
			<p class="px-3 py-2 text-secondary">No plan limits were reported</p>
		{/each}
		{#if extra?.is_enabled}
			<div class="flex items-baseline gap-3 px-3 py-2">
				<span class="min-w-0 flex-1 truncate">Extra usage this month</span>
				<span class="shrink-0 tabular-nums">
					{money(extra.used_credits ?? 0, extra.currency)} of
					{money(extra.monthly_limit, extra.currency)}
				</span>
			</div>
		{/if}
	{:else}
		<p class="px-3 py-2 text-secondary">Loading</p>
	{/if}
</div>
