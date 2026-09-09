<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
	defineProps<{
		values: number[]
		color?: string
		height?: number
	}>(),
	{
		color: 'var(--color-brand)',
		height: 56,
	},
)

const WIDTH = 600
const HEIGHT = 120

const linePath = computed(() => {
	const values = props.values
	if (values.length < 2) {
		return ''
	}
	const max = Math.max(...values, 1)
	const step = WIDTH / (values.length - 1)
	return values
		.map((value, index) => {
			const x = index * step
			const y = HEIGHT - 6 - (Math.max(0, Math.min(value, max)) / max) * (HEIGHT - 12)
			return `${index === 0 ? 'M' : 'L'}${x.toFixed(1)} ${y.toFixed(1)}`
		})
		.join(' ')
})

const areaPath = computed(() =>
	linePath.value ? `${linePath.value} L${WIDTH} ${HEIGHT} L0 ${HEIGHT} Z` : '',
)
</script>

<template>
	<div class="w-full" :style="{ height: `${height}px` }">
		<svg
			v-if="areaPath"
			viewBox="0 0 600 120"
			preserveAspectRatio="none"
			class="h-full w-full"
			aria-hidden="true"
		>
			<path :d="areaPath" :fill="color" fill-opacity="0.15" />
			<path
				:d="linePath"
				fill="none"
				:stroke="color"
				stroke-width="4"
				stroke-linejoin="round"
				stroke-linecap="round"
			/>
		</svg>
		<div
			v-else
			class="flex h-full items-center text-xs text-secondary"
		>
			Collecting data…
		</div>
	</div>
</template>
