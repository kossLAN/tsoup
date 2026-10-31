<script lang="ts">
	import { CaretRightIcon } from '$lib/icons';
	import { follow, getMenu, items, navigate, setMenu } from '$lib/popup';
	import type { Snippet } from 'svelte';

	let { label, children }: { label: Snippet; children: Snippet } = $props();

	setMenu(getMenu());

	const id = $props.id();

	const OPEN_DELAY = 80;
	const CLOSE_DELAY = 200;

	let trigger: HTMLButtonElement;
	let menu: HTMLElement;
	let open = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let stop: (() => void) | undefined;

	function show(focus: boolean) {
		clearTimeout(timer);
		if (!menu.matches(':popover-open')) {
			menu.showPopover();
			stop = follow(trigger, menu, 'right-start', { gap: 2, skid: -5 });
		}
		if (focus) (items(menu)[0] ?? menu).focus();
	}

	function hide() {
		clearTimeout(timer);
		if (menu.matches(':popover-open')) menu.hidePopover();
	}

	function later(action: () => void, delay: number) {
		clearTimeout(timer);
		timer = setTimeout(action, delay);
	}

	function beforetoggle(event: ToggleEvent) {
		open = event.newState === 'open';
		if (!open) {
			stop?.();
			stop = undefined;
		}
	}

	function triggerkey(event: KeyboardEvent) {
		if (event.key === 'ArrowRight') {
			event.preventDefault();
			show(true);
		} else if (['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) {
			hide();
		}
	}

	function menukey(event: KeyboardEvent) {
		if (
			event.key === 'ArrowLeft' &&
			(event.target as Element).closest('[role="menu"]') === menu
		) {
			event.preventDefault();
			hide();
			trigger.focus();
			return;
		}
		navigate(event, menu);
	}

	function hover(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		if (document.activeElement !== event.currentTarget) event.currentTarget.focus();
	}

	function pointerleave() {
		if (menu.matches(':popover-open')) menu.focus();
	}

	$effect(() => {
		const parent = trigger.parentElement?.closest('[role="menu"]');
		if (!parent) return;

		function over(event: Event) {
			const item = (event.target as Element).closest('[role="menuitem"]');
			if (open && item && item !== trigger && item.closest('[role="menu"]') === parent) {
				later(hide, CLOSE_DELAY);
			}
		}

		parent.addEventListener('pointerover', over);
		return () => {
			parent.removeEventListener('pointerover', over);
			clearTimeout(timer);
			stop?.();
		};
	});
</script>

<button
	bind:this={trigger}
	class="menu-item"
	role="menuitem"
	tabindex="-1"
	aria-haspopup="menu"
	aria-expanded={open}
	aria-controls={id}
	onclick={(event) => show(event.detail === 0)}
	onkeydown={triggerkey}
	onpointerenter={() => later(() => show(false), OPEN_DELAY)}
	onpointerleave={() => {
		if (!open) clearTimeout(timer);
	}}
	onpointermove={hover}
>
	{@render label()}
	<span class="ml-auto inline-flex pl-3 text-secondary"><CaretRightIcon size={10} /></span>
</button>

<div
	bind:this={menu}
	{id}
	class="popup min-w-36"
	popover="auto"
	role="menu"
	tabindex="-1"
	onbeforetoggle={beforetoggle}
	onkeydown={menukey}
	onpointerenter={() => clearTimeout(timer)}
	onpointerleave={pointerleave}
>
	{@render children()}
</div>
