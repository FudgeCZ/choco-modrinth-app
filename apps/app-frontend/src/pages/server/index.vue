<script setup lang="ts">
import {
	ArchiveIcon,
	BoxesIcon,
	FolderOpenIcon,
	GaugeIcon,
	PlayIcon,
	StopCircleIcon,
	TerminalSquareIcon,
	UsersIcon,
	WrenchIcon,
} from '@modrinth/assets'
import { Button, injectNotificationManager, NavTabs } from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, onMounted, ref, useTemplateRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { DEMO_SERVERS, isDemoId } from '@/helpers/demo-data'
import { demoIsRunning, demoSetRunning, ensureDemoRuntime } from '@/helpers/demo-runtime'
import {
	accept_server_eula,
	type ChocoServer,
	is_server_running,
	list_servers,
	run_server,
	stop_server,
} from '@/helpers/servers'
import { useBreadcrumb, useRootBreadcrumb } from '@/providers/breadcrumbs'

import ConsoleTab from './ConsoleTab.vue'
import ContentTab from './ContentTab.vue'
import DashboardTab from './DashboardTab.vue'
import PlayersTab from './PlayersTab.vue'
import PropertiesTab from './PropertiesTab.vue'
import SettingsModal from './SettingsModal.vue'

const route = useRoute()
const router = useRouter()
const { handleError } = injectNotificationManager()

const serverId = computed(() => route.params.id as string)
const server = ref<ChocoServer | null>(null)
const loading = ref(true)
const running = ref(false)
const restarting = ref(false)

const isDemoServer = computed(() => isDemoId(server.value?.id))

const activeTab = ref('dashboard')
const tabItems = ['dashboard', 'content', 'properties', 'console', 'players'] as const
const tabIcons: Record<string, unknown> = {
	dashboard: GaugeIcon,
	content: BoxesIcon,
	properties: WrenchIcon,
	console: TerminalSquareIcon,
	players: UsersIcon,
}
const tabLabels: Record<string, string> = {
	dashboard: 'Dashboard',
	content: 'Content',
	properties: 'Properties',
	console: 'Console',
	players: 'Players',
}

const tabLinks = computed(() =>
	tabItems.map((item) => ({
		label: tabLabels[item] ?? item,
		href: '#' + item,
		icon: tabIcons[item],
	})),
)
const tabIndex = computed(() => Math.max(0, tabItems.indexOf(activeTab.value)))

useRootBreadcrumb({
	slot: 'root',
	id: 'local-servers',
	label: 'Servers',
	to: '/local-servers',
})
useBreadcrumb({
	slot: 'server',
	id: () => serverId.value,
	label: () => server.value?.name ?? 'Server',
})

async function load() {
	loading.value = true
	try {
		if (isDemoId(serverId.value)) {
			// Demo servers come from the demo data, not the backend
			ensureDemoRuntime()
			server.value = DEMO_SERVERS.find((s) => s.id === serverId.value) ?? null
			running.value = server.value ? demoIsRunning(server.value.id) : false
			return
		}
		const servers = await list_servers()
		server.value = servers.find((s) => s.id === serverId.value) ?? null
		running.value = server.value
			? await is_server_running(server.value.id).catch(() => false)
			: false
	} catch (error) {
		handleError(error)
	} finally {
		loading.value = false
	}
}

onMounted(load)

watch(serverId, load)

// Demo stop/start updates the runtime state asynchronously (with a short
// simulated shutdown); mirror it into the page's running flag.
watch(
	() => (isDemoId(serverId.value) ? demoIsRunning(serverId.value) : false),
	(value) => {
		if (isDemoId(serverId.value)) running.value = value
	},
)

async function startServer() {
	if (!server.value) return
	if (isDemoId(server.value.id)) {
		demoSetRunning(server.value.id, true)
		running.value = true
		return
	}
	try {
		await run_server(server.value.id)
		running.value = true
	} catch (error) {
		handleError(error)
	}
}

async function stopServer() {
	if (!server.value) return
	if (isDemoId(server.value.id)) {
		demoSetRunning(server.value.id, false)
		return
	}
	try {
		await stop_server(server.value.id)
		running.value = false
	} catch (error) {
		handleError(error)
	}
}

async function restartServer() {
	if (!server.value || restarting.value) return
	if (isDemoId(server.value.id)) {
		const id = server.value.id
		restarting.value = true
		demoSetRunning(id, false)
		await new Promise((resolve) => setTimeout(resolve, 1600))
		demoSetRunning(id, true)
		running.value = true
		restarting.value = false
		return
	}
	restarting.value = true
	try {
		await stop_server(server.value.id)
		for (let i = 0; i < 60; i++) {
			await new Promise((resolve) => setTimeout(resolve, 500))
			if (!(await is_server_running(server.value.id).catch(() => true))) {
				break
			}
		}
		await run_server(server.value.id)
		running.value = true
	} catch (error) {
		handleError(error)
	} finally {
		restarting.value = false
	}
}

async function acceptEula() {
	if (!server.value) return
	if (isDemoId(server.value.id)) return
	try {
		await accept_server_eula(server.value.id)
		server.value = { ...server.value, eula_accepted: true }
	} catch (error) {
		handleError(error)
	}
}

