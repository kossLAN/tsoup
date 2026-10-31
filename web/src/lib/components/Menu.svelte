<script lang="ts">
	import { follow, items, navigate, setMenu, type Placement } from '$lib/popup';
	import type { Snippet } from 'svelte';
	import { createAttachmentKey } from 'svelte/attachments';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	let {
		trigger,
		children,
		placement = 'bottom-start',
		open = $bindable(false)
	}: {
		trigger: Snippet<[HTMLButtonAttributes]>;
		children: Snippet;
		placement?: Placement;
		open?: boolean;
	} = $props();

	const id = $props.id();
	const anchor = createAttachmentKey();

	let menu: HTMLElement;
	let button: HTMLElement;
	let keyboard = false;
	let stop: (() => void) | undefined;

	setMenu({ close: () => menu.hidePopover() });

	const attributes: HTMLButtonAttributes = $derived({
		popovertarget: id,
		'aria-haspopup': 'menu',
		'aria-expanded': open,
		'aria-controls': id,
		onclick: (event) => (keyboard = event.detail === 0),
		[anchor]: (node: HTMLElement) => {
			button = node;
		}
	});

	function beforetoggle(event: ToggleEvent) {
		open = event.newState === 'open';
		stop?.();
		stop = undefined;
		if (!open) return;

		requestAnimationFrame(() => {
			if (!menu.matches(':popover-open')) return;
			stop = follow(button, menu, placement);
			(keyboard ? (items(menu)[0] ?? menu) : menu).focus();
		});
	}

	function keydown(event: KeyboardEvent) {
		if (event.key === 'Tab') {
			menu.hidePopover();
			return;
		}
		navigate(event, menu);
	}

	function pointerleave() {
		if (menu.matches(':popover-open')) menu.focus();
	}
</script>

{@render trigger(attributes)}

<div
	bind:this={menu}
	{id}
	class="popup min-w-36"
	popover="auto"
	role="menu"
	tabindex="-1"
	onbeforetoggle={beforetoggle}
	onkeydown={keydown}
	onpointerleave={pointerleave}
>
	{@render children()}
</div>
