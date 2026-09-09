<script setup lang="ts">
import { Button, injectNotificationManager } from '@modrinth/ui'
import { onMounted, ref } from 'vue'

import { DEMO_PROPERTIES, isDemoId } from '@/helpers/demo-data'
import { type ChocoServer, get_server_properties, set_server_properties } from '@/helpers/servers'

const props = defineProps<{
	server: ChocoServer
}>()

const { handleError } = injectNotificationManager()

function asError(error: unknown): Error {
	return error instanceof Error ? error : new Error(String(error))
}

const loading = ref(true)
const saving = ref(false)
const saved = ref(false)
const entries = ref<[string, string][]>([])

const knownFields: {
	key: string
	label: string
	type: 'text' | 'number' | 'toggle' | 'select'
	options?: string[]
	hint?: string
}[] = [
	{ key: 'motd', label: 'MOTD', type: 'text' },
	{ key: 'server-port', label: 'Port', type: 'number' },
	{ key: 'max-players', label: 'Max players', type: 'number' },
	{
		key: 'difficulty',
		label: 'Difficulty',
		type: 'select',
		options: ['peaceful', 'easy', 'normal', 'hard'],
	},
	{
		key: 'gamemode',
		label: 'Game mode',
		type: 'select',
		options: ['survival', 'creative', 'adventure', 'spectator'],
	},
	{ key: 'view-distance', label: 'View distance', type: 'number' },
	{ key: 'simulation-distance', label: 'Simulation distance', type: 'number' },
	{ key: 'level-name', label: 'World name', type: 'text' },
	{ key: 'spawn-protection', label: 'Spawn protection radius', type: 'number' },
	{ key: 'pvp', label: 'PvP', type: 'toggle' },
	{ key: 'online-mode', label: 'Online mode (premium accounts only)', type: 'toggle' },
	{ key: 'white-list', label: 'Whitelist', type: 'toggle' },
	{ key: 'allow-flight', label: 'Allow flight', type: 'toggle' },
	{ key: 'force-gamemode', label: 'Force game mode', type: 'toggle' },
	{ key: 'enable-command-block', label: 'Command blocks', type: 'toggle' },
]

function valueOf(key: string): string {
	return entries.value.find(([entryKey]) => entryKey === key)?.[1] ?? ''
}

function setValue(key: string, value: string) {
	const entry = entries.value.find(([entryKey]) => entryKey === key)
	if (entry) {
		entry[1] = value
	} else {
		entries.value.push([key, value])
	}
	saved.value = false
}

function toggleValue(key: string) {
	setValue(key, valueOf(key) === 'true' ? 'false' : 'true')
}

onMounted(async () => {
	if (isDemoId(props_serverId())) {
		// Demo servers serve their properties from the demo data; copies so
		// edits stay local (saving is a no-op).
		entries.value = (DEMO_PROPERTIES[props_serverId()] ?? []).map((entry) => [entry[0], entry[1]])
		loading.value = false
		return
	}
	try {
		let props = await get_server_properties(props_serverId())
		if (props.length === 0) {
			// Server never ran: offer the standard defaults
			props = [
				['motd', 'A ChocoModrinth server'],
				['server-port', '25565'],
				['max-players', '20'],
				['difficulty', 'normal'],
				['gamemode', 'survival'],
				['view-distance', '10'],
				['simulation-distance', '10'],
				['level-name', 'world'],
				['spawn-protection', '16'],
				['pvp', 'true'],
				['online-mode', 'true'],
				['white-list', 'false'],
				['allow-flight', 'false'],
				['force-gamemode', 'false'],
				['enable-command-block', 'false'],
			]
		}
		entries.value = props
	} catch (error) {
		handleError(asError(error))
	} finally {
		loading.value = false
	}
})

function props_serverId(): string {
	return props.server.id
}

async function save() {
	// Demo properties are not persisted anywhere; just show "Saved."
	if (isDemoId(props_serverId())) {
		saved.value = true
		return
	}
	saving.value = true
	try {
		await set_server_properties(props.server.id, entries.value)
		saved.value = true
	} catch (error) {
		handleError(asError(error))
	} finally {
		saving.value = false
	}
}
</script>

<template>
	<div class="flex flex-col gap-4">
		<div class="flex items-center justify-between gap-3">
			<div>
				<h2 class="m-0 text-lg font-semibold text-contrast">server.properties</h2>
				<p class="m-0 text-xs text-secondary">
					Changes take effect after the server restarts.
					<span v-if="saved" class="ml-1 font-semibold text-green">Saved.</span>
				</p>
			</div>
			<Button color="brand" size="lg" :loading="saving" :disabled="loading" @click="save">
				Save properties
			</Button>
		</div>

		<p v-if="loading" class="m-0 text-sm text-secondary">Loading…</p>

		<div v-else class="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
			<div
				v-for="field in knownFields"
				:key="field.key"
				class="flex flex-col gap-1 rounded-xl bg-surface-2 p-3"
			>
				<span class="text-sm font-semibold text-contrast">{{ field.label }}</span>
				<select
					v-if="field.type === 'select'"
					:value="valueOf(field.key)"
					class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
					@change="setValue(field.key, ($event.target as HTMLSelectElement).value)"
				>
					<option v-for="option in field.options" :key="option" :value="option">
						{{ option }}
					</option>
				</select>
				<div v-else-if="field.type === 'toggle'" class="flex items-center gap-2">
					<button
						type="button"
						class="h-6 w-11 rounded-full border-0 p-0 transition-colors"
						:class="valueOf(field.key) === 'true' ? 'bg-brand' : 'bg-surface-5'"
						@click="toggleValue(field.key)"
					>
						<span
							class="mx-0.5 block h-5 w-5 rounded-full bg-white transition-transform"
							:class="valueOf(field.key) === 'true' ? 'translate-x-5' : ''"
						/>
					</button>
					<span class="text-sm text-secondary">{{ valueOf(field.key) }}</span>
				</div>
				<input
					v-else
					:type="field.type === 'number' ? 'number' : 'text'"
					:value="valueOf(field.key)"
					class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
					@input="setValue(field.key, ($event.target as HTMLInputElement).value)"
				/>
			</div>
		</div>
	</div>
</template>
