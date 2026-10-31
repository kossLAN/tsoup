<script lang="ts">
	const id = $props.id();
</script>

<svg class="size-full" viewBox="0 0 32 32" aria-hidden="true">
	<clipPath {id}><circle cx="16" cy="16" r="10.6" /></clipPath>
	<circle cx="16" cy="16" r="14.25" fill="none" stroke="currentColor" stroke-width="3.5" />
	<g clip-path="url(#{id})">
		<g>
			<animateTransform
				attributeName="transform"
				type="rotate"
				values="-14 16 16;14 16 16;-14 16 16"
				keyTimes="0;0.5;1"
				calcMode="spline"
				keySplines="0.45 0 0.55 1;0.45 0 0.55 1"
				dur="2.2s"
				repeatCount="indefinite"
			/>
			<path
				class="fill-accent"
				d="M-38 16.5 q5.5 -2.4 11 0 t11 0 t11 0 t11 0 t11 0 t11 0 t11 0 t11 0 V40 H-38 Z"
			>
				<animateTransform
					attributeName="transform"
					type="translate"
					from="0 0"
					to="22 0"
					dur="1.1s"
					repeatCount="indefinite"
				/>
			</path>
		</g>
	</g>
</svg>