async function openFolder() {
	if (!server.value) return
	if (isDemoId(server.value.id)) return
	const { open_server_folder } = await import('@/helpers/servers')
	await open_server_folder(server.value.id).catch(handleError)
}

const settingsModal = useTemplateRef('settingsModal')
function openSettings() {
	if (!server.value) return
	// The shared settings modal would try to persist to the backend; keep it
	// inert for demo servers.
	if (isDemoId(server.value.id)) return
	settingsModal.value?.show(server.value)
}

function onSettingsSaved(updated: ChocoServer) {
	server.value = updated
}

const loaderLabel = computed(() =>
	server.value ? (loaderNames[server.value.loader] ?? server.value.loader) : '',
)
const loaderNames: Record<string, string> = {
	vanilla: 'Vanilla',
	fabric: 'Fabric',
	forge: 'Forge',
	neoforge: 'NeoForge',
	quilt: 'Quilt',
	paper: 'Paper',
	purpur: 'Purpur',
}

function serverIconUrl(value: ChocoServer): string | null {
	return value.icon_file ? convertFileSrc(value.icon_file) : null
}
</script>

<template>
	<div class="mx-auto flex flex-col gap-4 px-8 pt-8 pb-12">
		<template v-if="loading">
			<div class="flex justify-center py-16 text-secondary">
				<span class="sr-only">Loading</span>
			</div>
		</template>

		<template v-else-if="!server">
			<div class="flex flex-col items-center gap-3 py-16">
				<ArchiveIcon class="size-12 text-secondary" />
				<p class="m-0 text-lg font-semibold text-contrast">Server not found</p>
				<Button color="brand" @click="router.push('/local-servers')"> Back to servers </Button>
			</div>
		</template>

		<template v-else>
			<div class="flex flex-wrap items-start justify-between gap-4">
				<div class="flex items-center gap-4">
					<img
						v-if="server.icon_file"
						:src="serverIconUrl(server)"
						:alt="server.name"
						class="size-16 rounded-2xl object-cover"
					/>
					<div v-else class="flex size-16 items-center justify-center rounded-2xl bg-surface-3">
						<ArchiveIcon class="size-8 text-secondary" />
					</div>
					<div class="flex flex-col gap-1">
						<h1 class="m-0 text-2xl font-bold text-contrast">
							{{ server.name }}
						</h1>
						<p class="m-0 flex items-center gap-2 text-sm text-secondary">
							{{ loaderLabel }} {{ server.game_version }}
							<template v-if="server.loader_version"> · {{ server.loader_version }}</template>
							<span
								class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-semibold"
								:class="running ? 'bg-highlight-green text-green' : 'bg-surface-4 text-secondary'"
							>
								<span
									class="size-1.5 rounded-full"
									:class="running ? 'bg-green' : 'bg-secondary'"
								/>
								{{ running ? 'Running' : 'Stopped' }}
							</span>
						</p>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-2">
					<Button v-if="!server.eula_accepted" size="lg" color="brand" @click="acceptEula">
						Accept EULA
					</Button>
					<Button v-if="running" size="lg" color="red" @click="stopServer">
						<StopCircleIcon aria-hidden="true" />
						Stop
					</Button>
					<Button
						v-else
						size="lg"
						color="brand"
						:disabled="!server.eula_accepted"
						:title="server.eula_accepted ? undefined : 'Accept the EULA first'"
						@click="startServer"
					>
						<PlayIcon aria-hidden="true" />
						Start
					</Button>
					<Button v-if="running" size="lg" :disabled="restarting" @click="restartServer">
						<span
							v-if="restarting"
							class="size-4 animate-spin rounded-full border-2 border-solid border-brand border-t-transparent"
						/>
						{{ restarting ? 'Restarting…' : 'Restart' }}
					</Button>
					<Button v-if="!isDemoServer" icon-only size="lg" @click="openFolder">
						<FolderOpenIcon aria-hidden="true" />
					</Button>
					<Button v-tooltip="'More settings'" icon-only size="lg" @click="openSettings">
						<WrenchIcon aria-hidden="true" />
					</Button>
				</div>
			</div>

			<div class="w-fit">
				<NavTabs
					mode="local"
					:links="tabLinks"
					:active-index="tabIndex"
					@tab-click="(index: number) => (activeTab = tabItems[index])"
				/>
			</div>

			<DashboardTab
				v-if="activeTab === 'dashboard'"
				:server="server"
				:running="running"
				@open-console="activeTab = 'console'"
			/>
			<ContentTab v-else-if="activeTab === 'content'" :server="server" />
			<PropertiesTab v-else-if="activeTab === 'properties'" :server="server" />
			<ConsoleTab v-else-if="activeTab === 'console'" :server="server" :running="running" />
			<PlayersTab v-else-if="activeTab === 'players'" :server="server" :running="running" />
		</template>

		<SettingsModal
			ref="settingsModal"
			@saved="onSettingsSaved"
			@deleted="router.push('/local-servers')"
		/>
	</div>
</template>
