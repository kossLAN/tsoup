<script lang="ts">
	import { PaperPlaneRightIcon, StopIcon } from '$lib/icons';

	let {
		busy,
		onsend,
		onstop
	}: { busy: boolean; onsend: (text: string) => void; onstop: () => void } = $props();

	let text = $state('');
	let input: HTMLTextAreaElement;

	$effect(() => {
		void text;
		input.style.height = 'auto';
		input.style.height = `${input.scrollHeight}px`;
	});

	function send() {
		const content = text.trim();
		if (!content || busy) return;
		text = '';
		onsend(content);
	}

	function keydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			send();
		}
	}
</script>

<div class="composer">
	<div class="field">
		<textarea
			bind:this={input}
			bind:value={text}
			onkeydown={keydown}
			rows="1"
			placeholder="Ask the agent"></textarea>
		{#if busy}
			<button class="icon-button stop" title="Stop" onclick={onstop}>
				<StopIcon size={14} weight="fill" />
			</button>
		{:else}
			<button class="icon-button send" title="Send" disabled={!text.trim()} onclick={send}>
				<PaperPlaneRightIcon size={14} weight="fill" />
			</button>
		{/if}
	</div>
</div>

<style>
	.composer {
		width: 100%;
		max-width: 820px;
		margin: 0 auto;
		padding: 0 24px 20px;
	}

	.field {
		display: flex;
		align-items: flex-end;
		gap: 8px;
		padding: 6px 6px 6px 14px;
		background: var(--base);
		border: 1px solid var(--light);
		border-radius: var(--radius-surface);
		transition: border-color var(--duration) var(--ease-out-cubic);
	}

	.field:focus-within {
		border-color: var(--accent);
	}

	textarea {
		flex: 1;
		max-height: 240px;
		padding: 3px 0;
		resize: none;
		border: none;
		outline: none;
		background: transparent;
		font-size: var(--font-title);
		line-height: 20px;
		overflow-y: auto;
	}

	textarea::placeholder {
		color: var(--text-secondary);
	}

	.send {
		background: var(--accent);
		color: var(--highlighted-text);
	}

	.send:hover {
		background: var(--accent);
		filter: brightness(1.1);
	}

	.send:disabled:hover {
		background: var(--accent);
		filter: none;
	}

	.stop {
		background: var(--danger-tint);
		color: var(--danger);
	}
</style>
