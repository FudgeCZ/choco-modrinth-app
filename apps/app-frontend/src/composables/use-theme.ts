import { prepareThemeColorTransition } from '@modrinth/ui'
import { computed, reactive, ref, watch } from 'vue'

export const THEME_OPTIONS = ['dark', 'light', 'oled', 'retro', 'system'] as const
export const DARK_THEMES = ['dark', 'oled', 'retro'] as const

export type ColorTheme = (typeof THEME_OPTIONS)[number]
export type DarkTheme = (typeof DARK_THEMES)[number]
type Theme = Exclude<ColorTheme, 'system'>
type NativeTheme = 'light' | 'dark'

const PREFERRED_THEME_KEY = 'modrinth-theme'
const PREFERRED_DARK_THEME_KEY = 'modrinth-preferred-dark-theme'
const PREFERRED_ACCENT_KEY = 'chocomodrinth-accent'

export const ACCENT_OPTIONS = [
	'chocolate',
	'green',
	'blue',
	'red',
	'purple',
	'orange',
	'pink',
] as const
export type AccentColor = (typeof ACCENT_OPTIONS)[number]

const ACCENT_COLORS: Record<Exclude<AccentColor, 'chocolate'>, { light: string; dark: string }> = {
	green: { light: '#00af5c', dark: '#1bd96a' },
	blue: { light: '#1f68c0', dark: '#4f9cff' },
	red: { light: '#cb2245', dark: '#ff496e' },
	purple: { light: '#8e32f3', dark: '#c78aff' },
	orange: { light: '#e08325', dark: '#ffa347' },
	pink: { light: '#d4427a', dark: '#ff7eb6' },
}

function hexToRgbChannel(hex: string): string {
	const value = hex.replace('#', '')
	const r = parseInt(value.slice(0, 2), 16)
	const g = parseInt(value.slice(2, 4), 16)
	const b = parseInt(value.slice(4, 6), 16)
	return `${r}, ${g}, ${b}`
}

export function getAccentSwatch(accent: AccentColor, darkTheme: boolean): string {
	if (accent === 'chocolate') {
		return darkTheme ? '#d99a5b' : '#8a5a2b'
	}
	const color = ACCENT_COLORS[accent]
	return darkTheme ? color.dark : color.light
}

export function isDarkTheme(theme: string): theme is DarkTheme {
	return (DARK_THEMES as readonly string[]).includes(theme)
}

function loadPreferredTheme(): ColorTheme {
	try {
		const stored = window.localStorage.getItem(PREFERRED_THEME_KEY)
		if (stored && (THEME_OPTIONS as readonly string[]).includes(stored)) {
			return stored as ColorTheme
		}
	} catch {
		// storage blocked or full
	}

	for (const option of THEME_OPTIONS) {
		if (option !== 'system' && document.documentElement.classList.contains(`${option}-mode`)) {
			return option
		}
	}

	return 'dark'
}

function loadPreferredDarkTheme(): DarkTheme {
	try {
		const stored = window.localStorage.getItem(PREFERRED_DARK_THEME_KEY)
		if (stored && isDarkTheme(stored)) {
			return stored
		}
	} catch {
		// storage blocked or full
	}

	return 'dark'
}

function loadPreferredAccent(): AccentColor {
	try {
		const stored = window.localStorage.getItem(PREFERRED_ACCENT_KEY)
		if (stored && (ACCENT_OPTIONS as readonly string[]).includes(stored)) {
			return stored as AccentColor
		}
	} catch {
		// storage blocked or full
	}

	return 'chocolate'
}

const preferred = ref<ColorTheme>(loadPreferredTheme())
const preview = ref<ColorTheme | null>(null)
const preferredDark = ref<DarkTheme>(loadPreferredDarkTheme())
const advancedRendering = ref(true)
const syncAcrossDevices = ref(false)
const nativeThemeQuery = window.matchMedia('(prefers-color-scheme: dark)')
const native = ref<NativeTheme>(nativeThemeQuery.matches ? 'dark' : 'light')
const active = computed<Theme>(() => {
	const selectedTheme = preview.value ?? preferred.value
	if (selectedTheme !== 'system') {
		return selectedTheme
	}

	return native.value === 'light' ? 'light' : preferredDark.value
})

nativeThemeQuery.addEventListener('change', (event) => {
	native.value = event.matches ? 'dark' : 'light'
})

watch([preferred, preview], ([selectedPreferred, selectedPreview]) => {
	const selectedTheme = selectedPreview ?? selectedPreferred
	if (isDarkTheme(selectedTheme)) {
		preferredDark.value = selectedTheme
	}
})

watch(
	preferred,
	(theme) => {
		try {
			window.localStorage.setItem(PREFERRED_THEME_KEY, theme)
		} catch {
			// storage blocked or full
		}
	},
	{ immediate: true },
)

watch(preferredDark, (theme) => {
	try {
		window.localStorage.setItem(PREFERRED_DARK_THEME_KEY, theme)
	} catch {
		// storage blocked or full
	}
})

const preferredAccent = ref<AccentColor>(loadPreferredAccent())

watch(preferredAccent, (accent) => {
	try {
		window.localStorage.setItem(PREFERRED_ACCENT_KEY, accent)
	} catch {
		// storage blocked or full
	}
})

function applyAccentColors(accent: AccentColor, mode: Theme): void {
	const html = document.documentElement
	const properties = ['--color-brand', '--color-brand-highlight', '--color-brand-shadow']
	if (accent === 'chocolate') {
		for (const property of properties) {
			html.style.removeProperty(property)
		}
		return
	}

	const color = ACCENT_COLORS[accent]
	const hex = isDarkTheme(mode) ? color.dark : color.light
	const rgb = hexToRgbChannel(hex)
	html.style.setProperty('--color-brand', hex)
	html.style.setProperty('--color-brand-highlight', `rgba(${rgb}, 0.25)`)
	html.style.setProperty('--color-brand-shadow', `rgba(${rgb}, 0.7)`)
}

watch([preferredAccent, active], ([accent, mode]) => applyAccentColors(accent, mode), {
	immediate: true,
})

watch(
	active,
	(theme, previousTheme) => {
		if (previousTheme && previousTheme !== theme) {
			prepareThemeColorTransition()
		}

		const html = document.documentElement
		for (const option of THEME_OPTIONS) {
			html.classList.remove(`${option}-mode`)
		}
		html.classList.add(`${theme}-mode`)
	},
	{ immediate: true },
)

function applyAccountAppearance(appearance: { auto: boolean; theme: string }): void {
	if (isDarkTheme(appearance.theme)) {
		preferredDark.value = appearance.theme
	}

	if (appearance.auto) {
		preferred.value = 'system'
		return
	}

	if ((THEME_OPTIONS as readonly string[]).includes(appearance.theme)) {
		preferred.value = appearance.theme as ColorTheme
	}
}

const theme = reactive({
	preferred,
	preview,
	preferredDark,
	active,
	native,
	preferredAccent,
	syncAcrossDevices,
	advancedRendering,
	options: THEME_OPTIONS,
	accentOptions: ACCENT_OPTIONS,
	applyAccountAppearance,
})

export function useTheme() {
	return theme
}
