<script setup lang="ts">
import {
	Button,
	Checkbox,
	Combobox,
	injectNotificationManager,
	NewModal,
	Slider,
	Toggle,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { readFile } from '@tauri-apps/plugin-fs'
import { computed, ref, useTemplateRef } from 'vue'

import {
	change_server_loader_version,
	type ChocoServer,
	delete_server,
	loader_versions,
	runningServers,
	type ServerLoaderVersion,
	type ServerSettingsUpdate,
	set_server_icon,
	update_server,
} from '@/helpers/servers'

const emit = defineEmits<{
	saved: [server: ChocoServer]
	deleted: []
}>()

const { handleError } = injectNotificationManager()

const modal = useTemplateRef('modal')

// The server whose settings are being edited (set in show()).
const server = ref<ChocoServer | null>(null)

// Editable copies of the server's settings.
const name = ref('')
const ramMb = ref(4096)
const port = ref(25565)
const motd = ref('')
const difficulty = ref('normal')
const gamemode = ref('survival')
const maxPlayers = ref(20)
const onlineMode = ref(true)

const difficultyOptions = [
	{ value: 'peaceful', label: 'Peaceful' },
	{ value: 'easy', label: 'Easy' },
	{ value: 'normal', label: 'Normal' },
	{ value: 'hard', label: 'Hard' },
]
const difficultyLabels: Record<string, string> = {
	peaceful: 'Peaceful',
	easy: 'Easy',
	normal: 'Normal',
	hard: 'Hard',
}
const gamemodeOptions = [
	{ value: 'survival', label: 'Survival' },
	{ value: 'creative', label: 'Creative' },
	{ value: 'adventure', label: 'Adventure' },
	{ value: 'spectator', label: 'Spectator' },
]
const gamemodeLabels: Record<string, string> = {
	survival: 'Survival',
	creative: 'Creative',
	adventure: 'Adventure',
	spectator: 'Spectator',
}

// Icon: preview is either the freshly generated PNG data URL or the server's
// icon file on disk. Base64 is kept until save.
const iconPreview = ref<string | null>(null)
const pendingIconBase64 = ref<string | null>(null)
const uploadingIcon = ref(false)

// Loader version "Change version" flow.
const showLoaderChange = ref(false)
const loadingLoaderVersions = ref(false)
const loaderVersionOptions = ref<ServerLoaderVersion[]>([])
const pendingLoaderVersion = ref('')

// Danger zone.
const saveWorldToProfile = ref(true)
const saving = ref(false)
const deleting = ref(false)

const isRunning = computed(
	() => !!server.value && runningServers.value[server.value.id] === true,
)

const loaderChanged = computed(
	() =>
		!!pendingLoaderVersion.value &&
		pendingLoaderVersion.value !== (server.value?.loader_version ?? ''),
)

function show(original: ChocoServer) {
	server.value = original
	name.value = original.name
	ramMb.value = original.ram_mb
	port.value = original.port
	motd.value = ''
	difficulty.value = 'normal'
	gamemode.value = 'survival'
	maxPlayers.value = 20
	onlineMode.value = true
	iconPreview.value = original.icon_file
		? convertFileSrc(original.icon_file)
		: null
	pendingIconBase64.value = null
	showLoaderChange.value = false
	loaderVersionOptions.value = []
	pendingLoaderVersion.value = ''
	saveWorldToProfile.value = true
	modal.value?.show()
}

defineExpose({ show })

async function uploadIcon() {
	if (!server.value) return
	const selected = await open({
		multiple: false,
		directory: false,
		filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
	}).catch((error) => {
		handleError(error)
		return null
	})
	if (!selected) return
	const path = typeof selected === 'string' ? selected : selected.path
	if (!path) return

	uploadingIcon.value = true
	try {
		const bytes = await readFile(path)
		const dataUrl = await new Promise<string>((resolve, reject) => {
			const reader = new FileReader()
			reader.onload = () => resolve(reader.result as string)
			reader.onerror = () => reject(reader.error ?? new Error('Failed to read image'))
			reader.readAsDataURL(new Blob([bytes]))
		})

		const image = new Image()
		image.src = dataUrl
		await new Promise<void>((resolve, reject) => {
			image.onload = () => resolve()
			image.onerror = () => reject(new Error('Failed to decode image'))
		})

		const canvas = document.createElement('canvas')
		canvas.width = 64
		canvas.height = 64
		const context = canvas.getContext('2d')
		if (!context) throw new Error('Canvas is not supported')
		context.drawImage(image, 0, 0, 64, 64)

		const pngDataUrl = canvas.toDataURL('image/png')
		pendingIconBase64.value = pngDataUrl.replace(/^data:image\/png;base64,/, '')
		iconPreview.value = pngDataUrl
	} catch (error) {
		handleError(error)
	} finally {
		uploadingIcon.value = false
	}
}

async function beginLoaderChange() {
	if (!server.value) return
	showLoaderChange.value = true
	loadingLoaderVersions.value = true
	pendingLoaderVersion.value = ''
	try {
		loaderVersionOptions.value = await loader_versions(
			server.value.loader,
			server.value.game_version,
		)
	} catch (error) {
		loaderVersionOptions.value = []
		handleError(error)
	} finally {
		loadingLoaderVersions.value = false
	}
}

async function save() {
	if (!server.value) return
	saving.value = true
	try {
		const update: ServerSettingsUpdate = {
			name: name.value.trim() || server.value.name,
			ram_mb: ramMb.value,
			port: port.value,
			difficulty: difficulty.value,
			gamemode: gamemode.value,
			max_players: maxPlayers.value,
			online_mode: onlineMode.value,
		}
		if (motd.value.trim().length > 0) {
			update.motd = motd.value
		}
		const updated = await update_server(server.value.id, update)
		if (pendingIconBase64.value) {
			await set_server_icon(server.value.id, pendingIconBase64.value)
		}
		if (loaderChanged.value && pendingLoaderVersion.value) {
			await change_server_loader_version(server.value.id, pendingLoaderVersion.value)
		}
		emit('saved', updated)
		modal.value?.hide()
	} catch (error) {
		handleError(error)
	} finally {
		saving.value = false
	}
}

async function deleteThisServer() {
	if (!server.value) return
	deleting.value = true
	try {
		await delete_server(
			server.value.id,
			server.value.linked_instance_id ? saveWorldToProfile.value : false,
		)
		emit('deleted')
		modal.value?.hide()
	} catch (error) {
		handleError(error)
	} finally {
		deleting.value = false
	}
}
</script>

<template>
	<NewModal ref="modal" header="Server settings" max-width="600px" :scrollable="true">
		<div class="flex min-h-[24rem] flex-col gap-5">
			<p class="m-0 text-sm text-secondary">
				Changes are written to the server config. The server keeps running while you
				edit &mdash; some changes only apply after a restart.
			</p>

			<section class="flex flex-col gap-2">
				<h3 class="m-0 text-lg font-semibold text-contrast">General</h3>
				<div>
					<label class="mb-1 block font-semibold text-contrast" for="settings-server-name">
						Name
					</label>
					<input
						id="settings-server-name"
						v-model="name"
						class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						placeholder="My server"
					/>
				</div>
			</section>

			<section class="flex flex-col gap-2">
				<h3 class="m-0 text-lg font-semibold text-contrast">Hardware</h3>
				<div>
					<span class="mb-1 block font-semibold text-contrast">
						RAM: {{ (ramMb / 1024).toFixed(0) }} GB
					</span>
					<Slider v-model="ramMb" :min="1024" :max="16384" :step="1024" />
				</div>
			</section>

			<section class="flex flex-col gap-2">
				<h3 class="m-0 text-lg font-semibold text-contrast">Network</h3>
				<div class="grid grid-cols-2 gap-3">
					<div>
						<label class="mb-1 block font-semibold text-contrast" for="settings-server-port">
							Port
						</label>
						<input
							id="settings-server-port"
							v-model.number="port"
							type="number"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						/>
					</div>
					<div>
						<label class="mb-1 block font-semibold text-contrast" for="settings-server-motd">
							MOTD
						</label>
						<input
							id="settings-server-motd"
							v-model="motd"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
							placeholder="A ChocoModrinth server"
						/>
						<p class="m-0 mt-1 text-xs text-secondary">
							Leave blank to keep the current MOTD.
						</p>
					</div>
				</div>
			</section>

			<section class="flex flex-col gap-2">
				<h3 class="m-0 text-lg font-semibold text-contrast">Gameplay</h3>
				<div class="grid grid-cols-2 gap-3">
					<div>
						<span class="mb-1 block font-semibold text-contrast">Difficulty</span>
						<Combobox
							v-model="difficulty"
							:options="difficultyOptions"
							:display-value="difficultyLabels[difficulty] ?? difficulty"
						/>
					</div>
					<div>
						<span class="mb-1 block font-semibold text-contrast">Game mode</span>
						<Combobox
							v-model="gamemode"
							:options="gamemodeOptions"
							:display-value="gamemodeLabels[gamemode] ?? gamemode"
						/>
					</div>
					<div>
						<label class="mb-1 block font-semibold text-contrast" for="settings-max-players">
							Max players
						</label>
						<input
							id="settings-max-players"
							v-model.number="maxPlayers"
							type="number"
							min="1"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						/>
					</div>
					<div class="flex items-end pb-2">
						<div class="flex items-center gap-2">
							<Toggle v-model="onlineMode" />
							<span class="text-contrast">Online mode</span>
						</div>
					</div>
				</div>
			</section>

			<section class="flex flex-col gap-2">
				<h3 class="m-0 text-lg font-semibold text-contrast">Icon</h3>
				<div class="flex items-center gap-3">
					<img
						v-if="iconPreview"
						:src="iconPreview"
						:alt="server?.name ?? 'Server icon'"
						class="size-16 rounded-xl object-cover"
					/>
					<div
						v-else
						class="flex size-16 items-center justify-center rounded-xl bg-surface-3 text-secondary"
					>
						64&times;64
					</div>
					<Button :disabled="uploadingIcon" @click="uploadIcon">
						{{ uploadingIcon ? 'Processing...' : 'Upload icon' }}
					</Button>
				</div>
				<p class="m-0 text-xs text-secondary">
					The image is resized to 64&times;64 PNG, matching what Minecraft shows in the
					server list.
				</p>
			</section>

			<section class="flex flex-col gap-2">
				<h3 class="m-0 text-lg font-semibold text-contrast">Loader version</h3>
				<p class="m-0 text-sm text-secondary">
					Current version:
					<span class="font-semibold text-contrast">
						{{ server?.loader_version ?? 'None' }}
					</span>
				</p>
				<Button
					v-if="!showLoaderChange"
					class="w-fit"
					:disabled="isRunning"
					:title="isRunning ? 'Stop the server before reinstalling' : undefined"
					@click="beginLoaderChange"
				>
					Change version
				</Button>
				<template v-else>
					<Combobox
						v-model="pendingLoaderVersion"
						:options="
							loaderVersionOptions.map((v) => ({
								value: v.id,
								label: v.recommended ? `${v.id} (recommended)` : v.id,
							}))
						"
						:display-value="pendingLoaderVersion || (loadingLoaderVersions ? 'Loading versions...' : 'Select version')"
						:disabled="loadingLoaderVersions || loaderVersionOptions.length === 0"
						:searchable="loaderVersionOptions.length > 8"
					/>
					<p class="m-0 text-xs text-orange">
						Reinstalling downloads the server jar again &mdash; worlds are kept.
					</p>
					<p v-if="loaderVersionOptions.length === 0 && !loadingLoaderVersions" class="m-0 text-xs text-secondary">
						No versions available for {{ server?.loader }} {{ server?.game_version }}.
					</p>
				</template>
			</section>

			<section
				class="flex flex-col gap-3 rounded-xl border border-solid border-red bg-highlight-red p-4"
			>
				<h3 class="m-0 text-lg font-semibold text-red">Danger zone</h3>
				<div v-if="server?.linked_instance_id" class="flex flex-col gap-1">
					<Checkbox v-model="saveWorldToProfile" label="Save the world to the linked profile" />
					<p class="m-0 text-xs text-secondary">
						Moves the server's world folder into the linked profile's saves so you can
						keep playing it in singleplayer or re-create a server later.
					</p>
				</div>
				<div>
					<Button color="red" :loading="deleting" @click="deleteThisServer">Delete</Button>
					<p class="m-0 mt-1 text-xs text-secondary">
						Deletes the server folder and all its files. This cannot be undone.
					</p>
				</div>
			</section>
		</div>

		<template #actions>
			<div class="flex w-full items-center justify-end gap-2">
				<Button @click="modal?.hide()">Cancel</Button>
				<Button color="brand" :loading="saving" @click="save">Save changes</Button>
			</div>
		</template>
	</NewModal>
</template>
