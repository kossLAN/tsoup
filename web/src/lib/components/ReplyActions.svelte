<script lang="ts">
	import { ArrowsClockwiseIcon, CheckIcon, CopyIcon } from '$lib/icons';
	import { tooltip } from '$lib/popup';

	let { text, onregenerate }: { text: string; onregenerate?: () => void } = $props();

	const button =
		'inline-flex size-6 items-center justify-center rounded-input transition-colors hover:bg-window-text/8 hover:text-window-text';

	let copied = $state(false);

	async function copy() {
		await navigator.clipboard.writeText(text);
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}
</script>

<div class="-mt-5.5 -ml-1.5 flex items-center self-start text-secondary">
	<button class={button} onclick={copy} {@attach tooltip('Copy')}>
		{#if copied}
			<CheckIcon size={14} />
		{:else}
			<CopyIcon size={14} />
		{/if}
	</button>
	{#if onregenerate}
		<button class={button} onclick={onregenerate} {@attach tooltip('Regenerate')}>
			<ArrowsClockwiseIcon size={14} />
		</button>
	{/if}
</div>
