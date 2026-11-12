<script lang="ts">
	import { compact, tokens, whole } from '$lib/analytics';
	import type { Analytics } from '$lib/api';
	import ChartTooltip from './ChartTooltip.svelte';

	type Cell = { weekday: number; hour: number; requests: number; tokens: number };

	let { hours }: { hours: Analytics['hours'] } = $props();

	const LEVELS = 5;
	const weekdays = [
		{ index: 1, name: 'Mon' },
		{ index: 2, name: 'Tue' },
		{ index: 3, name: 'Wed' },
		{ index: 4, name: 'Thu' },
		{ index: 5, name: 'Fri' },
		{ index: 6, name: 'Sat' },
		{ index: 0, name: 'Sun' }
	];
	const marks = [0, 3, 6, 9, 12, 15, 18, 21];

	let root: HTMLElement;
	let active = $state<{ cell: Cell; day: string; x: number; y: number; flipped: boolean } | null>(
		null
	);

	const grid = $derived(
		weekdays.map(({ index, name }) => ({
			name,
			cells: Array.from({ length: 24 }, (_, hour): Cell => {
				const row = hours.find((row) => row.weekday === index && row.hour === hour);
				return {
					weekday: index,
					hour,
					requests: row?.requests ?? 0,
					tokens: row ? tokens(row) : 0
				};
			})
		}))
	);
	const peak = $derived(Math.max(1, ...hours.map(({ requests }) => requests)));

	function level(requests: number): number {
		return requests > 0 ? Math.ceil((requests / peak) * LEVELS) : 0;
	}

	function time(hour: number): string {
		return `${String(hour).padStart(2, '0')}:00`;
	}

	function describe(day: string, cell: Cell): string {
		return `${day} ${time(cell.hour)}, ${whole(cell.requests)} requests`;
	}

	function enter(event: PointerEvent & { currentTarget: HTMLElement }, day: string, cell: Cell) {
		const frame = root.getBoundingClientRect();
		const box = event.currentTarget.getBoundingClientRect();
		const flipped = box.left - frame.left > frame.width / 2;
		active = {
			cell,
			day,
			flipped,
			x: flipped ? frame.right - box.left + 6 : box.right - frame.left + 6,
			y: box.top - frame.top
		};
	}
</script>

<figure class="rounded-surface surface p-4">
	<figcaption class="pb-3">
		<span class="block text-title font-medium">Activity</span>
		<span class="block text-secondary">Requests by weekday and hour</span>
	</figcaption>

	<div class="relative" bind:this={root}>
		<table class="w-full table-fixed border-separate border-spacing-0.5">
			<thead>
				<tr class="text-left text-label text-secondary">
					<th class="w-9"></th>
					{#each marks as hour (hour)}
						<th class="pb-0.5 font-normal tabular-nums" colspan="3" scope="col">
							{String(hour).padStart(2, '0')}
						</th>
					{/each}
				</tr>
			</thead>
			<tbody onpointerleave={() => (active = null)}>
				{#each grid as { name, cells } (name)}
					<tr>
						<th class="text-left text-label font-normal text-secondary" scope="row">
							{name}
						</th>
						{#each cells as cell (cell.hour)}
							{@const step = level(cell.requests)}
							<td
								class={[
									'h-6.5 rounded-[3px]',
									step === 0 && 'bg-window-text/5',
									active?.cell === cell && 'outline outline-window-text'
								]}
								style:background-color={step ? `var(--heat-${step})` : undefined}
								aria-label={describe(name, cell)}
								onpointerenter={(event) => enter(event, name, cell)}
							></td>
						{/each}
					</tr>
				{/each}
			</tbody>
		</table>

		{#if active}
			<ChartTooltip x={active.x} y={active.y} side={active.flipped ? 'left' : 'right'}>
				<p class="pb-1 text-label text-secondary">
					{active.day}
					{time(active.cell.hour)}
				</p>
				<p class="flex gap-2 leading-5">
					<span class="font-medium tabular-nums">{whole(active.cell.requests)}</span>
					<span class="text-secondary">requests</span>
				</p>
				<p class="flex gap-2 leading-5">
					<span class="font-medium tabular-nums">{compact(active.cell.tokens)}</span>
					<span class="text-secondary">tokens</span>
				</p>
			</ChartTooltip>
		{/if}
	</div>

	<div class="flex items-center justify-end gap-1.5 pt-3 text-label text-secondary">
		Less
		<span class="size-3 rounded-[3px] bg-window-text/5"></span>
		{#each { length: LEVELS }, step (step)}
			<span class="size-3 rounded-[3px]" style:background-color="var(--heat-{step + 1})"
			></span>
		{/each}
		More
	</div>
</figure>
