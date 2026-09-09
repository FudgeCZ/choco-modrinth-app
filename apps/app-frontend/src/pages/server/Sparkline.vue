<script setup lang="ts">
const props = withDefaults(
	defineProps<{
		values: number[]
		color?: string
		height?: number
	}>(),
	{
		color: 'var(--color-brand)',
		height: 48,
	},
)

const points = computed(() => {
	const values = props.values
	if (values.length === 0) {
		return ''
	}
	const max = Math.max(...values, 0.0001)
	const step = 100 / Math.max(values.length - 1, 1)
	return values
		.map((value, index) => {
			const x = index * step
			const y = 100 - Math.max(0, Math.min(1, value / max)) * 92 - 4
			return `${x.toFixed(2)},${y.toFixed(2)}`
		})
		.join(' ')
})
</script>

<template>
	<svg
		:viewBox="`0 0 100 100`"
		preserveAspectRatio="none"
		class="w-full"
		:style="{ height: `${height}px` }"
		aria-hidden="true"
	>
		<polyline
			:points="points"
			fill="none"
			:stroke="color"
			stroke-width="2.5"
			vector-effect="non-scaling-stroke"
			stroke-linejoin="round"
			stroke-linecap="round"
		/>
	</svg>
</template>
