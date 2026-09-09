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

const lines = ref<string[]>([])

watch(
	() => serverConsoleLines.value[props.server.id]?.length,
	async () => {
		lines.value = serverConsoleLines.value[props.server.id] ?? []
		await nextTick()
		const element = consoleElement.value
		if (element) {
			element.scrollTop = element.scrollHeight
		}
	},
	{ immediate: true },
)
</script>

<template>
	<div class="flex h-[calc(100vh-19rem)] min-h-[20rem] flex-col gap-3">
		<div class="flex items-center justify-between">
			<h2 class="m-0 text-lg font-semibold text-contrast">Console</h2>
			<p class="m-0 text-xs text-secondary">
				{{
					running
						? 'Type a server command (e.g. op, whitelist, say) and press Send.'
						: 'Start the server to see console output.'
				}}
			</p>
		</div>

		<div
			ref="consoleElement"
			class="min-h-0 flex-1 overflow-y-auto rounded-2xl bg-surface-3 p-3 font-mono text-xs whitespace-pre-wrap text-secondary"
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
			<Button color="brand" native-type="submit" :disabled="!running || command.trim().length === 0">
				Send
			</Button>
		</form>
	</div>
</template>
