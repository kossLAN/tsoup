<script lang="ts">
	import type { AgentSummary, Entry, ToolCall } from '$lib/api';
	import { CaretRightIcon, WarningCircleIcon } from '$lib/icons';
	import type { ClassValue } from 'svelte/elements';
	import Diff from './Diff.svelte';
	import Spinner from './Spinner.svelte';

	type ToolResult = Extract<Entry, { kind: 'tool' }>;

	let {
		call,
		result,
		pending,
		agents
	}: {
		call: ToolCall;
		result: ToolResult | undefined;
		pending: boolean;
		agents: AgentSummary[];
	} = $props();

	let open = $state<boolean | null>(null);

	const expanded = $derived(open ?? Boolean(result?.diff));

	const labels: Record<string, [string, string]> = {
		shell: ['running', 'ran'],
		read_file: ['reading', 'read'],
		write_file: ['writing', 'wrote'],
		edit_file: ['editing', 'edited'],
		create_vm: ['creating a vm', 'created a vm'],
		run_vm: ['starting the vm', 'started the vm'],
		destroy_vm: ['destroying the vm', 'destroyed the vm'],
		show_preview: ['showing a preview', 'showed a preview'],
		set_actions: ['setting actions', 'set actions'],
		web_search: ['searching', 'searched'],
		web_fetch: ['fetching', 'fetched'],
		spawn_agent: ['starting agent', 'started agent'],
		resume_agent: ['resuming agent', 'resumed agent'],
		list_agents: ['listing agents', 'listed agents'],
		wait_agents: ['waiting for agents', 'waited for agents'],
		ask: ['asking', 'asked']
	};

	const running = $derived(!result && pending);

	const mcp = $derived(call.function.name.match(/^mcp__(.+?)__(.+)$/));

	const label = $derived.by(() => {
		if (mcp) return `${mcp[1]} ${mcp[2]}`;
		const forms = labels[call.function.name];
		if (!forms) return call.function.name;
		return running ? forms[0] : forms[1];
	});

	const args = $derived.by((): Record<string, unknown> => {
		try {
			return JSON.parse(call.function.arguments || '{}');
		} catch {
			return {};
		}
	});

	const questions = $derived(
		Array.isArray(args.questions)
			? args.questions.flatMap((asked) =>
					typeof asked?.question === 'string' ? [asked.question as string] : []
				)
			: []
	);

	const agent = $derived(agents.find(({ id }) => id === args.agent));

	const awaited = $derived.by(() => {
		const ids = args.agents;
		return Array.isArray(ids) ? agents.filter(({ id }) => ids.includes(id)) : [];
	});

	const target = $derived.by(() => {
		if (typeof args.command === 'string') return args.command;
		if (typeof args.path === 'string') return args.path;
		if (typeof args.url === 'string') return args.url;
		if (typeof args.query === 'string') return args.query;
		if (typeof args.port === 'number') return `port ${args.port}`;
		if (typeof args.name === 'string') return args.name;
		if (agent) return agent.name;
		if (awaited.length) return awaited.map(({ name }) => name).join(', ');
		if (questions.length === 1) return questions[0];
		if (questions.length) return `${questions.length} questions`;
		return '';
	});

	const detail = $derived.by(() => {
		if (call.function.name === 'create_vm' && typeof args.module === 'string') {
			return args.module;
		}
		if (call.function.name === 'shell' && typeof args.command === 'string') return args.command;
		if (typeof args.prompt === 'string') return args.prompt;
		if (questions.length) return questions.join('\n');
		return null;
	});
</script>

{#snippet block(text: string, style: ClassValue)}
	<pre
		class={[
			'overflow-y-auto text-label/normal wrap-break-word whitespace-pre-wrap',
			style
		]}>{text}</pre>
{/snippet}

<div class="flex min-w-0 flex-col gap-1">
	<button
		class="group flex h-5.5 max-w-full items-center gap-1.5 text-secondary transition-colors hover:text-window-text"
		onclick={() => (open = !expanded)}
	>
		<span class="shrink-0">{label}</span>
		{#if target}
			<span class="truncate font-mono text-label">{target}</span>
		{/if}
		{#if running}
			<Spinner size={12} kind="stir" />
		{:else if result?.error}
			<span class="inline-flex shrink-0"><WarningCircleIcon size={12} /></span>
		{/if}
		<span
			class={[
				'inline-flex shrink-0 transition',
				expanded ? 'rotate-90' : 'opacity-0 group-hover:opacity-100'
			]}
		>
			<CaretRightIcon size={10} />
		</span>
	</button>

	{#if result?.images?.length}
		<div class="flex flex-wrap gap-1.5">
			{#each result.images as image, position (position)}
				<img
					class="max-h-72 max-w-full rounded-control border border-light"
					src="data:{image.media_type};base64,{image.data}"
					alt="{label} result {position + 1}"
				/>
			{/each}
		</div>
	{/if}

	{#if expanded}
		{#if result?.diff}
			<div class="max-h-105 overflow-y-auto rounded-control surface">
				<Diff diff={result.diff} />
			</div>
		{:else if detail || result || !pending}
			<div class="flex flex-col gap-1.5 rounded-control surface p-1.5">
				{#if detail}
					{@render block(
						detail,
						'max-h-60 rounded-input bg-alternate-base px-2 py-1.5 text-window-text'
					)}
				{/if}
				{#if result}
					{@render block(result.output, [
						'max-h-90 px-1.5 py-0.5',
						result.error ? 'text-danger' : 'text-secondary'
					])}
				{:else if !pending}
					{@render block('interrupted', 'px-1.5 py-0.5 text-secondary')}
				{/if}
			</div>
		{/if}
	{/if}
</div>
