<template>
	<svg
		xmlns="http://www.w3.org/2000/svg"
		:class="{ animate }"
		viewBox="0 0 1900 240"
		role="img"
		aria-label="ChocoModrinth"
	>
		<!-- chocolate bar mark -->
		<g fill="var(--color-brand)" transform="translate(12, 20)">
			<rect x="0" y="0" width="88" height="88" rx="16" />
			<rect x="104" y="0" width="88" height="88" rx="16" />
			<rect x="0" y="104" width="88" height="88" rx="16" />
			<rect x="104" y="104" width="88" height="88" rx="16" />
		</g>
		<text
			x="266"
			y="191"
			font-family="Inter, 'Segoe UI Variable Display', 'Segoe UI', system-ui, -apple-system, sans-serif"
			font-size="205"
			font-weight="800"
			letter-spacing="-5"
			textLength="1610"
			lengthAdjust="spacingAndGlyphs"
		>
			<tspan fill="var(--color-brand)">Choco</tspan><tspan fill="currentColor">Modrinth</tspan>
		</text>
	</svg>
</template>

<script setup>
defineProps({
	animate: {
		type: Boolean,
		default: false,
	},
})
</script>

<style lang="scss" scoped>
.animate {
	text {
		transform-origin: center;
		transform-box: fill-box;
	}
}
</style>
