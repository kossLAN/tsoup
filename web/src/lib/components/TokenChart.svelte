<script lang="ts">
	import {
		compact,
		dateOf,
		dayOf,
		OTHER,
		seriesColor,
		seriesOf,
		today,
		tokens,
		whole
	} from '$lib/analytics';
	import type { Analytics } from '$lib/api';
	import { ChartBarIcon, TableIcon } from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import ChartTooltip from './ChartTooltip.svelte';

	type Column = { start: number; end: number; values: number[]; total: number };

	let { report, since }: { report: Analytics; since: number } = $props();

	const HEIGHT = 200;
	const TOP = 8;
	const AXIS = 24;
	const GUTTER = 44;
	const MARGIN = 12;
	const BAR = 24;
	const GAP = 2;
	const RADIUS = 4;
	const LABEL_SPACING = 64;

	let width = $state(0);
	let active = $state<number | null>(null);
	let tabular = $state(false);

	const series = $derived.by(() => {
		const used = new Set(report.models.map((model) => seriesOf(report.ranking, model)));
		const named = report.ranking
			.map((model, slot) => ({
				slot: slot as number | null,
				label: model.model,
				color: seriesColor(slot)
			}))
			.filter(({ slot }) => used.has(slot));
		return used.has(null)
			? [...named, { slot: null, label: 'Other models', color: OTHER }]
			: named;
	});

	const span = $derived.by(() => {
		const last = today();
		const first =
			since > 0 ? dayOf(since) : Math.min(last, ...report.days.map(({ day }) => day));
		const days = last - first + 1;
		const step = days <= 92 ? 1 : days <= 644 ? 7 : 30;
		return { last, step, count: Math.ceil(days / step) };
	});

	const columns = $derived.by(() => {
		const { last, step, count } = span;
		const columns: Column[] = Array.from({ length: count }, (_, index) => {
			const end = last - (count - 1 - index) * step;
			return { start: end - step + 1, end, values: series.map(() => 0), total: 0 };
		});
		for (const row of report.days) {
			const column = columns[count - 1 - Math.floor((last - row.day) / step)];
			if (!column) continue;
			const slot = seriesOf(report.ranking, row);
			column.values[series.findIndex((entry) => entry.slot === slot)] += tokens(row);
			column.total += tokens(row);
		}
		return columns;
	});

	const scale = $derived.by(() => {
		const max = Math.max(...columns.map(({ total }) => total));
		if (max <= 0) return { top: 1, ticks: [0] };
		const rough = max / 4;
		const power = 10 ** Math.floor(Math.log10(rough));
		const unit = power * ([1, 2, 2.5, 5].find((factor) => factor * power >= rough) ?? 10);
		const top = Math.ceil(max / unit) * unit;
		return {
			top,
			ticks: Array.from({ length: Math.round(top / unit) + 1 }, (_, index) => index * unit)
		};
	});

	const band = $derived(Math.max(0, width - GUTTER - MARGIN) / columns.length);
	const bar = $derived(Math.max(1, Math.min(BAR, band - GAP)));
	const every = $derived(Math.max(1, Math.ceil(LABEL_SPACING / band)));
	const unit = $derived(span.step === 1 ? 'day' : span.step === 7 ? 'week' : '30 days');
	const hovered = $derived(active === null ? null : (columns[active] ?? null));

	function height(value: number): number {
		return (value / scale.top) * HEIGHT;
	}

	function segments(values: number[]) {
		const top = values.findLastIndex((value) => value > 0);
		let base = TOP + HEIGHT;
		return values.flatMap((value, index) => {
			if (value <= 0) return [];
			const full = height(value);
			const gap = index !== top && full > GAP * 2 ? GAP : 0;
			base -= full;
			return [
				{
					color: series[index].color,
					y: base + gap,
					height: full - gap,
					capped: index === top
				}
			];
		});
	}

	function capped(x: number, y: number, tall: number): string {
		const radius = Math.min(RADIUS, bar / 2, tall);
		const right = x + bar;
		const bottom = y + tall;
		return `M${x},${bottom}V${y + radius}Q${x},${y} ${x + radius},${y}H${right - radius}Q${right},${y} ${right},${y + radius}V${bottom}Z`;
	}

	function label({ start, end }: Column): string {
		if (span.step === 1) return dateOf(end, { month: 'short', day: 'numeric' });
		if (span.step === 7) return dateOf(start, { month: 'short', day: 'numeric' });
		return dateOf(start, { month: 'short', year: '2-digit' });
	}

	function period({ start, end }: Column): string {
		if (span.step === 1) {
			return dateOf(end, { weekday: 'short', month: 'short', day: 'numeric' });
		}
		const day: Intl.DateTimeFormatOptions = { month: 'short', day: 'numeric' };
		return `${dateOf(start, day)} to ${dateOf(end, { ...day, year: 'numeric' })}`;
	}

	function pointermove(event: PointerEvent & { currentTarget: SVGSVGElement }) {
		const x = event.clientX - event.currentTarget.getBoundingClientRect().left;
		const index = Math.floor((x - GUTTER) / band);
		active = index >= 0 && index < columns.length ? index : null;
	}

	function keydown(event: KeyboardEvent) {
		const move = { ArrowLeft: -1, ArrowRight: 1 }[event.key];
		if (!move) return;
		event.preventDefault();
		const from = active ?? columns.length - 1;
		active = Math.max(0, Math.min(columns.length - 1, from + move));
	}
