import { getContext, setContext } from 'svelte';
import type { Attachment } from 'svelte/attachments';

type Side = 'top' | 'bottom' | 'left' | 'right';
type Align = 'start' | 'center' | 'end';

export type Placement = Side | `${Side}-start` | `${Side}-end`;

export type Offset = {
	gap?: number;
	skid?: number;
};

const MARGIN = 8;

const opposite = { top: 'bottom', bottom: 'top', left: 'right', right: 'left' } as const;

export function place(
	anchor: Element,
	popup: HTMLElement,
	placement: Placement,
	{ gap = 4, skid = 0 }: Offset = {}
) {
	const box = anchor.getBoundingClientRect();
	const width = popup.offsetWidth;
	const height = popup.offsetHeight;
	const [preferred, align = 'center'] = placement.split('-') as [Side, Align?];

	const room = {
		top: box.top - gap - height >= MARGIN,
		bottom: box.bottom + gap + height <= innerHeight - MARGIN,
		left: box.left - gap - width >= MARGIN,
		right: box.right + gap + width <= innerWidth - MARGIN
	};
	const side = room[preferred] || !room[opposite[preferred]] ? preferred : opposite[preferred];

	let x: number;
	let y: number;
	if (side === 'top' || side === 'bottom') {
		y = side === 'top' ? box.top - gap - height : box.bottom + gap;
		x = aligned(box.left, box.right, width, align) + skid;
	} else {
		x = side === 'left' ? box.left - gap - width : box.right + gap;
		y = aligned(box.top, box.bottom, height, align) + skid;
	}

	popup.style.left = `${clamp(x, MARGIN, innerWidth - MARGIN - width)}px`;
	popup.style.top = `${clamp(y, MARGIN, innerHeight - MARGIN - height)}px`;
}

export function follow(
	anchor: Element,
	popup: HTMLElement,
	placement: Placement,
	offset?: Offset
): () => void {
	const update = () => place(anchor, popup, placement, offset);
	update();
	addEventListener('scroll', update, { capture: true, passive: true });
	addEventListener('resize', update, { passive: true });
	return () => {
		removeEventListener('scroll', update, { capture: true });
		removeEventListener('resize', update);
	};
}

function aligned(start: number, end: number, size: number, align: Align): number {
	if (align === 'start') return start;
	if (align === 'end') return end - size;
	return (start + end - size) / 2;
}

function clamp(value: number, min: number, max: number): number {
	return Math.max(min, Math.min(value, max));
}

export type MenuContext = { close: () => void };

const MENU = Symbol('menu');

export function setMenu(context: MenuContext) {
	setContext(MENU, context);
}

export function getMenu(): MenuContext {
	return getContext(MENU);
}

export function items(menu: HTMLElement): HTMLElement[] {
	return [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)')].filter(
		(item) => item.closest('[role="menu"]') === menu
	);
}

export function navigate(event: KeyboardEvent, menu: HTMLElement) {
	if ((event.target as Element).closest('[role="menu"]') !== menu) return;

	if (event.key === 'Escape') {
		event.stopPropagation();
		return;
	}

	const list = items(menu);
	const index = list.indexOf(document.activeElement as HTMLElement);
	let next: HTMLElement | undefined;
	switch (event.key) {
		case 'ArrowDown':
			next = list[index + 1] ?? list[0];
			break;
		case 'ArrowUp':
			next = index > 0 ? list[index - 1] : list.at(-1);
			break;
		case 'Home':
			next = list[0];
			break;
		case 'End':
			next = list.at(-1);
			break;
		default:
			return;
	}
	event.preventDefault();
	next?.focus();
}

const SHOW_DELAY = 500;
const WARM_FOR = 400;

let tooltips = 0;
let lastHidden = -Infinity;

export function tooltip(text: string, placement: Placement = 'top'): Attachment<HTMLElement> {
	return (node) => {
		const tip = document.createElement('div');
		tip.id = `tooltip-${++tooltips}`;
		tip.className = 'tooltip';
		tip.popover = 'manual';
		tip.role = 'tooltip';
		tip.textContent = text;
		document.body.append(tip);

		const labelled = Boolean(node.textContent?.trim());
		node.setAttribute(labelled ? 'aria-describedby' : 'aria-label', labelled ? tip.id : text);

		let timer: ReturnType<typeof setTimeout> | undefined;

		function show() {
			tip.showPopover();
			place(node, tip, placement, { gap: 6 });
		}

		function schedule() {
			clearTimeout(timer);
			const warm = performance.now() - lastHidden < WARM_FOR;
			timer = setTimeout(show, warm ? 0 : SHOW_DELAY);
		}

		function hide() {
			clearTimeout(timer);
			if (tip.matches(':popover-open')) {
				tip.hidePopover();
				lastHidden = performance.now();
			}
		}

		function enter(event: PointerEvent) {
			if (event.pointerType === 'mouse') schedule();
		}

		function focus() {
			if (node.matches(':focus-visible')) schedule();
		}

		function keydown(event: KeyboardEvent) {
			if (event.key === 'Escape') hide();
		}

		node.addEventListener('pointerenter', enter);
		node.addEventListener('pointerleave', hide);
		node.addEventListener('pointerdown', hide);
		node.addEventListener('focus', focus);
		node.addEventListener('blur', hide);
		node.addEventListener('keydown', keydown);

		return () => {
			hide();
			node.removeEventListener('pointerenter', enter);
			node.removeEventListener('pointerleave', hide);
			node.removeEventListener('pointerdown', hide);
			node.removeEventListener('focus', focus);
			node.removeEventListener('blur', hide);
			node.removeEventListener('keydown', keydown);
			node.removeAttribute(labelled ? 'aria-describedby' : 'aria-label');
			tip.remove();
		};
	};
}
