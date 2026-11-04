<script lang="ts">
	let {
		checked,
		label,
		onchange
	}: { checked: boolean; label: string; onchange: (checked: boolean) => void } = $props();
</script>

<button
	type="button"
	role="switch"
	aria-checked={checked}
	aria-label={label}
	class={[
		'relative inline-flex h-5 w-8.5 shrink-0 items-center rounded-full transition-colors',
		checked ? 'bg-accent' : 'bg-light'
	]}
	onclick={() => onchange(!checked)}
>
	<span
		class={[
			'size-4 rounded-full bg-window-text shadow-sm transition-transform',
			checked ? 'translate-x-4' : 'translate-x-0.5'
		]}
	></span>
</button>
