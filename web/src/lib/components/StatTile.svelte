<script lang="ts" module>
	export type Delta = {
		text: string;
		rising: boolean;
		good: boolean | null;
	};
</script>

<script lang="ts">
	import { TrendDownIcon, TrendUpIcon } from '$lib/icons';

	let {
		label,
		value,
		hint,
		delta
	}: { label: string; value: string; hint?: string; delta?: Delta } = $props();
</script>

<div class="flex min-w-0 flex-col gap-1 rounded-surface surface px-4 py-3">
	<span class="truncate text-secondary">{label}</span>
	<span class="truncate text-2xl font-semibold tracking-tight">{value}</span>
	<span class="flex h-4 items-center gap-2 text-label text-secondary">
		{#if delta}
			<span
				class={[
					'inline-flex shrink-0 items-center gap-1',
					delta.good === true && 'text-positive',
					delta.good === false && 'text-negative'
				]}
			>
				{#if delta.rising}
					<TrendUpIcon size={12} />
				{:else}
					<TrendDownIcon size={12} />
				{/if}
				{delta.text}
			</span>
		{/if}
		{#if hint}
			<span class="truncate">{hint}</span>
		{/if}
	</span>
</div>
