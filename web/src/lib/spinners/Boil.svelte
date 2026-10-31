<script lang="ts">
	const id = $props.id();

	const bubbles = [
		{ x: 11, begin: '0s' },
		{ x: 17, begin: '-0.4s' },
		{ x: 21.5, begin: '-0.75s' }
	];
</script>

<svg class="size-full" viewBox="0 0 32 32" aria-hidden="true">
	<clipPath {id}><circle cx="16" cy="16" r="10.6" /></clipPath>
	<circle cx="16" cy="16" r="14.25" fill="none" stroke="currentColor" stroke-width="3.5" />
	<g class="fill-accent" clip-path="url(#{id})">
		<path
			opacity="0.45"
			d="M-38 18.5 q4 -3 8 0 t8 0 t8 0 t8 0 t8 0 t8 0 t8 0 t8 0 t8 0 t8 0 t8 0 V40 H-38 Z"
		>
			<animateTransform
				attributeName="transform"
				type="translate"
				from="16 0"
				to="0 0"
				dur="0.7s"
				repeatCount="indefinite"
			/>
		</path>
		<path d="M-38 20 q5.5 -2 11 0 t11 0 t11 0 t11 0 t11 0 t11 0 t11 0 t11 0 V40 H-38 Z">
			<animateTransform
				attributeName="transform"
				type="translate"
				from="0 0"
				to="22 0"
				dur="0.9s"
				repeatCount="indefinite"
			/>
		</path>
		{#each bubbles as { x, begin } (x)}
			<circle cx={x} cy="20" r="1.8">
				<animate
					attributeName="cy"
					values="20;14.5"
					dur="1.1s"
					{begin}
					repeatCount="indefinite"
				/>
				<animate
					attributeName="r"
					values="1.8;0.8"
					dur="1.1s"
					{begin}
					repeatCount="indefinite"
				/>
				<animate
					attributeName="opacity"
					values="1;1;0"
					keyTimes="0;0.6;1"
					dur="1.1s"
					{begin}
					repeatCount="indefinite"
				/>
			</circle>
		{/each}
	</g>
</svg>
