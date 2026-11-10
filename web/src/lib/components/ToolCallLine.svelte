<script lang="ts">
	import type { Entry, ToolCall } from '$lib/api';
	import { CaretRightIcon, WarningCircleIcon } from '$lib/icons';
	import Diff from './Diff.svelte';
	import Spinner from './Spinner.svelte';

	type ToolResult = Extract<Entry, { kind: 'tool' }>;

	let {
		call,
		result,
		pending
	}: { call: ToolCall; result: ToolResult | undefined; pending: boolean } = $props();

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
		spawn_agent: ['running agent', 'ran agent'],
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

	const target = $derived.by(() => {
		if (typeof args.command === 'string') return args.command;
		if (typeof args.path === 'string') return args.path;
		if (typeof args.url === 'string') return args.url;
		if (typeof args.port === 'number') return `port ${args.port}`;
		if (typeof args.name === 'string') return args.name;
		if (typeof args.question === 'string') return args.question;
		return '';
	});

	const detail = $derived.by(() => {
		if (call.function.name === 'create_vm' && typeof args.module === 'string') {
			return args.module;
		}
		if (call.function.name === 'shell' && typeof args.command === 'string') return args.command;
		if (typeof args.prompt === 'string') return args.prompt;
		if (typeof args.question === 'string') return args.question;
		return null;
	});
</script>

{#snippet output(text: string, error = false)}
	<pre
		class={[
			'max-h-90 overflow-y-auto rounded-control surface px-3 py-2 text-label/normal',
			'wrap-break-word whitespace-pre-wrap',
			error ? 'text-danger' : 'text-secondary'
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

	{#if expanded}
		{#if result?.diff}
			<div class="max-h-105 overflow-y-auto rounded-control surface">
				<Diff diff={result.diff} />
			</div>
		{:else}
			{#if detail}
				{@render output(detail)}
			{/if}
			{#if result}
				{@render output(result.output, result.error)}
			{:else if !pending}
				{@render output('interrupted')}
			{/if}
		{/if}
	{/if}
</div>
