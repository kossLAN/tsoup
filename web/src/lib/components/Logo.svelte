<script lang="ts">
	let {
		size,
		weight = 3,
		class: className
	}: {
		size: number | string;
		weight?: number;
		class?: string;
	} = $props();

	const id = $props.id();

	const outer = $derived(16 - weight / 2);
	const inner = $derived(outer - weight / 2 - (1 + weight / 4));
</script>

<svg class={className} width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
	<clipPath {id}><circle cx="16" cy="16" r={inner} /></clipPath>
	<circle cx="16" cy="16" r={outer} fill="none" stroke="currentColor" stroke-width={weight} />
	<path
		class="fill-accent"
		clip-path="url(#{id})"
		d="M-6 16.5 q5.5 2.4 11 0 t11 0 t11 0 t11 0 V32 H-6 Z"
	/>
</svg>
