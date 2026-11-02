<script lang="ts">
	import { getMenu } from '$lib/popup';
	import type { Snippet } from 'svelte';

	let {
		onselect,
		danger = false,
		disabled = false,
		children
	}: {
		onselect: () => void;
		danger?: boolean;
		disabled?: boolean;
		children: Snippet;
	} = $props();

	const menu = getMenu();

	function select() {
		menu.close();
		onselect();
	}

	function hover(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		if (document.activeElement !== event.currentTarget) event.currentTarget.focus();
	}
</script>

<button
	type="button"
	class={['menu-item', danger && 'text-danger']}
	role="menuitem"
	tabindex="-1"
	{disabled}
	onclick={select}
	onpointermove={hover}
>
	{@render children()}
</button>
