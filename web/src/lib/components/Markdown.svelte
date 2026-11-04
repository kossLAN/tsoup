<script lang="ts">
	import { render } from '$lib/markdown';

	let { text, muted = false }: { text: string; muted?: boolean } = $props();

	const html = $derived(render(text));
</script>

<div class={['markdown', muted && 'muted']}>{@html html}</div>
