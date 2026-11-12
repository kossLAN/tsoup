<script lang="ts">
	import {
		ago,
		compact,
		duration,
		modelId,
		percent,
		seriesColor,
		seriesOf,
		speed,
		startOf,
		today,
		tokens,
		utcOffset,
		whole
	} from '$lib/analytics';
	import { getAnalytics, type Analytics, type RequestTotals } from '$lib/api';
	import ActivityHeatmap from '$lib/components/ActivityHeatmap.svelte';
	import StatTile, { type Delta } from '$lib/components/StatTile.svelte';
	import TokenChart from '$lib/components/TokenChart.svelte';
	import { WarningCircleIcon } from '$lib/icons';
	import { sessions } from '$lib/sessions.svelte';

	type Ranked = { label: string; value: number };

	const ranges = [
		{ days: 7, label: '7 days' },
		{ days: 30, label: '30 days' },
		{ days: 90, label: '90 days' },
		{ days: null, label: 'All time' }
	];
	const SESSIONS = 8;
	const TOOLS = 10;

	let days = $state<number | null>(30);
	let view = $state<{ report: Analytics; since: number } | null>(null);
	let loading = $state(false);
	let failure = $state<string | null>(null);

	const since = $derived(days === null ? 0 : startOf(today() - days + 1));
	const titles = $derived(
		new Map(sessions.items.map((session) => [session.id, session.title || 'New session']))
	);

	$effect(() => {
		load(since);
	});

	async function load(from: number) {
		loading = true;
		failure = null;
		try {
			const report = await getAnalytics(from, utcOffset() / 60);
			if (from === since) view = { report, since: from };
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
		if (from === since) loading = false;
	}

	function rate(part: number, total: number): number {
		return total > 0 ? part / total : 0;
	}

	function change(
		current: number,
		previous: number | undefined,
		good?: boolean
	): Delta | undefined {
		if (!previous) return undefined;
		const ratio = current / previous - 1;
		if (Math.abs(ratio) < 0.005) return undefined;
		return {
			text: `${ratio > 0 ? '+' : ''}${whole(ratio * 100)}%`,
			rising: ratio > 0,
			good: good === undefined ? null : ratio > 0 === good
		};
	}

	function shift(
		current: number,
		previous: number | undefined,
		good: boolean
	): Delta | undefined {
		if (previous === undefined) return undefined;
		const points = (current - previous) * 100;
		if (Math.abs(points) < 0.5) return undefined;
		return {
			text: `${points > 0 ? '+' : ''}${Math.round(points)} pts`,
			rising: points > 0,
			good: points > 0 === good
		};
	}

	function kinds(totals: RequestTotals): Ranked[] {
		return [
			{ label: 'Fresh input', value: totals.input - totals.cache_read - totals.cache_write },
			{ label: 'Cached input', value: totals.cache_read },
			{ label: 'Cache writes', value: totals.cache_write },
			{ label: 'Output', value: totals.output - totals.reasoning },
			{ label: 'Reasoning', value: totals.reasoning }
		];
	}

	function uses(rows: Analytics['kinds']): Ranked[] {
		const sums = new Map<string, number>();
		for (const row of rows) {
			const label =
				row.purpose === 'compaction'
					? 'Compaction'
					: row.purpose === 'title'
						? 'Titles'
						: row.subagent
							? 'Subagents'
							: 'Main agent';
			sums.set(label, (sums.get(label) ?? 0) + tokens(row));
		}
		return [...sums].map(([label, value]) => ({ label, value }));
	}

	function ranked(rows: Ranked[]): Ranked[] {
		return rows
			.filter(({ value }) => value > 0)
			.sort((left, right) => right.value - left.value);
	}

	function tool(name: string): string {
		const mcp = name.match(/^mcp__(.+?)__(.+)$/);
		return mcp ? `${mcp[1]} ${mcp[2]}` : name;
	}
</script>

{#snippet bar(share: number, color: string)}
	<span class="block h-2 min-w-0 flex-1">
		<span
			class="block h-full min-w-0.5 rounded-r-small"
			style:width="{share * 100}%"
			style:background-color={color}
		></span>
	</span>
{/snippet}

{#snippet breakdown(title: string, rows: Ranked[])}
	{@const total = rows.reduce((sum, row) => sum + row.value, 0)}
	{@const peak = Math.max(1, ...rows.map((row) => row.value))}
	<div class="flex flex-col gap-2">
		<span class="label">{title}</span>
		{#each rows as row (row.label)}
			<div class="flex items-center gap-3">
				<span class="w-24 shrink-0 truncate">{row.label}</span>
				{@render bar(row.value / peak, 'var(--series-1)')}
				<span class="w-12 shrink-0 text-right tabular-nums">{compact(row.value)}</span>
				<span class="w-10 shrink-0 text-right text-secondary tabular-nums">
					{percent(row.value, total)}
				</span>
			</div>
		{/each}
	</div>
{/snippet}

{#snippet card(title: string, description: string)}
	<div class="pb-3">
		<h2 class="text-title font-medium">{title}</h2>
		<p class="text-secondary">{description}</p>
	</div>
{/snippet}

<div class="flex min-w-0 flex-1 flex-col">
	<header class="toolbar pr-6 pl-11 text-title font-medium group-data-[sidebar=true]/shell:pl-6">
		Analytics
	</header>
	<div class="min-h-0 flex-1 overflow-y-auto">
		<div class="mx-auto flex w-full max-w-280 flex-col gap-4 px-6 pt-2 pb-8">
			<div class="flex flex-wrap items-center gap-3">
				<div
					class="flex rounded-control surface p-0.5"
					role="radiogroup"
					aria-label="Period"
				>
					{#each ranges as range (range.label)}
						<button
							class={[
								'button h-6.5 px-2.5',
								days === range.days
									? 'bg-window-text/10'
									: 'text-secondary hover:text-window-text'
							]}
							role="radio"
							aria-checked={days === range.days}
							onclick={() => (days = range.days)}
						>
							{range.label}
						</button>
					{/each}
				</div>
				{#if view?.report.previous && days !== null}
					<span class="text-label text-secondary">
						Changes compare with the {days} days before
					</span>
				{/if}
			</div>

			{#if failure}
				<div class="alert whitespace-pre-wrap">
					<WarningCircleIcon size={14} />{failure}
				</div>
			{/if}

			{#if view}
				{@const report = view.report}
				{@const totals = report.totals.requests}
				{@const before = report.previous?.requests}
				{@const calls = report.totals.tools}
				{@const pace = speed(totals)}
				{@const paceBefore = before ? speed(before) : null}
				<div
					class={[
						'flex flex-col gap-4 transition-opacity',
						loading && 'pointer-events-none opacity-50'
					]}
				>
					{#if totals.requests === 0}
						<div class="flex flex-col items-center gap-1 py-32 text-center">
							<span class="text-title font-medium">No usage in this period</span>
							<span class="text-secondary">
								Requests, tokens and tool calls show up here as sessions run
							</span>
						</div>
					{:else}
						<div class="grid grid-cols-2 gap-3 lg:grid-cols-3">
							<StatTile
								label="Tokens"
								value={compact(tokens(totals))}
								hint="{compact(totals.input)} in, {compact(totals.output)} out"
								delta={change(tokens(totals), before && tokens(before))}
							/>
							<StatTile
								label="Requests"
								value={whole(totals.requests)}
								hint="{percent(totals.failed, totals.requests)} failed"
								delta={change(totals.requests, before?.requests)}
							/>
							<StatTile
								label="Sessions"
								value={whole(report.totals.sessions)}
								hint="{compact(
									tokens(totals) / report.totals.sessions
								)} tokens each"
								delta={change(report.totals.sessions, report.previous?.sessions)}
							/>
							<StatTile
								label="Cached input"
								value={percent(totals.cache_read, totals.input)}
								hint="{compact(totals.cache_read)} tokens reused"
								delta={shift(
									rate(totals.cache_read, totals.input),
									before && rate(before.cache_read, before.input),
									true
								)}
							/>
							<StatTile
								label="Output speed"
								value={pace === null ? 'None' : `${whole(pace)} tok/s`}
								hint="{duration(
									totals.duration_ms /
										Math.max(1, totals.requests - totals.failed)
								)} a request"
								delta={pace !== null && paceBefore
									? change(pace, paceBefore, true)
									: undefined}
							/>
							<StatTile
								label="Tool calls"
								value={whole(calls.calls)}
								hint="{percent(calls.failed, calls.calls)} failed"
								delta={change(calls.calls, report.previous?.tools.calls)}
							/>
						</div>

						<TokenChart {report} since={view.since} />

						<section class="rounded-surface surface p-4">
							{@render card(
								'Models',
								'What each model was asked for and how it answered'
							)}
							<div class="overflow-x-auto">
								<table class="w-full text-right whitespace-nowrap tabular-nums">
									<thead>
										<tr class="label">
											<th class="pb-2 text-left font-medium">Model</th>
											<th class="px-3 pb-2 font-medium">Requests</th>
											<th class="px-3 pb-2 font-medium">Input</th>
											<th class="px-3 pb-2 font-medium">Output</th>
											<th class="px-3 pb-2 font-medium">Cached</th>
											<th class="px-3 pb-2 font-medium">Speed</th>
											<th class="px-3 pb-2 font-medium">Failed</th>
											<th class="w-56 pb-2 pl-3 text-left font-medium">
												Share of tokens
											</th>
										</tr>
									</thead>
									<tbody>
										{#each report.models as model (modelId(model))}
											{@const color = seriesColor(
												seriesOf(report.ranking, model)
											)}
											{@const fast = speed(model)}
											<tr class="border-t border-light/50">
												<td class="py-2 text-left">
													<span class="flex items-center gap-2">
														<span
															class="size-2.5 shrink-0 rounded-[2px]"
															style:background-color={color}
														></span>
														<span class="truncate">{model.model}</span>
														<span class="truncate text-secondary">
															{model.provider}
														</span>
													</span>
												</td>
												<td class="px-3">{whole(model.requests)}</td>
												<td class="px-3">{compact(model.input)}</td>
												<td class="px-3">{compact(model.output)}</td>
												<td class="px-3">
													{percent(model.cache_read, model.input)}
												</td>
												<td class="px-3">
													{fast === null ? '' : `${whole(fast)} tok/s`}
												</td>
												<td
													class={[
														'px-3',
														!model.failed && 'text-secondary'
													]}
												>
													{whole(model.failed)}
												</td>
												<td class="pl-3">
													<span class="flex items-center gap-2">
														{@render bar(
															rate(tokens(model), tokens(totals)),
															color
														)}
														<span
															class="w-10 text-right text-secondary"
														>
															{percent(tokens(model), tokens(totals))}
														</span>
													</span>
												</td>
											</tr>
										{/each}
									</tbody>
								</table>
							</div>
						</section>

						<div class="grid gap-4 xl:grid-cols-2">
							<ActivityHeatmap hours={report.hours} />

							<section class="rounded-surface surface p-4">
								{@render card(
									'Where tokens go',
									'What they were and what spent them'
								)}
								<div class="flex flex-col gap-5">
									{@render breakdown('By kind', ranked(kinds(totals)))}
									{@render breakdown('By use', ranked(uses(report.kinds)))}
								</div>
							</section>
						</div>

						<div class="grid gap-4 xl:grid-cols-2">
							<section class="rounded-surface surface p-4">
								{@render card('Sessions', 'The sessions that used the most tokens')}
								<table class="w-full table-fixed text-right tabular-nums">
									<thead>
										<tr class="label">
											<th class="pb-2 text-left font-medium">Session</th>
											<th class="w-20 px-3 pb-2 font-medium">Requests</th>
											<th class="w-40 pb-2 pl-3 text-left font-medium"
												>Tokens</th
											>
											<th class="w-20 pb-2 pl-3 font-medium">Active</th>
										</tr>
									</thead>
									<tbody>
										{#each report.sessions.slice(0, SESSIONS) as session (session.session)}
											{@const title = titles.get(session.session)}
											<tr class="border-t border-light/50">
												<td class="truncate py-2 text-left">
													{#if title}
														<a
															class="hover:text-accent"
															href="/sessions/{session.session}"
														>
															{title}
														</a>
													{:else}
														<span class="text-secondary"
															>Deleted session</span
														>
													{/if}
												</td>
												<td class="px-3">{whole(session.requests)}</td>
												<td class="pl-3">
													<span class="flex items-center gap-2">
														{@render bar(
															rate(
																tokens(session),
																tokens(report.sessions[0])
															),
															'var(--series-1)'
														)}
														<span class="w-12 text-right">
															{compact(tokens(session))}
														</span>
													</span>
												</td>
												<td class="pl-3 text-secondary"
													>{ago(session.last)}</td
												>
											</tr>
										{/each}
									</tbody>
								</table>
							</section>

							<section class="rounded-surface surface p-4">
								{@render card(
									'Tools',
									'What agents called, how often it failed and how long it took'
								)}
								<table class="w-full table-fixed text-right tabular-nums">
									<thead>
										<tr class="label">
											<th class="pb-2 text-left font-medium">Tool</th>
											<th class="w-40 pb-2 pl-3 text-left font-medium"
												>Calls</th
											>
											<th class="w-16 pb-2 pl-3 font-medium">Failed</th>
											<th class="w-20 pb-2 pl-3 font-medium">Average</th>
										</tr>
									</thead>
									<tbody>
										{#each report.tools.slice(0, TOOLS) as row (row.name)}
											<tr class="border-t border-light/50">
												<td class="truncate py-2 text-left"
													>{tool(row.name)}</td
												>
												<td class="pl-3">
													<span class="flex items-center gap-2">
														{@render bar(
															rate(row.calls, report.tools[0].calls),
															'var(--series-1)'
														)}
														<span class="w-12 text-right">
															{whole(row.calls)}
														</span>
													</span>
												</td>
												<td
													class={[
														'pl-3',
														!row.failed && 'text-secondary'
													]}
												>
													{percent(row.failed, row.calls)}
												</td>
												<td class="pl-3 text-secondary">
													{duration(row.duration_ms / row.calls)}
												</td>
											</tr>
										{/each}
									</tbody>
								</table>
							</section>
						</div>
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>
