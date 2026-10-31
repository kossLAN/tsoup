<script lang="ts">
	const bubbles = [
		{ x: 7, begin: '0s' },
		{ x: 16, begin: '-0.37s' },
		{ x: 25, begin: '-0.73s' }
	];
</script>

<svg class="size-full" viewBox="0 0 32 32" aria-hidden="true">
	{#each bubbles as { x, begin } (x)}
		<circle class="fill-accent" cx={x} cy="26" r="2">
			<animate attributeName="cy" values="26;8" dur="1.1s" {begin} repeatCount="indefinite" />
			<animate
				attributeName="r"
				values="2;5.5;2"
				keyTimes="0;0.5;1"
				calcMode="spline"
				keySplines="0.39 0.575 0.565 1;0.47 0 0.745 0.715"
				dur="1.1s"
				{begin}
				repeatCount="indefinite"
			/>
			<animate
				attributeName="opacity"
				values="1;1;0"
				keyTimes="0;0.75;1"
				dur="1.1s"
				{begin}
				repeatCount="indefinite"
			/>
		</circle>
	{/each}
</svg>