</script>

<figure class="rounded-surface surface p-4">
	<figcaption class="flex items-start gap-3 pb-3">
		<span class="min-w-0 flex-1">
			<span class="block text-title font-medium">Tokens over time</span>
			<span class="block text-secondary">Input and output tokens per {unit}, by model</span>
		</span>
		<button
			class="icon-button text-secondary hover:text-window-text"
			onclick={() => (tabular = !tabular)}
			{@attach tooltip(tabular ? 'Show the chart' : 'Show as a table')}
		>
			{#if tabular}
				<ChartBarIcon size={16} />
			{:else}
				<TableIcon size={16} />
			{/if}
		</button>
	</figcaption>

	<div class="flex flex-wrap gap-x-4 gap-y-1 pb-3 text-label text-secondary">
		{#each series as entry (entry.slot)}
			<span class="flex items-center gap-1.5">
				<span class="size-2.5 rounded-[2px]" style:background-color={entry.color}></span>
				{entry.label}
			</span>
		{/each}
	</div>

	{#if tabular}
		<div class="overflow-auto" style:max-height="{TOP + HEIGHT + AXIS}px">
			<table class="w-full text-right tabular-nums">
				<thead class="sticky top-0 bg-base">
					<tr class="label">
						<th class="pr-3 pb-2 text-left font-medium">
							{span.step === 1 ? 'Day' : 'Period'}
						</th>
						{#each series as entry (entry.slot)}
							<th class="px-3 pb-2 font-medium">{entry.label}</th>
						{/each}
						<th class="pb-2 pl-3 font-medium">Total</th>
					</tr>
				</thead>
				<tbody>
					{#each columns.toReversed() as column (column.end)}
						<tr class="border-t border-light/50">
							<th class="py-1.5 pr-3 text-left font-normal whitespace-nowrap">
								{period(column)}
							</th>
							{#each column.values as value, index (index)}
								<td class="px-3 py-1.5 text-secondary">{whole(value)}</td>
							{/each}
							<td class="py-1.5 pl-3">{whole(column.total)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else}
		<div class="relative" bind:clientWidth={width}>
			<svg
				class="block outline-none focus-visible:rounded-input focus-visible:ring-1 focus-visible:ring-accent"
				{width}
				height={TOP + HEIGHT + AXIS}
				role="slider"
				tabindex="0"
				aria-label="Tokens per {unit}"
				aria-valuemin={0}
				aria-valuemax={columns.length - 1}
				aria-valuenow={active ?? columns.length - 1}
				aria-valuetext={hovered
					? `${period(hovered)}, ${whole(hovered.total)} tokens`
					: undefined}
				onpointermove={pointermove}
				onpointerleave={() => (active = null)}
				onfocus={() => (active ??= columns.length - 1)}
				onblur={() => (active = null)}
				onkeydown={keydown}
			>
				{#each scale.ticks as tick (tick)}
					{@const y = TOP + HEIGHT - height(tick)}
					<line
						class={tick === 0 ? 'stroke-light' : 'stroke-window-text/8'}
						x1={GUTTER}
						x2={width - MARGIN}
						y1={y}
						y2={y}
					/>
					<text
						class="fill-secondary text-label tabular-nums"
						x={GUTTER - 8}
						{y}
						text-anchor="end"
						dominant-baseline="middle"
					>
						{compact(tick)}
					</text>
				{/each}

				{#if active !== null}
					<rect
						class="fill-window-text/6"
						x={GUTTER + active * band}
						y={TOP}
						width={band}
						height={HEIGHT}
					/>
				{/if}

				{#each columns as column, index (column.end)}
					{@const x = GUTTER + index * band + (band - bar) / 2}
					{#each segments(column.values) as segment (segment.color)}
						{#if segment.capped}
							<path d={capped(x, segment.y, segment.height)} fill={segment.color} />
						{:else}
							<rect
								{x}
								y={segment.y}
								width={bar}
								height={segment.height}
								fill={segment.color}
							/>
						{/if}
					{/each}
					{#if (columns.length - 1 - index) % every === 0}
						<text
							class="fill-secondary text-label"
							x={GUTTER + (index + 0.5) * band}
							y={TOP + HEIGHT + 16}
							text-anchor="middle"
						>
							{label(column)}
						</text>
					{/if}
				{/each}
			</svg>

			{#if hovered && active !== null}
				{@const left = GUTTER + active * band}
				{@const flipped = left > width / 2}
				<ChartTooltip
					x={flipped ? width - left + 8 : left + band + 8}
					y={TOP}
					side={flipped ? 'left' : 'right'}
				>
					<p class="pb-1 text-label text-secondary">{period(hovered)}</p>
					{#each series as entry, index (entry.slot)}
						{#if hovered.values[index] > 0}
							<p class="flex items-center gap-2 leading-5">
								<span
									class="h-0.5 w-3 shrink-0 rounded-full"
									style:background-color={entry.color}
								></span>
								<span class="font-medium tabular-nums">
									{compact(hovered.values[index])}
								</span>
								<span class="truncate text-secondary">{entry.label}</span>
							</p>
						{/if}
					{/each}
					<p class="flex items-center gap-2 leading-5">
						<span class="w-3 shrink-0"></span>
						<span class="font-medium tabular-nums">{compact(hovered.total)}</span>
						<span class="text-secondary">Total</span>
					</p>
				</ChartTooltip>
			{/if}
		</div>
	{/if}
</figure>
