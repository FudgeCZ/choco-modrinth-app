// Reactive demo runtime for the "Demo view" developer feature. Simulates
// running state, live stats, console output and player activity for the fake
// servers declared in demo-data.ts. The pure data lives in demo-data.ts.
import { ref } from 'vue'

import { useAppSettings } from '@/composables/use-app-settings'
import {
	DEMO_CHAT_ROTATION,
	DEMO_CONSOLE_IDLE_LINES,
	DEMO_PLAYERS,
	DEMO_SERVERS,
} from '@/helpers/demo-data'
import {
	type ChocoServer,
	serverConsoleLines,
	type ServerPing,
	type ServerPlayersOverview,
	type ServerStats,
	serverStats,
} from '@/helpers/servers'

// Survival and modded start out running, creative is stopped.
const demoRunning = ref<Record<string, boolean>>({
	'demo:server-survival': true,
	'demo:server-modded': true,
	'demo:server-creative': false,
})

// Ids currently performing their simulated shutdown; toggling is ignored
// while the stop sequence is playing out.
const stoppingIds = new Set<string>()

function demoTimestamp(): string {
	const now = new Date()
	const pad = (value: number) => String(value).padStart(2, '0')
	return `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`
}

function appendConsoleLines(id: string, newLines: string[]): void {
	if (newLines.length === 0) return
	const lines = serverConsoleLines.value[id] ?? []
	lines.push(...newLines)
	// Keep the buffer bounded, matching the real backend listener
	if (lines.length > 500) lines.splice(0, lines.length - 500)
	serverConsoleLines.value = { ...serverConsoleLines.value, [id]: lines }
}

export function demoIsRunning(id: string): boolean {
	return demoRunning.value[id] ?? false
}

export function demoSetRunning(id: string, running: boolean): void {
	if (running) {
		if (demoIsRunning(id)) return
		stoppingIds.delete(id)
		demoRunning.value = { ...demoRunning.value, [id]: true }
		seedBootSequence(id)
	} else {
		if (!demoIsRunning(id) || stoppingIds.has(id)) return
		stoppingIds.add(id)
		const timestamp = demoTimestamp()
		appendConsoleLines(id, [
			`${timestamp} [Server thread/INFO]: Stopping the server`,
			`${timestamp} [Server thread/INFO]: Saving players`,
			`${timestamp} [Server thread/INFO]: Saving worlds`,
			`${timestamp} [Server thread/INFO]: Saving chunks for level 'ServerLevel[world]'/minecraft:overworld`,
		])
		window.setTimeout(() => {
			demoRunning.value = { ...demoRunning.value, [id]: false }
			stoppingIds.delete(id)
			appendConsoleLines(id, [`${demoTimestamp()} [Server thread/INFO]: Done, exiting`])
		}, 1500)
	}
}

export function demoToggleRunning(id: string): void {
	demoSetRunning(id, !demoIsRunning(id))
}

export function demoRunningServers(): ChocoServer[] {
	return DEMO_SERVERS.filter((server) => demoIsRunning(server.id))
}

function seedBootSequence(id: string): void {
	const server = DEMO_SERVERS.find((entry) => entry.id === id)
	const version = server?.game_version ?? '26.2'
	const timestamp = demoTimestamp()
	const lines = [
		`${timestamp} [ServerMain/INFO]: Starting minecraft server version ${version}`,
		`${timestamp} [ServerMain/INFO]: Loading properties`,
		`${timestamp} [Server thread/INFO]: Preparing level "world"`,
		`${timestamp} [Server thread/INFO]: Preparing spawn area: 85%`,
		`${timestamp} [Server thread/INFO]: Done (6.3s)! For help, type "help"`,
	]
	for (const player of DEMO_PLAYERS[id] ?? []) {
		lines.push(`${timestamp} [Server thread/INFO]: ${player} joined the game`)
	}
	appendConsoleLines(id, lines)
}

function clamp(value: number, min: number, max: number): number {
	return Math.min(max, Math.max(min, value))
}

