<script setup lang="ts">
import { ChevronRightIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, injectNotificationManager } from '@modrinth/ui'
import { computed, onMounted, onUnmounted, ref } from 'vue'

import {
	get_server_stats,
	ping_server,
	send_server_command,
	serverConsoleLines,
	type ChocoServer,
	type ServerPing,
	type ServerStats,
} from '@/helpers/servers'

import Sparkline from './Sparkline.vue'

const props = defineProps<{
	server: ChocoServer
	running: boolean
}>()

const emit = defineEmits<{
	'open-console': []
}>()

const { handleError } = injectNotificationManager()

const cpuHistory = ref<number[]>([])
const ramHistory = ref<number[]>([])
const latestStats = ref<ServerStats | null>(null)
const statsError = ref(false)

const ping = ref<ServerPing | null>(null)
const pinging = ref(false)

const chatMessage = ref('')

let statsInterval: number | undefined
let pingInterval: number | undefined

const chatLines = computed(() =>
	(serverConsoleLines.value[props.server.id] ?? [])
		.filter((line) => /\]:\s*<[^>]+>/.test(line))
		.slice(-30),
)

const consolePreview = computed(() =>
	(serverConsoleLines.value[props.server.id] ?? []).slice(-10),
)

async function pollStats() {
	if (!props.running) {
		statsError.value = false
		return
	}
	try {
		const stats = await get_server_stats(props.server.id)
		latestStats.value = stats
		statsError.value = false
		cpuHistory.value.push(Math.min(stats.cpu_percent, 100))
		ramHistory.value.push(Math.min(stats.ram_percent, 100))
		if (cpuHistory.value.length > 120) cpuHistory.value.shift()
		if (ramHistory.value.length > 120) ramHistory.value.shift()
	} catch {
		statsError.value = true
	}
}

async function pollPing() {
	if (!props.running) {
		ping.value = null
		return
	}
	pinging.value = true
	try {
		ping.value = await ping_server(props.server.port)
	} catch {
		ping.value = null
	} finally {
		pinging.value = false
	}
}

function sendChat() {
	const message = chatMessage.value.trim()
	if (!message || !props.running) return
	send_server_command(props.server.id, `/say ${message}`).catch(handleError)
	chatMessage.value = ''
}

onMounted(() => {
	void pollStats()
	void pollPing()
	statsInterval = window.setInterval(pollStats, 1000)
	pingInterval = window.setInterval(pollPing, 15000)
})

onUnmounted(() => {
	if (statsInterval) window.clearInterval(statsInterval)
	if (pingInterval) window.clearInterval(pingInterval)
})
</script>

<template>
	<div class="grid grid-cols-1 gap-4 xl:grid-cols-[24rem_1fr]">
		<div class="flex flex-col gap-4 rounded-2xl border-0 border-solid border-divider bg-surface-2 p-4">
			<h2 class="m-0 text-lg font-semibold text-contrast">Stats</h2>
			<template v-if="running">
				<div>
					<div class="mb-1 flex items-baseline justify-between">
						<span class="font-semibold text-contrast">RAM</span>
						<span class="text-xl font-bold text-brand">
							{{ Math.round(latestStats?.ram_percent ?? 0) }}%
						</span>
					</div>
					<Sparkline :values="ramHistory" />
					<p class="m-0 text-xs text-secondary">
						{{ latestStats?.ram_mb ?? 0 }} MB used by the server process
					</p>
				</div>
				<div>
					<div class="mb-1 flex items-baseline justify-between">
						<span class="font-semibold text-contrast">CPU</span>
						<span class="text-xl font-bold text-brand">
							{{ Math.round(latestStats?.cpu_percent ?? 0) }}%
						</span>
					</div>
					<Sparkline :values="cpuHistory" />
					<p class="m-0 text-xs text-secondary">
						of one core, averaged over the last second
					</p>
				</div>
				<p v-if="statsError && !latestStats" class="m-0 text-xs text-secondary">
					Waiting for stats…
				</p>
			</template>
			<p v-else class="m-0 text-sm text-secondary">
				Start the server to see live RAM and CPU usage.
			</p>
		</div>

		<div class="flex flex-col gap-4">
			<div class="flex min-h-[16rem] flex-col rounded-2xl border-0 border-solid border-divider bg-surface-2 p-4">
				<h2 class="m-0 text-lg font-semibold text-contrast">Players &amp; chat</h2>
				<div v-if="running" class="mb-2 flex flex-wrap items-center gap-2 text-sm text-secondary">
					<span>
						{{ ping?.players_online ?? 0 }} / {{ ping?.players_max ?? '?' }} players online
					</span>
					<SpinnerIcon v-if="pinging" class="size-3 animate-spin" />
				</div>
				<div v-if="running" class="mb-2 flex flex-wrap gap-1.5">
					<span
						v-for="player in ping?.players ?? []"
						:key="player"
						class="rounded-full bg-surface-4 px-2.5 py-0.5 text-sm text-contrast"
					>
						{{ player }}
					</span>
					<span
						v-if="ping && ping.players_online > ping.players.length"
						class="rounded-full bg-surface-4 px-2.5 py-0.5 text-sm text-secondary"
					>
						+{{ ping.players_online - ping.players.length }} more
					</span>
				</div>
				<p v-if="!running" class="m-0 text-sm text-secondary">
					Start the server to see players and chat.
				</p>
				<template v-else>
					<div class="mb-2 flex min-h-[4rem] flex-1 flex-col gap-0.5 overflow-y-auto rounded-xl bg-surface-3 p-2 text-xs text-secondary">
						<p v-if="chatLines.length === 0" class="m-0">
							Chat messages appear here.
						</p>
						<p v-for="(line, index) in chatLines" :key="index" class="m-0 break-words">
							{{ line.replace(/^\[[^\]]+\]\s*\[[^\]]+\]:\s*/, '') }}
						</p>
					</div>
					<form class="flex gap-2" @submit.prevent="sendChat">
						<input
							v-model="chatMessage"
							type="text"
							placeholder="Send a message as [Server]…"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						/>
						<Button color="brand" native-type="submit">Send</Button>
					</form>
				</template>
			</div>

			<div class="flex flex-col rounded-2xl border-0 border-solid border-divider bg-surface-2 p-4">
				<div class="flex items-center justify-between gap-2">
					<h2 class="m-0 text-lg font-semibold text-contrast">Console</h2>
					<Button @click="emit('open-console')">
						Open console
						<ArrowRightIcon aria-hidden="true" />
					</Button>
				</div>
				<pre
					class="mt-2 h-40 overflow-hidden rounded-xl bg-surface-3 p-3 text-xs whitespace-pre-wrap text-secondary"
					>{{ consolePreview.length > 0 ? consolePreview.join('\n') : 'Console output appears here while the server is running.' }}</pre
				>
			</div>
		</div>
	</div>
</template>
