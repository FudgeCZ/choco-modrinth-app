<script setup lang="ts">
import {
	ArchiveIcon,
	FolderOpenIcon,
	PlayIcon,
	PlusIcon,
	SpinnerIcon,
	StopCircleIcon,
	TrashIcon,
} from '@modrinth/assets'
import {
	Button,
	Chips,
	ConfirmModal,
	injectNotificationManager,
	NewModal,
	Slider,
	Toggle,
} from '@modrinth/ui'
import { computed, onMounted, ref, useTemplateRef, watch } from 'vue'

import {
	type ChocoServer,
	create_server,
	delete_server,
	type CreateServerOptions,
	type ServerLoader,
	loader_versions,
	list_servers,
	minecraft_versions,
	type ServerLoaderVersion,
	init_server_listeners,
	is_server_running,
	runningServers,
	run_server,
	serverConsoleLines,
	stop_server,
	open_server_folder,
} from '@/helpers/servers'

const { handleError, pushNotification } = injectNotificationManager()
const servers = ref<ChocoServer[]>([])
const loading = ref(true)

const loaderLabels: Record<ServerLoader, string> = {
	vanilla: 'Vanilla',
	fabric: 'Fabric',
	forge: 'Forge',
	neoforge: 'NeoForge',
	quilt: 'Quilt',
	paper: 'Paper',
	purpur: 'Purpur',
}

const createModal = useTemplateRef('createModal')
const confirmDeleteModal = useTemplateRef('confirmDeleteModal')
const deleteServerId = ref<string | null>(null)

// Create form state
const creating = ref(false)
const name = ref('')
const gameVersion = ref('')
const selectedLoader = ref<ServerLoader>('vanilla')
const loaderVersion = ref('')
const ramMb = ref(4096)
const port = ref(25565)
const motd = ref('A ChocoModrinth server')
const difficulty = ref('normal')
const gamemode = ref('survival')
const maxPlayers = ref(20)
const onlineMode = ref(true)
const acceptEula = ref(false)

const mcVersions = ref<string[]>([])
const loaderVersionOptions = ref<ServerLoaderVersion[]>([])
const loadingLoaderVersions = ref(false)

const canCreate = computed(
	() =>
		name.value.trim().length > 0 &&
		gameVersion.value.length > 0 &&
		acceptEula.value &&
		!creating.value,
)

onMounted(async () => {
	await init_server_listeners()
	await refresh()
	try {
		mcVersions.value = await minecraft_versions()
		if (mcVersions.value.length > 0) {
			gameVersion.value = mcVersions.value[0]
		}
	} catch (error) {
		handleError(error)
	}
	await refreshRunningStates()
})

async function refresh() {
	loading.value = true
	try {
		servers.value = await list_servers()
	} catch (error) {
		handleError(error)
	} finally {
		loading.value = false
	}
}

async function refreshRunningStates() {
	for (const server of servers.value) {
		try {
			runningServers.value[server.id] = await is_server_running(server.id)
		} catch {
			runningServers.value[server.id] = false
		}
	}
}

watch([gameVersion, selectedLoader], async () => {
	loaderVersion.value = ''
	loaderVersionOptions.value = []
	if (selectedLoader.value === 'vanilla' || !gameVersion.value) return
	loadingLoaderVersions.value = true
	try {
		loaderVersionOptions.value = await loader_versions(
			selectedLoader.value,
			gameVersion.value,
		)
		if (loaderVersionOptions.value.length > 0) {
			loaderVersion.value = loaderVersionOptions.value[0].id
		}
	} catch {
		loaderVersionOptions.value = []
	} finally {
		loadingLoaderVersions.value = false
	}
})

async function submitCreate() {
	creating.value = true
	const notification = pushNotification({
		title: 'Creating server',
		text: `Downloading ${loaderLabels[selectedLoader.value]} ${gameVersion.value}...`,
		loading: true,
	})
	try {
		const options: CreateServerOptions = {
			name: name.value.trim(),
			game_version: gameVersion.value,
			loader: selectedLoader.value,
			loader_version: loaderVersion.value || null,
			ram_mb: ramMb.value,
			port: port.value,
			motd: motd.value,
			difficulty: difficulty.value,
			gamemode: gamemode.value,
			max_players: maxPlayers.value,
			online_mode: onlineMode.value,
			accept_eula: acceptEula.value,
		}
		await create_server(options)
		notification.update({
			title: 'Server created',
			text: `${options.name} is ready to play.`,
			loading: false,
			type: 'success',
		})
		createModal.value?.hide()
		name.value = ''
		acceptEula.value = false
		await refresh()
	} catch (error) {
		notification.hide()
		handleError(error)
	} finally {
		creating.value = false
	}
}

