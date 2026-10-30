<script lang="ts">
	import { CircleNotchIcon } from '$lib/icons';

	let { size }: { size: number } = $props();
</script>

<span class="inline-flex shrink-0 animate-spin"><CircleNotchIcon {size} /></span>
