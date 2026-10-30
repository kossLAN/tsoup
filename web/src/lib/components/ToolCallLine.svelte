<script lang="ts">
	import type { Entry, ToolCall } from '$lib/api';
	import { CaretRightIcon, CircleNotchIcon, WarningCircleIcon } from '$lib/icons';
	import Diff from './Diff.svelte';

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
		show_website: ['showing the website', 'showed the website']
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
		return '';
	});

	const detail = $derived.by(() => {
		if (call.function.name === 'create_vm' && typeof args.module === 'string') {
			return args.module;
		}
		if (call.function.name === 'shell' && typeof args.command === 'string') return args.command;
		return null;
	});
</script>

<div class="call">
	<button class="line" class:open={expanded} onclick={() => (open = !expanded)}>
		<span class="label">{label}</span>
		{#if target}
			<span class="target">{target}</span>
		{/if}
		{#if running}
			<span class="spinner"><CircleNotchIcon size={12} /></span>
		{:else if result?.error}
			<span class="failed"><WarningCircleIcon size={12} /></span>
		{/if}
		<span class="caret"><CaretRightIcon size={10} /></span>
	</button>

	{#if expanded}
		{#if result?.diff}
			<div class="block diff">
				<Diff diff={result.diff} />
			</div>
		{:else}
			{#if detail}
				<pre class="block">{detail}</pre>
			{/if}
			{#if result}
				<pre class="block" class:error={result.error}>{result.output}</pre>
			{:else if !pending}
				<pre class="block">interrupted</pre>
			{/if}
		{/if}
	{/if}
</div>

<style>
	.call {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}

	.line {
		justify-content: flex-start;
		gap: 6px;
		max-width: 100%;
		height: 22px;
		padding: 0;
		border-radius: 0;
		color: var(--text-secondary);
		transition: color var(--duration-fast) linear;
	}

	.line:hover {
		background: transparent;
		color: var(--window-text);
	}

	.label {
		flex-shrink: 0;
		font-weight: 400;
	}

	.target {
		min-width: 0;
		font-family: var(--font-mono);
		font-size: var(--font-label);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.spinner,
	.failed,
	.caret {
		display: inline-flex;
		flex-shrink: 0;
	}

	.spinner {
		color: var(--accent);
		animation: spin 1s linear infinite;
	}

	.failed {
		color: var(--danger);
	}

	.caret {
		opacity: 0;
		transition:
			opacity var(--duration-fast) linear,
			transform var(--duration-fast) linear;
	}

	.line:hover .caret,
	.line.open .caret {
		opacity: 1;
	}

	.line.open .caret {
		transform: rotate(90deg);
	}

	.block {
		margin: 0;
		background: var(--base);
		border: 1px solid var(--light);
		border-radius: var(--radius-control);
		overflow: hidden;
	}

	pre.block {
		padding: 8px 12px;
		font-size: var(--font-label);
		line-height: 1.5;
		color: var(--text-secondary);
		white-space: pre-wrap;
		word-break: break-word;
		max-height: 360px;
		overflow-y: auto;
	}

	pre.block.error {
		color: var(--danger);
	}

	.diff {
		max-height: 420px;
		overflow-y: auto;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