async function toggleRun(server: ChocoServer) {
	try {
		if (runningServers.value[server.id]) {
			await stop_server(server.id)
		} else {
			await run_server(server.id)
			runningServers.value[server.id] = true
		}
	} catch (error) {
		handleError(error)
	}
}

async function openFolder(server: ChocoServer) {
	try {
		await open_server_folder(server.id)
	} catch (error) {
		handleError(error)
	}
}

function confirmDelete(server: ChocoServer) {
	deleteServerId.value = server.id
	confirmDeleteModal.value?.show()
}

async function doDelete() {
	if (!deleteServerId.value) return
	try {
		await delete_server(deleteServerId.value)
		await refresh()
	} catch (error) {
		handleError(error)
	} finally {
		deleteServerId.value = null
	}
}

function formatConsole(serverId: string): string {
	return (serverConsoleLines.value[serverId] ?? []).join('\n')
}
</script>

<template>
	<div class="mx-auto flex flex-col gap-4 px-8 pt-8 pb-12">
		<div class="flex items-center justify-between">
			<div>
				<h1 class="m-0 text-2xl font-bold text-contrast">Servers</h1>
				<p class="m-0 text-secondary">
					Create and run your own local Minecraft servers.
				</p>
			</div>
			<Button
				color="brand"
				class="!mt-0"
				@click="createModal?.show()"
			>
				<PlusIcon aria-hidden="true" />
				Create server
			</Button>
		</div>

		<div v-if="loading" class="flex justify-center py-16 text-secondary">
			<SpinnerIcon class="animate-spin size-8" />
		</div>

		<div v-else-if="servers.length === 0" class="flex flex-col items-center gap-3 py-16">
			<ArchiveIcon class="size-12 text-secondary" />
			<p class="m-0 text-lg font-semibold text-contrast">No servers yet</p>
			<p class="m-0 max-w-md text-center text-secondary">
				Create a server and ChocoModrinth will automatically download the server
				jar, set up the EULA, server.properties and start scripts for you.
			</p>
			<Button color="brand" @click="createModal?.show()">
				<PlusIcon aria-hidden="true" />
				Create your first server
			</Button>
		</div>

		<div v-else class="flex flex-col gap-3">
			<div
				v-for="server in servers"
				:key="server.id"
				class="rounded-2xl border-0 border-solid border-divider bg-surface-2 p-4"
			>
				<div class="flex flex-wrap items-center gap-3">
					<div class="flex min-w-0 flex-1 flex-col gap-1">
						<p class="m-0 truncate text-lg font-bold text-contrast">
							{{ server.name }}
						</p>
						<p class="m-0 text-sm text-secondary">
							{{ loaderLabels[server.loader] }} {{ server.game_version }}
							<template v-if="server.loader_version"> · {{ server.loader_version }}</template>
							· {{ server.ram_mb }} MB RAM · port {{ server.port }}
						</p>
					</div>
					<div class="flex items-center gap-2">
						<Button
							:color="runningServers[server.id] ? 'red' : 'brand'"
							:disabled="!server.eula_accepted"
							:title="server.eula_accepted ? undefined : 'EULA not accepted'"
							@click="toggleRun(server)"
						>
							<StopCircleIcon v-if="runningServers[server.id]" aria-hidden="true" />
							<PlayIcon v-else aria-hidden="true" />
							{{ runningServers[server.id] ? 'Stop' : 'Run' }}
						</Button>
						<Button icon-only @click="openFolder(server)">
							<FolderOpenIcon aria-hidden="true" />
						</Button>
						<Button icon-only color="red" @click="confirmDelete(server)">
							<TrashIcon aria-hidden="true" />
						</Button>
					</div>
				</div>
				<pre
					v-if="serverConsoleLines[server.id]?.length"
					class="mt-3 max-h-40 overflow-auto rounded-xl bg-surface-4 p-3 text-xs whitespace-pre-wrap text-secondary"
					>{{ formatConsole(server.id) }}</pre
				>
			</div>
		</div>

		<NewModal ref="createModal" header="Create a server">
			<div class="flex flex-col gap-4">
				<div>
					<label class="mb-1 block font-semibold text-contrast" for="server-name">Name</label>
					<input
						id="server-name"
						v-model="name"
						class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						placeholder="My server"
					/>
				</div>

				<div>
					<label class="mb-1 block font-semibold text-contrast" for="server-mc-version">
						Minecraft version
					</label>
					<select
						id="server-mc-version"
						v-model="gameVersion"
						class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
					>
						<option v-for="version in mcVersions" :key="version" :value="version">
							{{ version }}
						</option>
					</select>
				</div>

				<div>
					<span class="mb-1 block font-semibold text-contrast">Server loader</span>
					<Chips
						v-model="selectedLoader"
						:items="Object.keys(loaderLabels) as ServerLoader[]"
						:format-label="(item: ServerLoader) => loaderLabels[item]"
					/>
				</div>

				<div v-if="selectedLoader !== 'vanilla'">
					<label class="mb-1 block font-semibold text-contrast" for="server-loader-version">
						{{ loaderLabels[selectedLoader] }} version
					</label>
					<select
						id="server-loader-version"
						v-model="loaderVersion"
						class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						:disabled="loadingLoaderVersions || loaderVersionOptions.length === 0"
					>
						<option v-if="loadingLoaderVersions" value="">Loading...</option>
						<option v-else-if="loaderVersionOptions.length === 0" value="">
							Not available for this version
						</option>
						<option v-for="version in loaderVersionOptions" :key="version.id" :value="version.id">
							{{ version.id }}{{ version.recommended ? ' (recommended)' : '' }}
						</option>
					</select>
				</div>

				<div>
					<span class="mb-1 block font-semibold text-contrast">
						RAM: {{ (ramMb / 1024).toFixed(0) }} GB
					</span>
					<Slider v-model="ramMb" :min="1024" :max="16384" :step="1024" />
				</div>

				<div class="grid grid-cols-2 gap-3">
					<div>
						<label class="mb-1 block font-semibold text-contrast" for="server-port">Port</label>
						<input
							id="server-port"
							v-model.number="port"
							type="number"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						/>
					</div>
					<div>
						<label class="mb-1 block font-semibold text-contrast" for="server-max-players">
							Max players
						</label>
						<input
							id="server-max-players"
							v-model.number="maxPlayers"
							type="number"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						/>
					</div>
					<div>
						<label class="mb-1 block font-semibold text-contrast" for="server-motd">MOTD</label>
						<input
							id="server-motd"
							v-model="motd"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						/>
					</div>
					<div>
						<label class="mb-1 block font-semibold text-contrast" for="server-difficulty">
							Difficulty
						</label>
						<select
							id="server-difficulty"
							v-model="difficulty"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						>
							<option>peaceful</option>
							<option>easy</option>
							<option>normal</option>
							<option>hard</option>
						</select>
					</div>
					<div>
						<label class="mb-1 block font-semibold text-contrast" for="server-gamemode">
							Game mode
						</label>
						<select
							id="server-gamemode"
							v-model="gamemode"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						>
							<option>survival</option>
							<option>creative</option>
							<option>adventure</option>
							<option>spectator</option>
						</select>
					</div>
					<div class="flex items-end">
						<Toggle v-model="onlineMode" />
						<span class="ml-2 text-contrast">Online mode (require premium accounts)</span>
					</div>
				</div>

				<div class="flex items-start gap-2 rounded-xl bg-surface-3 p-3">
					<Toggle v-model="acceptEula" />
					<span class="text-sm text-secondary">
						I agree to the Minecraft
						<a
							href="https://aka.ms/MinecraftEULA"
							target="_blank"
							class="text-brand underline"
							rel="noopener noreferrer"
							>EULA</a
						>. The server cannot start without accepting it.
					</span>
				</div>

				<Button color="brand" :disabled="!canCreate" :loading="creating" @click="submitCreate">
					<PlusIcon aria-hidden="true" />
					Create server
				</Button>
			</div>
		</NewModal>

		<ConfirmModal
			ref="confirmDeleteModal"
			title="Delete this server?"
			description="The server folder and all its files will be permanently deleted. This cannot be undone."
			:has-to-type="false"
			proceed-label="Delete"
			@proceed="doDelete"
		/>
	</div>
</template>
