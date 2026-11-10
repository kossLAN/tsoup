<script lang="ts">
	import { render } from '$lib/markdown';

	let { text, muted = false }: { text: string; muted?: boolean } = $props();

	const html = $derived(render(text));

	async function click(event: MouseEvent) {
		const button = (event.target as Element).closest('button[data-copy]');
		const code = button?.closest('.code')?.querySelector('code');
		if (!button || !code) return;
		await navigator.clipboard.writeText(code.textContent ?? '');
		button.textContent = 'Copied';
		setTimeout(() => (button.textContent = 'Copy'), 1500);
	}
</script>

<div class={['markdown', muted && 'muted']} role="presentation" onclick={click}>{@html html}</div>
