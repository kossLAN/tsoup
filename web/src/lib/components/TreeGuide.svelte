<script lang="ts">
	let { trail = [], last }: { trail?: boolean[]; last: boolean } = $props();
</script>

{#each trail as open, level (level)}
	<span class={['ml-1.75 w-1.75 shrink-0 self-stretch', open && 'border-l border-light']}></span>
{/each}
<span class={['ml-1.75 w-1.75 shrink-0 self-stretch', !last && 'border-l border-light']}>
	<span class={['block h-1/2 border-b border-light', last && 'rounded-bl-small border-l']}></span>
</span>
