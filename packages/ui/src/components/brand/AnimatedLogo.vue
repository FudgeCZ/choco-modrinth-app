<template>
	<div>
		<svg
			class="rotate outer"
			width="100%"
			height="100%"
			viewBox="0 0 590 591"
			xmlns="http://www.w3.org/2000/svg"
		>
			<circle
				cx="295"
				cy="295.5"
				r="225"
				fill="none"
				stroke="var(--color-brand)"
				stroke-width="56"
				stroke-linecap="round"
				pathLength="100"
				stroke-dasharray="15.5 9.5 15.5 9.5 15.5 9.5 15.5 9.5"
				transform="rotate(-90 295 295.5)"
			/>
		</svg>
		<svg
			class="rotate inner"
			width="100%"
			height="100%"
			viewBox="0 0 590 591"
			xmlns="http://www.w3.org/2000/svg"
		>
			<circle
				cx="295"
				cy="295.5"
				r="128"
				fill="none"
				stroke="var(--color-brand)"
				stroke-width="42"
				stroke-linecap="round"
				pathLength="100"
				stroke-dasharray="28 22 28 22"
				transform="rotate(90 295 295.5)"
			/>
		</svg>
		<svg width="100%" height="100%" viewBox="0 0 590 591" xmlns="http://www.w3.org/2000/svg">
			<!-- static chocolate bar center -->
			<g fill="var(--color-brand)">
				<rect x="197" y="197.5" width="88" height="88" rx="18" />
				<rect x="305" y="197.5" width="88" height="88" rx="18" />
				<rect x="197" y="305.5" width="88" height="88" rx="18" />
				<rect x="305" y="305.5" width="88" height="88" rx="18" />
			</g>
		</svg>
	</div>
</template>

<style lang="scss" scoped>
div {
	height: 5rem;

	svg {
		width: 5rem;
		height: 5rem;
		position: absolute;
		&.rotate {
			animation: rotate 4s infinite linear;
			&.inner {
				animation: rotate 6s infinite linear reverse;
			}
		}
		@keyframes rotate {
			0% {
				transform: rotate(0);
			}
			100% {
				transform: rotate(360deg);
			}
		}
	}
}
</style>
