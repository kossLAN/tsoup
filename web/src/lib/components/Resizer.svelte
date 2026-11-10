<script lang="ts">
	let { min, onresize }: { min: number; onresize: (width: number) => void } = $props();

	const reserved = 320;

	let dragging = $state(false);

	function down(event: PointerEvent) {
		(event.currentTarget as Element).setPointerCapture(event.pointerId);
		dragging = true;
	}

	function move(event: PointerEvent) {
		const pane = (event.currentTarget as Element).parentElement;
		if (!dragging || !pane) return;
		const width = pane.getBoundingClientRect().right - event.clientX;
		const max = Math.max(min, window.innerWidth - reserved);
		onresize(Math.round(Math.min(Math.max(width, min), max)));
	}
</script>

<div
	class={[
		'absolute inset-y-0 -left-0.75 z-10 w-1.5 cursor-col-resize transition-colors hover:bg-accent/50',
		dragging && 'bg-accent/50'
	]}
	role="separator"
	aria-orientation="vertical"
	onpointerdown={down}
	onpointermove={move}
	onpointerup={() => (dragging = false)}
	onpointercancel={() => (dragging = false)}
></div>
