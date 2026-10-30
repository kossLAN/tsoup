<script lang="ts">
	import { ArrowElbowDownLeftIcon, StopIcon } from '$lib/icons';

	let {
		busy = false,
		onsend,
		onstop
	}: {
		busy?: boolean;
		onsend: (text: string) => Promise<boolean>;
		onstop?: () => void;
	} = $props();

	let text = $state('');
	let sending = $state(false);
	let input: HTMLTextAreaElement;

	$effect(() => {
		void text;
		input.style.height = 'auto';
		input.style.height = `${input.scrollHeight}px`;
	});

	async function send() {
		const content = text.trim();
		if (!content || busy || sending) return;
		sending = true;
		text = '';
		if (!(await onsend(content)) && !text) {
			text = content;
		}
		sending = false;
	}

	function keydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			send();
		}
	}
</script>

<div class="mx-auto w-full max-w-chat px-6 pb-5">
	<div
		class="flex items-end gap-2 rounded-surface surface py-1.5 pr-1.5 pl-3.5 transition-colors focus-within:border-accent"
	>
		<textarea
			class="max-h-60 flex-1 resize-none overflow-y-auto py-0.75 text-title/5 outline-none placeholder:text-secondary"
			bind:this={input}
			bind:value={text}
			onkeydown={keydown}
			rows="1"
			placeholder="Ask the agent"></textarea>
		{#if busy && onstop}
			<button class="icon-button" title="Stop" onclick={onstop}>
				<StopIcon size={16} />
			</button>
		{:else}
			<button
				class="icon-button"
				title="Send"
				disabled={!text.trim() || sending}
				onclick={send}
			>
				<ArrowElbowDownLeftIcon size={16} />
			</button>
		{/if}
	</div>
</div>
