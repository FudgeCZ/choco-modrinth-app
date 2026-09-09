<script setup lang="ts">
import {
	ArchiveIcon,
	FolderOpenIcon,
	PlayIcon,
	StopCircleIcon,
	TrashIcon,
} from '@modrinth/assets'
import {
	Button,
	Chips,
	ConfirmModal,
	injectNotificationManager,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, onMounted, ref, useTemplateRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import {
	accept_server_eula,
	is_server_running,
	list_servers,
	run_server,
	stop_server,
	type ChocoServer,
	type ServerLoader,
} from '@/helpers/servers'
import { useBreadcrumb, useRootBreadcrumb } from '@/providers/breadcrumbs'

import ContentTab from './ContentTab.vue'
import ConsoleTab from './ConsoleTab.vue'
import DashboardTab from './DashboardTab.vue'
import PlayersTab from './PlayersTab.vue'
import PropertiesTab from './PropertiesTab.vue'

const route = useRoute()
const router = useRouter()
const { handleError } = injectNotificationManager()

const serverId = computed(() => route.params.id as string)
const server = ref<ChocoServer | null>(null)
const loading = ref(true)
const running = ref(false)
const restarting = ref(false)

const activeTab = ref('dashboard')
const tabItems = ['dashboard', 'content', 'properties', 'console', 'players'] as const
const tabLabels: Record<string, string> = {
	dashboard: 'Dashboard',
	content: 'Content',
	properties: 'Properties',
	console: 'Console',
	players: 'Players',
}

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

async function startServer() {
	if (!server.value) return
	try {
		await run_server(server.value.id)
		running.value = true
	} catch (error) {
		handleError(error)
	}
}

async function stopServer() {
	if (!server.value) return
	try {
		await stop_server(server.value.id)
		running.value = false
	} catch (error) {
		handleError(error)
	}
}

async function restartServer() {
	if (!server.value || restarting.value) return
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
	try {
		await accept_server_eula(server.value.id)
		server.value = { ...server.value, eula_accepted: true }
	} catch (error) {
		handleError(error)
	}
}

async function openFolder() {
	if (!server.value) return
	const { open_server_folder } = await import('@/helpers/servers')
	await open_server_folder(server.value.id).catch(handleError)
}

const deleteModal = useTemplateRef('deleteModal')
async function confirmDelete() {
	if (!server.value) return
	deleteModal.value?.show()
}

async function doDelete() {
	if (!server.value) return
	const { delete_server } = await import('@/helpers/servers')
	try {
		await delete_server(server.value.id)
		router.push('/local-servers')
	} catch (error) {
		handleError(error)
	}
}

const loaderLabel = computed(() =>
	server.value ? loaderNames[server.value.loader] ?? server.value.loader : '',
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
				<Button color="brand" @click="router.push('/local-servers')">
					Back to servers
				</Button>
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
					<div
						v-else
						class="flex size-16 items-center justify-center rounded-2xl bg-surface-3"
					>
						<ArchiveIcon class="size-8 text-secondary" />
					</div>
					<div class="flex flex-col gap-1">
						<h1 class="m-0 text-2xl font-bold text-contrast">
							{{ server.name }}
						</h1>
						<p class="m-0 flex items-center gap-2 text-sm text-secondary">
							{{ loaderLabel }} {{ server.game_version }}
							<template v-if="server.loader_version">
								· {{ server.loader_version }}</template
							>
							<span
								class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-semibold"
								:class="
									running
										? 'bg-highlight-green text-green'
										: 'bg-surface-4 text-secondary'
								"
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
					<Button
						v-if="!server.eula_accepted"
						color="brand"
						@click="acceptEula"
					>
						Accept EULA
					</Button>
					<Button
						v-if="running"
						color="red"
						@click="stopServer"
					>
						<StopCircleIcon aria-hidden="true" />
						Stop
					</Button>
					<Button
						v-else
						color="brand"
						:disabled="!server.eula_accepted"
						:title="server.eula_accepted ? undefined : 'Accept the EULA first'"
						@click="startServer"
					>
						<PlayIcon aria-hidden="true" />
						Start
					</Button>
					<Button
						v-if="running"
						:disabled="restarting"
						@click="restartServer"
					>
						<span
							v-if="restarting"
							class="size-4 animate-spin rounded-full border-2 border-solid border-brand border-t-transparent"
						/>
						{{ restarting ? 'Restarting…' : 'Restart' }}
					</Button>
					<Button icon-only @click="openFolder">
						<FolderOpenIcon aria-hidden="true" />
					</Button>
					<Button icon-only color="red" @click="confirmDelete">
						<TrashIcon aria-hidden="true" />
					</Button>
				</div>
			</div>

			<div class="w-fit rounded-xl bg-surface-3 p-1">
				<Chips
					v-model="activeTab"
					:items="[...tabItems]"
					:format-label="(item: string) => tabLabels[item] ?? item"
				/>
			</div>

			<DashboardTab
				v-if="activeTab === 'dashboard'"
				:server="server"
				:running="running"
				@open-console="activeTab = 'console'"
			/>
			<ContentTab
				v-else-if="activeTab === 'content'"
				:server="server"
			/>
			<PropertiesTab
				v-else-if="activeTab === 'properties'"
				:server="server"
			/>
			<ConsoleTab
				v-else-if="activeTab === 'console'"
				:server="server"
				:running="running"
			/>
			<PlayersTab
				v-else-if="activeTab === 'players'"
				:server="server"
				:running="running"
			/>
		</template>

		<ConfirmModal
			ref="deleteModal"
			title="Delete this server?"
			description="The server folder and all its files will be permanently deleted. This cannot be undone."
			:has-to-type="false"
			proceed-label="Delete"
			@proceed="doDelete"
		/>
	</div>
</template>
