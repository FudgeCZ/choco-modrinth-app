<script setup lang="ts">
import {
	ArchiveIcon,
	EyeIcon,
	EyeOffIcon,
	FolderOpenIcon,
	PlayIcon,
	PlusIcon,
	SpinnerIcon,
	StopCircleIcon,
	TrashIcon,
	MoreVerticalIcon,
} from '@modrinth/assets'
import {
	Button,
	Chips,
	Combobox,
	ConfirmModal,
	injectNotificationManager,
	TeleportOverflowMenu,
	NewModal,
	Slider,
	Toggle,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'
import { computed, defineComponent, h, onMounted, ref, useTemplateRef, watch } from 'vue'

import {
	type ChocoServer,
	accept_server_eula,
	create_server,
	create_server_from_profile,
	delete_server,
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
	sync_profile_content,
} from '@/helpers/servers'
import type { ButtonMenuOption } from '@modrinth/ui'
import type { GameInstance } from '@/helpers/types'
import { getInstanceIconUrl, list as list_instances } from '@/helpers/instance'
import { get_instance_worlds } from '@/helpers/worlds'
import { progress_bars_list, type LoadingBar } from '@/helpers/state'
import { useAppEvent } from '@/composables/use-app-event'

const { handleError, pushNotification } = injectNotificationManager()

const servers = ref<ChocoServer[]>([])
const loading = ref(true)
const highlightId = ref<string | null>(null)

type CreatingServer = {
	key: string
	name: string
	loaderLabel: string
	gameVersion: string
	message: string
	progress: number | null
}

const creatingServers = ref<CreatingServer[]>([])

// Server setup reports its phases via loading bars with an empty instance id
// (they also feed the downloads notification); mirror them onto the grid card.
function updateCreatingProgress(bars: Record<string, LoadingBar>) {
	for (const entry of creatingServers.value) {
		const bar = Object.values(bars).find(
			(candidate) =>
				candidate.bar_type?.type === 'zip_extract' &&
				!candidate.bar_type?.instance_id &&
				candidate.bar_type?.instance_name === entry.name,
		)
		if (bar) {
			entry.message = bar.message ?? entry.message
			const current = bar.current ?? 0
			const total = bar.total ?? 0
			entry.progress = total > 0 ? Math.max(0, Math.min(1, current / total)) : null
		}
	}
}

useAppEvent('loading', async () => {
	const bars = await progress_bars_list().catch(() => ({}))
	updateCreatingProgress(bars)
})

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

// Tab switch: create from scratch or from an existing profile
const creationTab = ref<'scratch' | 'profile'>('scratch')
const tabItems = ['scratch', 'profile'] as const

// From-scratch form state
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
const difficultyLabels: Record<string, string> = {
	peaceful: 'Peaceful',
	easy: 'Easy',
	normal: 'Normal',
	hard: 'Hard',
}
const gamemodeLabels: Record<string, string> = {
	survival: 'Survival',
	creative: 'Creative',
	adventure: 'Adventure',
	spectator: 'Spectator',
}
const maxPlayers = ref(20)
const onlineMode = ref(true)
const acceptEula = ref(false)

// From-profile form state
const profiles = ref<GameInstance[]>([])
const selectedProfileId = ref<string | null>(null)
const selectedSave = ref<string | null>(null)
const profileSaves = ref<string[]>([])
const loadingSaves = ref(false)
const copyMods = ref(true)
const copyConfig = ref(true)

const mcVersions = ref<string[]>([])
const showAllVersions = ref(false)
const loaderVersionOptions = ref<ServerLoaderVersion[]>([])
const loadingLoaderVersions = ref(false)

const selectedProfile = computed(() =>
	profiles.value.find((p) => p.id === selectedProfileId.value),
)

// Combobox options take an icon component rather than an image URL, so wrap
// each profile's icon in a tiny <img> component.
function profileIconComponent(iconPath: string | null | undefined) {
	const src = getInstanceIconUrl(iconPath)
	if (!src) return undefined
	return defineComponent({
		name: 'ProfileOptionIcon',
		render() {
			return h('img', {
				src,
				alt: '',
				class: 'h-5 w-5 shrink-0 rounded object-cover',
			})
		},
	})
}

const profileOptions = computed(() =>
	profiles.value.map((p) => ({
		value: p.id,
		label: p.name,
		subLabel: `${p.loader} ${p.game_version}`,
		icon: profileIconComponent(p.icon_path),
	})),
)

const displayedVersions = computed(() => {
	const versions = mcVersions.value
	return showAllVersions.value ? versions : versions.slice(0, 12)
})

const canCreateScratch = computed(
	() =>
		name.value.trim().length > 0 &&
		gameVersion.value.length > 0 &&
		acceptEula.value &&
		!creating.value,
)

const canCreateFromProfile = computed(
	() =>
		!!selectedProfileId.value &&
		acceptEula.value &&
		!creating.value &&
		!!selectedProfile.value,
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
	try {
		profiles.value = await list_instances()
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

watch(selectedProfileId, async () => {
	selectedSave.value = null
	profileSaves.value = []
	if (!selectedProfileId.value) return
	loadingSaves.value = true
	try {
		const worlds = await get_instance_worlds(selectedProfileId.value)
		profileSaves.value = worlds
			.filter((w) => w.type === 'singleplayer')
			.map((w) => w.name)
	} catch {
		profileSaves.value = []
	} finally {
		loadingSaves.value = false
	}
})

async function submitCreate() {
	creating.value = true
	const entry: CreatingServer = {
		key: `create-${Date.now()}`,
		name: name.value.trim() || 'New server',
		loaderLabel: loaderLabels[selectedLoader.value],
		gameVersion: gameVersion.value,
		message: 'Preparing...',
		progress: null,
	}
	// Snapshot options before resetting the form fields below
	const options = {
		name: entry.name,
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
	creatingServers.value.push(entry)
	createModal.value?.hide()
	name.value = ''
	acceptEula.value = false
	try {
		const server = await create_server(options)
		highlightId.value = server.id
		await refresh()
	} catch (error) {
		handleError(error)
	} finally {
		creatingServers.value = creatingServers.value.filter((c) => c.key !== entry.key)
		creating.value = false
	}
}

async function submitCreateFromProfile() {
	if (!selectedProfile.value) return
	creating.value = true
	const entry: CreatingServer = {
		key: `create-${Date.now()}`,
		name: `${selectedProfile.value.name} Server`,
		loaderLabel: loaderLabels[selectedProfile.value.loader as ServerLoader],
		gameVersion: selectedProfile.value.game_version ?? '',
		message: 'Preparing...',
		progress: null,
	}
	// Snapshot options before resetting the form fields below
	const options = {
		instanceId: selectedProfile.value.id,
		saveName: selectedSave.value,
		copyMods: copyMods.value,
		copyConfig: copyConfig.value,
		acceptEula: acceptEula.value,
		ramMb: ramMb.value,
		port: port.value,
	}
	creatingServers.value.push(entry)
	createModal.value?.hide()
	acceptEula.value = false
	try {
		const server = await create_server_from_profile(options)
		highlightId.value = server.id
		runningServers.value[server.id] = false
		await refresh()
	} catch (error) {
		handleError(error)
	} finally {
		creatingServers.value = creatingServers.value.filter((c) => c.key !== entry.key)
		creating.value = false
	}
}

async function syncServer(server: ChocoServer) {
	const notification = pushNotification({
		title: 'Syncing from profile',
		text: `Updating ${server.name} with the profile's mods and config...`,
		loading: true,
	})
	try {
		await sync_profile_content(server.id)
		notification.update({
			title: 'Server synced',
			text: `${server.name} now matches the profile's mods and config.`,
			loading: false,
			type: 'success',
		})
	} catch (error) {
		notification.hide()
		handleError(error)
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

function serverIconUrl(server: ChocoServer): string | null {
	return server.icon_file ? convertFileSrc(server.icon_file) : null
}

const router = useRouter()

function openDashboardPage(server: ChocoServer) {
	router.push(`/server/${server.id}`)
}

function serverMenuOptions(server: ChocoServer): ButtonMenuOption[] {
	const options: ButtonMenuOption[] = [
		{
			id: 'open-folder',
			label: 'Open folder',
			icon: FolderOpenIcon,
			action: () => void openFolder(server),
		},
	]
	if (server.linked_instance_id) {
		options.push({
			id: 'sync',
			label: 'Sync from profile',
			action: () => void syncServer(server),
		})
	}
	if (!server.eula_accepted) {
		options.push({
			id: 'accept-eula',
			label: 'Accept EULA',
			action: () => void acceptEulaFor(server),
		})
	}
	options.push({
		id: 'delete',
		label: 'Delete',
		icon: TrashIcon,
		tone: 'red',
		action: () => confirmDelete(server),
	})
	return options
}

async function acceptEulaFor(server: ChocoServer) {
	try {
		await accept_server_eula(server.id)
		await refresh()
		if (dashboardServer.value?.id === server.id) {
			dashboardServer.value = { ...dashboardServer.value, eula_accepted: true }
		}
	} catch (error) {
		handleError(error)
	}
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
			<Button color="brand" class="!mt-0" @click="createModal?.show()">
				<PlusIcon aria-hidden="true" />
				Create server
			</Button>
		</div>

		<div v-if="loading" class="flex justify-center py-16 text-secondary">
			<SpinnerIcon class="animate-spin size-8" />
		</div>

		<div
			v-else-if="servers.length === 0 && creatingServers.length === 0"
			class="flex flex-col items-center gap-3 py-16"
		>
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

		<div
			v-else
			class="grid items-stretch gap-3"
			style="grid-template-columns: repeat(auto-fill, minmax(180px, 1fr))"
		>
			<div
				v-for="entry in creatingServers"
				:key="entry.key"
				class="flex flex-col overflow-hidden rounded-2xl border-0 border-solid border-divider bg-surface-2"
			>
				<div
					class="flex aspect-square w-full items-center justify-center bg-surface-3"
				>
					<SpinnerIcon class="size-10 animate-spin text-brand" />
				</div>
				<div class="flex flex-col gap-1 p-2.5">
					<p class="m-0 truncate font-bold text-contrast">
						{{ entry.name }}
					</p>
					<p class="m-0 truncate text-xs text-secondary">
						{{ entry.loaderLabel }} {{ entry.gameVersion }}
					</p>
					<div class="h-1.5 w-full overflow-hidden rounded-full bg-surface-5">
						<div
							class="h-full rounded-full bg-brand transition-[width] duration-300"
							:class="entry.progress == null ? 'w-2/5 animate-pulse' : ''"
							:style="
								entry.progress != null
									? { width: `${Math.round(entry.progress * 100)}%` }
									: undefined
							"
						/>
					</div>
					<p class="m-0 truncate text-xs text-secondary">{{ entry.message }}</p>
				</div>
			</div>

			<div
				v-for="server in servers"
				:key="server.id"
				class="flex cursor-pointer flex-col overflow-hidden rounded-2xl border-0 border-solid transition-colors bg-surface-2 hover:bg-surface-3"
				:class="
					highlightId === server.id
						? 'border-2 border-brand shadow-[0_0_12px_var(--color-brand-shadow)]'
						: 'border-divider'
				"
				@click="openDashboardPage(server)"
			>
				<div class="relative">
					<div
						class="flex aspect-square w-full items-center justify-center bg-surface-4"
					>
						<img
							v-if="server.icon_file"
							:src="serverIconUrl(server)"
							:alt="server.name"
							class="size-full object-cover"
						/>
						<ArchiveIcon v-else class="size-12 text-secondary" />
					</div>
					<div
						v-if="runningServers[server.id]"
						class="absolute top-1.5 left-1.5 flex items-center gap-1 rounded-full bg-black/60 px-2 py-0.5 text-xs font-semibold text-green"
					>
						<span class="size-1.5 rounded-full bg-green" />
						Running
					</div>
					<div class="absolute top-1.5 right-1.5" @click.stop>
						<TeleportOverflowMenu
							icon-only
							size="sm"
							label="Server actions"
							:options="serverMenuOptions(server)"
						>
							<MoreVerticalIcon aria-hidden="true" />
						</TeleportOverflowMenu>
					</div>
				</div>
				<div class="flex flex-col gap-0.5 p-2.5">
					<p class="m-0 truncate font-bold text-contrast">
						{{ server.name }}
					</p>
					<p class="m-0 truncate text-xs text-secondary">
						{{ loaderLabels[server.loader] }} {{ server.game_version }}
					</p>
				</div>
				<div class="mt-auto p-2 pt-0" @click.stop>
					<Button
						v-if="!server.eula_accepted"
						class="w-full"
						@click="acceptEulaFor(server)"
					>
						Accept EULA
					</Button>
					<Button
						v-else-if="runningServers[server.id]"
						color="red"
						class="w-full"
						@click="toggleRun(server)"
					>
						<StopCircleIcon aria-hidden="true" />
						Stop
					</Button>
					<Button
						v-else
						color="brand"
						class="w-full"
						@click="toggleRun(server)"
					>
						<PlayIcon aria-hidden="true" />
						Start
					</Button>
				</div>
			</div>
		</div>

		<NewModal ref="createModal" header="Create a server">
			<div class="flex min-h-[24rem] flex-col gap-4">
				<Chips
					v-model="creationTab"
					:items="tabItems"
					:format-label="(item: 'scratch' | 'profile') => item === 'scratch' ? 'From scratch' : 'From profile'"
				/>

				<template v-if="creationTab === 'scratch'">
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
						<span class="mb-1 block font-semibold text-contrast">Server loader</span>
						<Chips
							v-model="selectedLoader"
							:items="Object.keys(loaderLabels) as ServerLoader[]"
							:format-label="(item: ServerLoader) => loaderLabels[item]"
						/>
					</div>

					<div>
						<span class="mb-1 block font-semibold text-contrast">Minecraft version</span>
						<Combobox
							v-model="gameVersion"
							:options="displayedVersions.map((v) => ({ value: v, label: v }))"
							:display-value="gameVersion || 'Select version'"
							:searchable="true"
							search-placeholder="Search versions..."
						>
							<template #dropdown-footer>
								<button
									class="flex w-full cursor-pointer items-center justify-center gap-1.5 border-0 border-t border-solid border-surface-5 bg-transparent py-3 text-center text-sm font-semibold text-secondary transition-colors hover:text-contrast"
									@mousedown.prevent
									@click="showAllVersions = !showAllVersions"
								>
									<EyeOffIcon v-if="showAllVersions" class="size-4" />
									<EyeIcon v-else class="size-4" />
									{{ showAllVersions ? 'Hide extra versions' : 'Show all versions' }}
								</button>
							</template>
						</Combobox>
					</div>

					<div v-if="selectedLoader !== 'vanilla'">
						<span class="mb-1 block font-semibold text-contrast">
							{{ loaderLabels[selectedLoader] }} version
						</span>
						<Combobox
							v-model="loaderVersion"
							:options="loaderVersionOptions.map((v) => ({ value: v.id, label: v.recommended ? `${v.id} (recommended)` : v.id }))"
							:display-value="loaderVersion || 'Select version'"
							:disabled="loadingLoaderVersions || loaderVersionOptions.length === 0"
							:searchable="loaderVersionOptions.length > 8"
						/>
						<span v-if="!loadingLoaderVersions && loaderVersionOptions.length === 0" class="text-xs text-secondary">
							Not available for this Minecraft version.
						</span>
					</div>
				</template>

				<template v-else>
					<div>
						<span class="mb-1 block font-semibold text-contrast">Profile</span>
						<Combobox
							v-model="selectedProfileId"
							:options="profileOptions"
							:display-value="selectedProfile?.name ?? 'Select a profile'"
							:searchable="profiles.length > 8"
							placeholder="Select a profile"
							show-icon-in-selected
						/>
						<p v-if="selectedProfile" class="m-0 mt-1 text-xs text-secondary">
							Server inherits {{ loaderLabels[selectedProfile.loader] }}
							{{ selectedProfile.game_version }} and is named
							"{{ selectedProfile.name }} Server".
						</p>
					</div>

					<div>
						<span class="mb-1 block font-semibold text-contrast">World (save)</span>
						<Combobox
							v-model="selectedSave"
							:options="profileSaves.map((s) => ({ value: s, label: s }))"
							:display-value="selectedSave ?? (loadingSaves ? 'Loading saves...' : 'No save (empty world)')"
							:disabled="loadingSaves || profileSaves.length === 0"
							:clearable="true"
						/>
						<p class="m-0 mt-1 text-xs text-secondary">
							Optional: pick a singleplayer save to use as the server world. The
							save is copied, not moved.
						</p>
					</div>

					<div class="flex flex-col gap-2 rounded-xl bg-surface-3 p-3">
						<div class="flex items-center gap-2">
							<Toggle v-model="copyMods" />
							<span class="text-contrast">Copy mods</span>
						</div>
						<p class="m-0 text-xs text-secondary">
							Client-only mods (marked unsupported on the server by Modrinth) are
							excluded automatically.
						</p>
						<div class="flex items-center gap-2">
							<Toggle v-model="copyConfig" />
							<span class="text-contrast">Copy config folder</span>
						</div>
					</div>
				</template>

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
						<label class="mb-1 block font-semibold text-contrast" for="server-motd">MOTD</label>
						<input
							v-if="creationTab === 'scratch'"
							id="server-motd"
							v-model="motd"
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-contrast"
						/>
						<input
							v-else
							:value="selectedSave ?? selectedProfile?.name ?? ''"
							disabled
							class="w-full rounded-xl border-0 border-solid border-divider bg-surface-3 px-3 py-2 text-secondary"
						/>
					</div>
					<div>
						<span class="mb-1 block font-semibold text-contrast">Difficulty</span>
						<Combobox
							v-model="difficulty"
							:options="[
								{ value: 'peaceful', label: 'Peaceful' },
								{ value: 'easy', label: 'Easy' },
								{ value: 'normal', label: 'Normal' },
								{ value: 'hard', label: 'Hard' },
							]"
							:display-value="difficultyLabels[difficulty] ?? difficulty"
						/>
					</div>
					<div>
						<span class="mb-1 block font-semibold text-contrast">Game mode</span>
						<Combobox
							v-model="gamemode"
							:options="[
								{ value: 'survival', label: 'Survival' },
								{ value: 'creative', label: 'Creative' },
								{ value: 'adventure', label: 'Adventure' },
								{ value: 'spectator', label: 'Spectator' },
							]"
							:display-value="gamemodeLabels[gamemode] ?? gamemode"
						/>
					</div>
				</div>

				<div class="flex items-center gap-2">
					<Toggle v-model="onlineMode" />
					<span class="text-contrast">Online mode (require premium accounts)</span>
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

				<Button
					color="brand"
					:disabled="creationTab === 'scratch' ? !canCreateScratch : !canCreateFromProfile"
					:loading="creating"
					@click="creationTab === 'scratch' ? submitCreate() : submitCreateFromProfile()"
				>
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