function pushRandomStats(id: string): void {
	const server = DEMO_SERVERS.find((entry) => entry.id === id)
	const entry = serverStats.value[id] ?? { cpu: [], ram: [], latest: null }
	const previousRam = entry.ram.length > 0 ? entry.ram[entry.ram.length - 1] : 45
	const cpu = clamp(5 + Math.random() * 80, 0, 100)
	const ram = clamp(previousRam + (Math.random() * 6 - 3), 25, 70)
	const stats: ServerStats = {
		cpu_percent: cpu,
		ram_percent: ram,
		ram_mb: Math.round((ram / 100) * (server?.ram_mb ?? 4096)),
	}
	entry.cpu = [...entry.cpu, cpu].slice(-120)
	entry.ram = [...entry.ram, ram].slice(-120)
	entry.latest = stats
	serverStats.value = { ...serverStats.value, [id]: entry }
}

function pickRandom<T>(items: T[]): T | undefined {
	if (items.length === 0) return undefined
	return items[Math.floor(Math.random() * items.length)]
}

function appendIdleLine(id: string): void {
	const timestamp = demoTimestamp()
	const players = DEMO_PLAYERS[id] ?? []
	const roll = Math.random()
	if (roll < 0.4) {
		const chat = pickRandom(DEMO_CHAT_ROTATION)
		if (chat) {
			appendConsoleLines(id, [
				`${timestamp} [Server thread/INFO]: <${chat.speaker}> ${chat.message}`,
			])
			return
		}
	}
	if (roll >= 0.4 && roll < 0.7 && players.length > 0) {
		const player = pickRandom(players)
		if (player) {
			const leaving = Math.random() < 0.5
			appendConsoleLines(id, [
				leaving
					? `${timestamp} [Server thread/INFO]: ${player} left the game`
					: `${timestamp} [Server thread/INFO]: ${player} joined the game`,
			])
			return
		}
	}
	const idleLine = pickRandom(DEMO_CONSOLE_IDLE_LINES)
	if (idleLine) {
		appendConsoleLines(id, [`${timestamp} ${idleLine}`])
	}
}

let tickerStarted = false
let tickCount = 0

function tick(): void {
	// Harmless no-op when the demo flag is off
	if (!useAppSettings().getFeatureFlag('demo_view')) return
	tickCount++
	for (const server of DEMO_SERVERS) {
		if (!demoIsRunning(server.id)) continue
		pushRandomStats(server.id)
		// ~7.5s between idle lines at a 1.5s tick
		if (tickCount % 5 === 0) {
			appendIdleLine(server.id)
		}
	}
}

export function startDemoTicker(): void {
	if (tickerStarted) return
	tickerStarted = true
	window.setInterval(tick, 1500)
}

export function ensureDemoRuntime(): void {
	startDemoTicker()
}

export function demoPing(id: string): ServerPing {
	const players = DEMO_PLAYERS[id] ?? []
	const suffix = id.startsWith('demo:server-') ? id.slice('demo:server-'.length) : 'demo'
	return {
		motd: `Demo ${suffix} server`,
		players_online: players.length,
		players_max: 20,
		players: [...players],
		favicon: null,
		version: DEMO_SERVERS.find((server) => server.id === id)?.game_version ?? '26.2',
	}
}

export function demoPlayersOverview(id: string): ServerPlayersOverview {
	const players = DEMO_PLAYERS[id] ?? []
	return {
		players: players.map((name, index) => ({
			name,
			uuid: `demo-${id.slice('demo:'.length)}-${index + 1}`,
			op: name === 'Steve',
			whitelisted: name === 'Steve' || name === 'Alex',
			banned: false,
			ban_reason: null,
		})),
	}
}

// Simulated /say from the console so the DashboardTab chat box works in demo
export function demoSay(id: string, message: string): void {
	appendConsoleLines(id, [`${demoTimestamp()} [Server thread/INFO]: [Server] ${message}`])
}

// Start the ticker lazily: it is gated on the demo flag inside tick(), so
// keeping it running while the feature is off is harmless.
ensureDemoRuntime()
