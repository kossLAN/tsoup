<script lang="ts">
	import { answerQuestion, type Question } from '$lib/api';
	import { ArrowElbowDownLeftIcon } from '$lib/icons';

	let { session, question, asker }: { session: string; question: Question; asker: string } =
		$props();

	let text = $state('');
	let sending = $state(false);
	let failure = $state<string | null>(null);

	async function answer(content: string) {
		if (!content.trim() || sending) return;
		sending = true;
		failure = null;
		try {
			await answerQuestion(session, question.id, content.trim());
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
		sending = false;
	}

	function keydown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			answer(text);
		}
	}
</script>

<div class="flex flex-col gap-1.5 rounded-control surface p-2">
	<span class="text-label text-secondary">{asker}</span>
	<p class="wrap-break-word whitespace-pre-wrap">{question.text}</p>
	{#if question.options.length}
		<div class="flex flex-wrap gap-1.5">
			{#each question.options as option (option)}
				<button
					class="button h-auto min-h-6.5 bg-alternate-base px-2 py-1 text-left text-label enabled:hover:bg-window-text/8"
					disabled={sending}
					onclick={() => answer(option)}
				>
					{option}
				</button>
			{/each}
		</div>
	{/if}
	<div class="flex items-center gap-1.5">
		<input
			class="input"
			bind:value={text}
			onkeydown={keydown}
			placeholder="Answer"
			spellcheck="false"
		/>
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
