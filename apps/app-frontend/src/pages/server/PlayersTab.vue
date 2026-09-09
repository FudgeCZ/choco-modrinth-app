<script setup lang="ts">
import { SpinnerIcon } from '@modrinth/assets'
import { injectNotificationManager } from '@modrinth/ui'
import { onMounted, onUnmounted, ref } from 'vue'

import { ping_server, type ChocoServer, type ServerPing } from '@/helpers/servers'

const props = defineProps<{
	server: ChocoServer
	running: boolean
}>()

const { handleError } = injectNotificationManager()

const ping = ref<ServerPing | null>(null)
const pinging = ref(false)
let interval: number | undefined

async function poll() {
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

onMounted(() => {
	void poll()
	interval = window.setInterval(poll, 10000)
})

onUnmounted(() => {
	if (interval) window.clearInterval(interval)
})
</script>

<template>
	<div class="flex flex-col gap-3">
		<h2 class="m-0 text-lg font-semibold text-contrast">Players</h2>

		<p v-if="!running" class="m-0 text-sm text-secondary">
			Start the server to see who is online.
		</p>

		<template v-else>
			<div class="flex items-center gap-3">
				<p class="m-0 text-2xl font-bold text-contrast">
					{{ ping?.players_online ?? 0 }}
					<span class="text-base font-semibold text-secondary">
						/ {{ ping?.players_max ?? '?' }} online
					</span>
				</p>
				<SpinnerIcon v-if="pinging" class="size-4 animate-spin text-secondary" />
			</div>

			<p v-if="ping?.motd" class="m-0 text-sm text-secondary">
				{{ ping.motd }}
			</p>
			<p v-if="ping?.version" class="m-0 text-xs text-secondary">
				Server reports version {{ ping.version }}
			</p>

			<div v-if="(ping?.players.length ?? 0) > 0" class="flex flex-wrap gap-2">
				<span
					v-for="player in ping?.players"
					:key="player"
					class="rounded-full bg-surface-3 px-3 py-1 text-contrast"
				>
					{{ player }}
				</span>
			</div>
			<p v-else class="m-0 text-sm text-secondary">
				No players are online right now. Note: servers only report the player
				list when it is enabled in their server.properties.
			</p>
		</template>
	</div>
</template>
