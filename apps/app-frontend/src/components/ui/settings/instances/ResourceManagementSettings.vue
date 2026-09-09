<script setup>
import { BoxIcon, FolderOpenIcon, FolderSearchIcon, TrashIcon } from '@modrinth/assets'
import {
	Button,
	defineMessages,
	IconButton,
	injectNotificationManager,
	Input,
	NewModal,
	Slider,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { ref, watch } from 'vue'

import ConfirmModalWrapper from '@/components/ui/modal/ConfirmModalWrapper.vue'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { purge_cache_types } from '@/helpers/cache.js'
import { default_profiles_dir, list as list_instances, move_profiles } from '@/helpers/instance'
import {
	default_servers_dir as get_default_servers_dir,
	list_servers,
	move_servers,
} from '@/helpers/servers'
import { get, set } from '@/helpers/settings.ts'
import { showAppDbBackupsFolder } from '@/helpers/utils.js'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const appSettings = useAppSettings()
const settings = ref(await get())
const purgeCacheConfirmModal = ref(null)
const alwaysShowCopyDetailsFlag = 'always_show_copy_details'

const moveModal = ref(null)
const moving = ref(false)
const defaultProfilesDir = ref('')
const defaultServersDir = ref('')
const pendingDir = ref('')
const pendingReset = ref(false)
const moveKind = ref('profiles')
const moveCandidates = ref([])

default_profiles_dir()
	.then((dir) => (defaultProfilesDir.value = dir))
	.catch(() => {})
get_default_servers_dir()
	.then((dir) => (defaultServersDir.value = dir))
	.catch(() => {})

const messages = defineMessages({
	appDirectoryTitle: {
		id: 'app.settings.resource-management.app-directory.title',
		defaultMessage: 'App directory',
	},
	appDirectoryDescription: {
		id: 'app.settings.resource-management.app-directory.description',
		defaultMessage:
			'Where ChocoModrinth stores instances and other files. Changes take effect after restarting the app.',
	},
	profilesFolderTitle: {
		id: 'app.settings.resource-management.profiles-folder.title',
		defaultMessage: 'Profiles folder',
	},
	profilesFolderDescription: {
		id: 'app.settings.resource-management.profiles-folder.description',
		defaultMessage:
			'Where new profiles are created. You can also move existing profiles here; moved profiles keep their settings and content.',
	},
	defaultProfilesFolder: {
		id: 'app.settings.resource-management.profiles-folder.default',
		defaultMessage: 'Default (in the app data folder)',
	},
	selectProfilesFolder: {
		id: 'app.settings.resource-management.profiles-folder.select',
		defaultMessage: 'Select a profiles folder',
	},
	browseProfilesFolder: {
		id: 'app.settings.resource-management.profiles-folder.browse',
		defaultMessage: 'Browse for a profiles folder',
	},
	resetProfilesFolder: {
		id: 'app.settings.resource-management.profiles-folder.reset',
		defaultMessage: 'Reset to default',
	},
	moveProfilesButton: {
		id: 'app.settings.resource-management.profiles-folder.move-button',
		defaultMessage: 'Move profiles…',
	},
	serversFolderTitle: {
		id: 'app.settings.resource-management.servers-folder.title',
		defaultMessage: 'Servers folder',
	},
	serversFolderDescription: {
		id: 'app.settings.resource-management.servers-folder.description',
		defaultMessage:
			'Where new local servers are created. You can also move existing servers here; moved servers keep their configuration and worlds.',
	},
	defaultServersFolder: {
		id: 'app.settings.resource-management.servers-folder.default',
		defaultMessage: 'Default (in the app data folder)',
	},
	selectServersFolder: {
		id: 'app.settings.resource-management.servers-folder.select',
		defaultMessage: 'Select a servers folder',
	},
	browseServersFolder: {
		id: 'app.settings.resource-management.servers-folder.browse',
		defaultMessage: 'Browse for a servers folder',
	},
	resetServersFolder: {
		id: 'app.settings.resource-management.servers-folder.reset',
		defaultMessage: 'Reset to default',
	},
	moveServersButton: {
		id: 'app.settings.resource-management.servers-folder.move-button',
		defaultMessage: 'Move servers…',
	},
	moveModalHeader: {
		id: 'app.settings.resource-management.profiles-folder.move-modal-header',
		defaultMessage: 'Change profiles folder',
	},
	moveModalHeaderServers: {
		id: 'app.settings.resource-management.servers-folder.move-modal-header',
		defaultMessage: 'Change servers folder',
	},
	moveServersDescription: {
		id: 'app.settings.resource-management.servers-folder.move-description',
		defaultMessage:
			'Select which servers to move to the new folder. Running servers are skipped; unselected servers stay where they are.',
	},
	moveProfilesDescription: {
		id: 'app.settings.resource-management.profiles-folder.move-description',
		defaultMessage:
			'Select which profiles to move to the new folder. Unselected profiles stay where they are and keep working.',
	},
	noProfilesToMove: {
		id: 'app.settings.resource-management.profiles-folder.no-profiles',
		defaultMessage: 'No profiles to move.',
	},
	moveSelected: {
		id: 'app.settings.resource-management.profiles-folder.move-selected',
		defaultMessage: 'Move selected',
	},
	setLocationOnly: {
		id: 'app.settings.resource-management.profiles-folder.set-only',
		defaultMessage: 'Set without moving',
	},
	cancel: {
		id: 'app.settings.resource-management.profiles-folder.cancel',
		defaultMessage: 'Cancel',
	},
	selectAppDirectory: {
		id: 'app.settings.resource-management.app-directory.select',
		defaultMessage: 'Select a new app directory',
	},
	browseAppDirectory: {
		id: 'app.settings.resource-management.app-directory.browse',
		defaultMessage: 'Browse for an app directory',
	},
	appCacheTitle: {
		id: 'app.settings.resource-management.app-cache.title',
		defaultMessage: 'App cache',
	},
	purgeCache: {
		id: 'app.settings.resource-management.app-cache.purge',
		defaultMessage: 'Purge cache',
	},
	purgeCacheConfirmTitle: {
		id: 'app.settings.resource-management.app-cache.confirm.title',
		defaultMessage: 'Purge the app cache?',
	},
	purgeCacheConfirmDescription: {
		id: 'app.settings.resource-management.app-cache.confirm.description',
		defaultMessage: 'The app may load more slowly until the cache is rebuilt.',
	},
	appCacheDescription: {
		id: 'app.settings.resource-management.app-cache.description',
		defaultMessage:
			'Clear cached data and download it again from Modrinth. The app may load more slowly until the cache is rebuilt.',
	},
	maximumConcurrentDownloadsTitle: {
		id: 'app.settings.resource-management.maximum-concurrent-downloads.title',
		defaultMessage: 'Maximum concurrent downloads',
	},
	maximumConcurrentDownloadsDescription: {
		id: 'app.settings.resource-management.maximum-concurrent-downloads.description',
		defaultMessage:
			'Number of files the app can download at once. Lower this if downloads are unreliable on your connection. Requires an app restart.',
	},
	maximumConcurrentWritesTitle: {
		id: 'app.settings.resource-management.maximum-concurrent-writes.title',
		defaultMessage: 'Maximum concurrent writes',
	},
	maximumConcurrentWritesDescription: {
		id: 'app.settings.resource-management.maximum-concurrent-writes.description',
		defaultMessage:
			'Number of files the app can write to disk at once. Lower this if you frequently encounter I/O errors. Requires an app restart.',
	},
	alwaysShowCopyDetailsTitle: {
		id: 'app.settings.resource-management.always-show-copy-details.title',
		defaultMessage: 'Always show copy details',
	},
	alwaysShowCopyDetailsDescription: {
		id: 'app.settings.resource-management.always-show-copy-details.description',
		defaultMessage:
			'Show the Copy details action while an install is queued or running. It is always available for failed or interrupted installs.',
	},
	appDatabaseBackupsTitle: {
		id: 'app.settings.resource-management.app-database-backups.title',
		defaultMessage: 'App database backups',
	},
	openBackupsFolder: {
		id: 'app.settings.resource-management.app-database-backups.open-folder',
		defaultMessage: 'Open backups folder',
	},
	appDatabaseBackupsDescription: {
		id: 'app.settings.resource-management.app-database-backups.description',
		defaultMessage:
			'Backups of important app data are stored here in case you need to recover them later.',
	},
})

watch(
	settings,
	async () => {
		const setSettings = JSON.parse(JSON.stringify(settings.value))

		if (!setSettings.custom_dir) {
			setSettings.custom_dir = null
		}

		await set(setSettings)
	},
	{ deep: true },
)

async function purgeCache() {
	await purge_cache_types([
		'project',
		'project_v3',
		'version',
		'user',
		'team',
		'organization',
		'file',
		'loader_manifest',
		'minecraft_manifest',
		'categories',
		'report_types',
		'loaders',
		'game_versions',
		'donation_platforms',
		'file_hash',
		'file_update',
		'search_results',
		'search_results_v3',
	]).catch(handleError)
}

function handlePurgeCacheClick() {
	if (appSettings.getFeatureFlag('skip_non_essential_warnings')) {
		void purgeCache()
		return
	}

	purgeCacheConfirmModal.value?.show()
}

async function openDbBackupsFolder() {
	await showAppDbBackupsFolder().catch(handleError)
}

async function findLauncherDir() {
	const newDir = await open({
		multiple: false,
		directory: true,
		title: formatMessage(messages.selectAppDirectory),
	})

	if (newDir) {
		settings.value.custom_dir = newDir
	}
}

async function browseProfilesDir() {
	const newDir = await open({
		multiple: false,
		directory: true,
		title: formatMessage(messages.selectProfilesFolder),
	})

	if (newDir) {
		await showMoveModal(newDir, false)
	}
}

async function resetProfilesDir() {
	await showMoveModal(defaultProfilesDir.value, true)
}

async function moveMoreProfiles() {
	const target = settings.value.custom_profiles_dir || defaultProfilesDir.value
	if (!target) {
		handleError('No profiles folder is set')
		return
	}
	await showMoveModal(target, false, 'profiles')
}

async function browseServersDir() {
	const newDir = await open({
		multiple: false,
		directory: true,
		title: formatMessage(messages.selectServersFolder),
	})

	if (newDir) {
		await showMoveModal(newDir, false, 'servers')
	}
}

async function resetServersDir() {
	await showMoveModal(defaultServersDir.value, true, 'servers')
}

async function moveMoreServers() {
	const target = settings.value.custom_servers_dir || defaultServersDir.value
	if (!target) {
		handleError('No servers folder is set')
		return
	}
	await showMoveModal(target, false, 'servers')
}

async function showMoveModal(dir, isReset, kind = 'profiles') {
	pendingDir.value = dir
	pendingReset.value = isReset
	moveKind.value = kind
	try {
		const items =
			moveKind.value === 'servers' ? await list_servers() : await list_instances()
		moveCandidates.value = items.map((item) => ({
			id: item.id,
			name: item.name,
			selected: true,
		}))
	} catch (error) {
		moveCandidates.value = []
		handleError(error)
	}
	moveModal.value?.show()
}

async function confirmMove(moveSelected) {
	moving.value = true
	try {
		if (moveSelected) {
			const ids = moveCandidates.value
				.filter((candidate) => candidate.selected)
				.map((candidate) => candidate.id)
			if (ids.length > 0) {
				const report =
					moveKind.value === 'servers'
						? await move_servers(ids, pendingDir.value)
						: await move_profiles(ids, pendingDir.value)
				for (const failure of report.failed) {
					handleError(failure.error)
				}
			}
		}
		// Moving into the default folder is equivalent to having no custom folder
		const keepCustomDir = !pendingReset.value && pendingDir.value !== defaultProfilesDir.value
		const keepCustomServersDir =
			!pendingReset.value && pendingDir.value !== defaultServersDir.value
		if (moveKind.value === 'servers') {
			settings.value.custom_servers_dir = keepCustomServersDir ? pendingDir.value : null
		} else {
			settings.value.custom_profiles_dir = keepCustomDir ? pendingDir.value : null
		}
		moveModal.value?.hide()
	} catch (error) {
		handleError(error)
	} finally {
		moving.value = false
	}
}
</script>

<template>
	<div class="flex flex-col gap-6">
		<div class="flex flex-col gap-2.5">
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.appDirectoryTitle) }}
			</h2>
			<Input
				id="appDir"
				v-model="settings.custom_dir"
				:icon="BoxIcon"
				type="text"
				wrapper-class="w-full"
			>
				<template #right>
					<IconButton
						v-tooltip="formatMessage(messages.browseAppDirectory)"
						:label="formatMessage(messages.browseAppDirectory)"
						class="ml-1.5"
						@click="findLauncherDir"
					>
						<FolderSearchIcon aria-hidden="true" />
					</IconButton>
				</template>
			</Input>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.appDirectoryDescription) }}
			</p>
		</div>

		<div class="flex flex-col gap-2.5">
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.profilesFolderTitle) }}
			</h2>
			<div class="flex items-center gap-2">
				<Input
					id="profilesDir"
					:model-value="
						settings.custom_profiles_dir ??
						(defaultProfilesDir || formatMessage(messages.defaultProfilesFolder))
					"
					:icon="FolderOpenIcon"
					type="text"
					wrapper-class="w-full"
					disabled
				>
					<template #right>
						<IconButton
							v-tooltip="formatMessage(messages.browseProfilesFolder)"
							:label="formatMessage(messages.browseProfilesFolder)"
							class="ml-1.5"
							@click="browseProfilesDir"
						>
							<FolderSearchIcon aria-hidden="true" />
						</IconButton>
					</template>
				</Input>
				<Button
					v-if="settings.custom_profiles_dir"
					class="shrink-0"
					@click="resetProfilesDir"
				>
					{{ formatMessage(messages.resetProfilesFolder) }}
				</Button>
				<Button class="shrink-0" @click="moveMoreProfiles">
					{{ formatMessage(messages.moveProfilesButton) }}
				</Button>
			</div>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.profilesFolderDescription) }}
			</p>

			<NewModal
				ref="moveModal"
				:header="
					formatMessage(
						moveKind === 'servers'
							? messages.moveModalHeaderServers
							: messages.moveModalHeader,
					)
				"
			>
				<div class="flex max-h-[26rem] flex-col gap-3">
					<p class="m-0 text-secondary">
						{{
							formatMessage(
								moveCandidates.length === 0
									? messages.noProfilesToMove
									: moveKind === 'servers'
										? messages.moveServersDescription
										: messages.moveProfilesDescription,
							)
						}}
					</p>
					<div class="flex max-h-64 flex-col gap-1 overflow-y-auto">
						<label
							v-for="candidate in moveCandidates"
							:key="candidate.id"
							class="flex cursor-pointer items-center gap-2 rounded-xl p-1.5 hover:bg-surface-3"
						>
							<Toggle v-model="candidate.selected" />
							<span class="text-contrast">{{ candidate.name }}</span>
						</label>
					</div>
					<div class="flex items-center justify-end gap-2">
						<Button @click="moveModal?.hide()">
							{{ formatMessage(messages.cancel) }}
						</Button>
						<Button :disabled="moving" @click="confirmMove(false)">
							{{ formatMessage(messages.setLocationOnly) }}
						</Button>
						<Button color="brand" :loading="moving" @click="confirmMove(true)">
							{{ formatMessage(messages.moveSelected) }}
						</Button>
					</div>
				</div>
			</NewModal>
		</div>

		<div class="flex flex-col gap-2.5">
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.serversFolderTitle) }}
			</h2>
			<div class="flex items-center gap-2">
				<Input
					id="serversDir"
					:model-value="
						settings.custom_servers_dir ??
						(defaultServersDir || formatMessage(messages.defaultServersFolder))
					"
					:icon="FolderOpenIcon"
					type="text"
					wrapper-class="w-full"
					disabled
				>
					<template #right>
						<IconButton
							v-tooltip="formatMessage(messages.browseServersFolder)"
							:label="formatMessage(messages.browseServersFolder)"
							class="ml-1.5"
							@click="browseServersDir"
						>
							<FolderSearchIcon aria-hidden="true" />
						</IconButton>
					</template>
				</Input>
				<Button
					v-if="settings.custom_servers_dir"
					class="shrink-0"
					@click="resetServersDir"
				>
					{{ formatMessage(messages.resetServersFolder) }}
				</Button>
				<Button class="shrink-0" @click="moveMoreServers">
					{{ formatMessage(messages.moveServersButton) }}
				</Button>
			</div>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.serversFolderDescription) }}
			</p>
		</div>

		<div class="flex items-center justify-between gap-4">
			<div>
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.alwaysShowCopyDetailsTitle) }}
				</h2>
				<p class="m-0 mt-1">
					{{ formatMessage(messages.alwaysShowCopyDetailsDescription) }}
				</p>
			</div>
			<Toggle
				id="always-show-copy-details"
				:model-value="appSettings.getFeatureFlag(alwaysShowCopyDetailsFlag)"
				@update:model-value="
					() => {
						const newValue = !appSettings.getFeatureFlag(alwaysShowCopyDetailsFlag)
						appSettings.featureFlags[alwaysShowCopyDetailsFlag] = newValue
						settings.feature_flags[alwaysShowCopyDetailsFlag] = newValue
					}
				"
			/>
		</div>

		<div class="flex flex-col gap-2.5">
			<ConfirmModalWrapper
				ref="purgeCacheConfirmModal"
				:title="formatMessage(messages.purgeCacheConfirmTitle)"
				:description="formatMessage(messages.purgeCacheConfirmDescription)"
				:has-to-type="false"
				:proceed-label="formatMessage(messages.purgeCache)"
				:show-ad-on-close="false"
				@proceed="purgeCache"
			/>
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.appCacheTitle) }}
			</h2>
			<Button id="purge-cache" class="w-fit" @click="handlePurgeCacheClick">
				<TrashIcon aria-hidden="true" />
				{{ formatMessage(messages.purgeCache) }}
			</Button>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.appCacheDescription) }}
			</p>
		</div>

		<div class="flex flex-col gap-2.5">
			<h2 class="m-0 text-lg font-semibold text-contrast mt-4">
				{{ formatMessage(messages.maximumConcurrentDownloadsTitle) }}
			</h2>
			<Slider
				id="max-downloads"
				v-model="settings.max_concurrent_downloads"
				:min="1"
				:max="10"
				:step="1"
			/>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.maximumConcurrentDownloadsDescription) }}
			</p>
		</div>

		<div class="flex flex-col gap-2.5">
			<h2 class="mt-0 m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.maximumConcurrentWritesTitle) }}
			</h2>
			<Slider
				id="max-writes"
				v-model="settings.max_concurrent_writes"
				:min="1"
				:max="50"
				:step="1"
			/>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.maximumConcurrentWritesDescription) }}
			</p>
		</div>

		<div class="flex flex-col gap-2.5">
			<h2 class="mt-0 m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.appDatabaseBackupsTitle) }}
			</h2>
			<Button id="open-db-backups-folder" class="w-fit" @click="openDbBackupsFolder">
				<FolderOpenIcon aria-hidden="true" />
				{{ formatMessage(messages.openBackupsFolder) }}
			</Button>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.appDatabaseBackupsDescription) }}
			</p>
		</div>
	</div>
</template>
