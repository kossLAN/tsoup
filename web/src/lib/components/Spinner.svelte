<script lang="ts">
	import Boil from '$lib/spinners/Boil.svelte';
	import Simmer from '$lib/spinners/Simmer.svelte';
	import Slosh from '$lib/spinners/Slosh.svelte';
	import Stir from '$lib/spinners/Stir.svelte';

	const spinners = { slosh: Slosh, stir: Stir, simmer: Simmer, boil: Boil };

	let { size, kind = 'slosh' }: { size: number; kind?: keyof typeof spinners } = $props();

	const Kind = $derived(spinners[kind]);
</script>

<span class="inline-flex shrink-0" style:width="{size}px" style:height="{size}px">
	<Kind />
</span>
