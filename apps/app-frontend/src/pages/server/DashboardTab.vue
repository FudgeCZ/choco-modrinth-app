<script setup lang="ts">
import { ChevronRightIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, injectNotificationManager } from '@modrinth/ui'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import { isDemoId } from '@/helpers/demo-data'
import { demoPing, demoSay } from '@/helpers/demo-runtime'
import {
	type ChocoServer,
	ping_server,
	send_server_command,
	serverConsoleLines,
	type ServerPing,
	serverStats,
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

const stats = computed(
	() => serverStats.value[props.server.id] ?? { cpu: [], ram: [], latest: null },
)

const ping = ref<ServerPing | null>(null)
const pinging = ref(false)

const chatMessage = ref('')

let pingInterval: number | undefined

type ChatLine = { speaker: string; message: string }

/**
 * Parses a console line into a chat entry.
 * Handles player chat (`<Steve> hello`), `/say` from the console
 * (`[Not Secure] [Server] hi` or `[Server] hi`) and returns null for
 * anything else (joins, deaths, plugin spam, …).
 */
function parseChatLine(line: string): ChatLine | null {
	const body = line.replace(/^\[[^\]]*\]\s*\[[^\]]*\]:\s*/, '')
	if (body === line) return null // missing "[time] [thread/level]: " prefix

	// Player chat: <Steve> hello world
	let match = /^<([^>]+)>\s*(.*)$/.exec(body)
	if (match) return { speaker: match[1], message: match[2] }

	// /say from console: [Not Secure] [Server] hi
	match = /^\[Not Secure\]\s*\[([^\]]+)\]\s*(.*)$/.exec(body)
	if (match) return { speaker: match[1], message: match[2] }

	// /say from RCON or plugins: [Server] hi
	match = /^\[([^\]]+)\]\s*(.*)$/.exec(body)
	if (match && match[1] === 'Server') return { speaker: match[1], message: match[2] }

	return null
}

const chatLines = computed(() =>
	(serverConsoleLines.value[props.server.id] ?? [])
		.map(parseChatLine)
		.filter((entry): entry is ChatLine => entry !== null)
		.slice(-30),
)

const consolePreview = computed(() => (serverConsoleLines.value[props.server.id] ?? []).slice(-10))

const previewElement = ref<HTMLElement | null>(null)

watch(
	() => serverConsoleLines.value[props.server.id]?.length,
	async () => {
		await nextTick()
		const element = previewElement.value
		if (element) {
			element.scrollTop = element.scrollHeight
		}
	},
	{ immediate: true },
)

async function pollPing() {
	if (!props.running) {
		ping.value = null
		return
	}
	if (isDemoId(props.server.id)) {
		ping.value = demoPing(props.server.id)
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
	if (isDemoId(props.server.id)) {
		demoSay(props.server.id, message)
		chatMessage.value = ''
		return
	}
	send_server_command(props.server.id, `/say ${message}`).catch(handleError)
	chatMessage.value = ''
}

onMounted(() => {
	void pollPing()
	pingInterval = window.setInterval(pollPing, 15000)
})

onUnmounted(() => {
	if (pingInterval) window.clearInterval(pingInterval)
})
</script>

<template>
	<div class="grid grid-cols-1 gap-4 xl:grid-cols-[24rem_1fr]">
		<div
			class="flex h-full min-h-[calc(100vh-24rem)] flex-col gap-4 rounded-2xl border-0 border-solid border-divider bg-surface-2 p-4"
		>
			<h2 class="m-0 text-lg font-semibold text-contrast">Stats</h2>
			<template v-if="running">
				<div>
					<div class="mb-1 flex items-baseline justify-between">
						<span class="font-semibold text-contrast">RAM</span>
						<span class="text-xl font-bold text-brand">
							{{ Math.round(stats.latest?.ram_percent ?? 0) }}%
						</span>
					</div>
					<Sparkline :values="stats.ram" />
					<p class="m-0 text-xs text-secondary">
						{{ stats.latest?.ram_mb ?? 0 }} MB used by the server process
					</p>
				</div>
				<div>
					<div class="mb-1 flex items-baseline justify-between">
						<span class="font-semibold text-contrast">CPU</span>
						<span class="text-xl font-bold text-brand">
							{{ Math.round(stats.latest?.cpu_percent ?? 0) }}%
						</span>
					</div>
					<Sparkline :values="stats.cpu" />
					<p class="m-0 text-xs text-secondary">of one core, averaged over the last second</p>
				</div>
				<p v-if="!stats.latest" class="m-0 text-xs text-secondary">Waiting for stats…</p>
			</template>
			<p v-else class="m-0 text-sm text-secondary">
				Start the server to see live RAM and CPU usage.
			</p>
		</div>

		<div class="flex min-h-[calc(100vh-24rem)] flex-col gap-4">
			<div
				class="flex min-h-[16rem] flex-1 flex-col rounded-2xl border-0 border-solid border-divider bg-surface-2 p-4"
			>
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
					<div
						class="mb-2 flex min-h-[4rem] flex-1 flex-col gap-0.5 overflow-y-auto rounded-xl bg-surface-3 p-2 text-xs text-secondary"
					>
						<p v-if="chatLines.length === 0" class="m-0">Chat messages appear here.</p>
						<p v-for="(line, index) in chatLines" :key="index" class="m-0 break-words">
							{{ line.speaker }}: {{ line.message }}
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

			<div
				class="flex h-64 flex-col rounded-2xl border-0 border-solid border-divider bg-surface-2 p-4"
			>
				<div class="flex items-center justify-between gap-2">
					<h2 class="m-0 text-lg font-semibold text-contrast">Console</h2>
					<Button @click="emit('open-console')">
						Open console
						<ChevronRightIcon aria-hidden="true" />
					</Button>
				</div>
				<pre
					ref="previewElement"
					class="mt-2 min-h-0 flex-1 overflow-y-auto rounded-xl bg-surface-3 p-3 text-xs whitespace-pre-wrap text-secondary"
					>{{
						consolePreview.length > 0
							? consolePreview.join('\n')
							: 'Console output appears here while the server is running.'
					}}</pre
				>
			</div>
		</div>
	</div>
</template>
