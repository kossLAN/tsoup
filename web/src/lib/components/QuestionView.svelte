<script lang="ts">
	import { answerQuestion, type Question } from '$lib/api';
	import { ArrowElbowDownLeftIcon } from '$lib/icons';
	import Markdown from './Markdown.svelte';

	let {
		session,
		question,
		asker,
		onanswered
	}: { session: string; question: Question; asker: string; onanswered: () => void } = $props();

	let text = $state('');
	let sending = $state(false);
	let failure = $state<string | null>(null);

	async function answer(content: string) {
		if (!content.trim() || sending) return;
		sending = true;
		failure = null;
		try {
			await answerQuestion(session, question.id, content.trim());
			onanswered();
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
		sending = false;
	}

	function keydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			answer(text);
		}
	}
</script>

<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3 pt-1 pb-3">
	<span class="label">{asker}</span>
	<Markdown text={question.text} />
	{#if question.options.length}
		<div class="flex flex-col gap-1">
			{#each question.options as option (option)}
				<button
					class="rounded-input surface px-2.5 py-1.5 text-left transition-colors enabled:hover:bg-alternate-base disabled:opacity-50"
					disabled={sending}
					onclick={() => answer(option)}
				>
					{option}
				</button>
			{/each}
		</div>
	{/if}
	<div
		class="flex items-end gap-1 rounded-control surface py-1 pr-1 pl-2.5 transition-colors focus-within:border-accent"
	>
		<textarea
			class="max-h-40 min-h-6.5 flex-1 resize-none py-1 outline-none placeholder:text-secondary"
			bind:value={text}
			onkeydown={keydown}
			rows="2"
			placeholder={question.options.length ? 'Or write your own answer' : 'Write your answer'}
		></textarea>
		<button
			class="icon-button"
			disabled={!text.trim() || sending}
			onclick={() => answer(text)}
			aria-label="Send answer"
		>
			<ArrowElbowDownLeftIcon size={14} />
		</button>
	</div>
	{#if failure}
		<span class="text-label text-danger">{failure}</span>
	{/if}
</div>
