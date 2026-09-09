<script setup lang="ts">
import { injectNotificationManager } from '@modrinth/ui'
import { nextTick, ref, watch } from 'vue'

import {
	send_server_command,
	serverConsoleLines,
	type ChocoServer,
} from '@/helpers/servers'

const props = defineProps<{
	server: ChocoServer
	running: boolean
}>()

const { handleError } = injectNotificationManager()

const command = ref('')
const consoleElement = ref<HTMLElement | null>(null)
const autoScroll = ref(true)

const lines = ref<string[]>([])

watch(
	() => serverConsoleLines.value[props.server.id],
	(newLines) => {
		lines.value = newLines ?? []
		if (autoScroll.value) {
			void nextTick(() => {
				consoleElement.value?.scrollTo({ top: consoleElement.value.scrollHeight })
			})
		}
	},
	{ immediate: true, deep: true },
)

function onScroll() {
	if (!consoleElement.value) return
	const element = consoleElement.value
	autoScroll.value =
		element.scrollHeight - element.scrollTop - element.clientHeight < 48
}

function sendCommand() {
	const text = command.value.trim()
	if (!text || !props.running) return
	send_server_command(props.server.id, text).catch(handleError)
	command.value = ''
}
</script>

<template>
	<div class="flex flex-col gap-3">
		<div class="flex items-center justify-between">
			<h2 class="m-0 text-lg font-semibold text-contrast">Console</h2>
			<p class="m-0 text-xs text-secondary">
				{{ running ? 'Type a server command (e.g. op, whitelist, say) and press Send.' : 'Start the server to see console output.' }}
			</p>
		</div>

		<div
			ref="consoleElement"
			class="h-[28rem] overflow-y-auto rounded-2xl bg-surface-3 p-3 font-mono text-xs whitespace-pre-wrap text-secondary"
			@scroll="onScroll"
		>
			<template v-if="lines.length > 0">
				<p v-for="(line, index) in lines" :key="index" class="m-0 break-words">
					{{ line }}
				</p>
			</template>
			<p v-else class="m-0">Console output appears here while the server is running.</p>
		</div>

		<form class="flex gap-2" @submit.prevent="sendCommand">
			<input
				v-model="command"
				type="text"
				:disabled="!running"
				placeholder="Server command…"
				class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 font-mono text-contrast disabled:opacity-50"
			/>
			<Button color="brand" :disabled="!running || command.trim().length === 0">
				Send
			</Button>
		</form>
	</div>
</template>
