<script setup lang="ts">
import { SpinnerIcon, TrashIcon } from '@modrinth/assets'
import {
	Button,
	ConfirmModal,
	injectNotificationManager,
	Toggle,
} from '@modrinth/ui'
import { onMounted, useTemplateRef, ref } from 'vue'

import {
	delete_server_content,
	list_server_content,
	set_server_content_enabled,
	type ChocoServer,
	type ServerContentItem,
} from '@/helpers/servers'

const props = defineProps<{
	server: ChocoServer
}>()

const { handleError } = injectNotificationManager()

const loading = ref(true)
const items = ref<ServerContentItem[]>([])
const deleteTarget = ref<ServerContentItem | null>(null)
const deleteModal = useTemplateRef('deleteModal')

async function load() {
	loading.value = true
	try {
		items.value = await list_server_content(props.server.id)
	} catch (error) {
		handleError(error)
	} finally {
		loading.value = false
	}
}

onMounted(load)

function displayName(item: ServerContentItem): string {
	return item.title ?? item.file_name.replace(/\.(jar|disabled)$/i, '')
}

async function toggleEnabled(item: ServerContentItem) {
	try {
		await set_server_content_enabled(
			props.server.id,
			item.file_name,
			!item.enabled,
		)
		await load()
	} catch (error) {
		handleError(error)
	}
}

function askDelete(item: ServerContentItem) {
	deleteTarget.value = item
	deleteModal.value?.show()
}

async function doDelete() {
	if (!deleteTarget.value) return
	try {
		await delete_server_content(props.server.id, deleteTarget.value.file_name)
		await load()
	} catch (error) {
		handleError(error)
	} finally {
		deleteTarget.value = null
	}
}

function formatSize(bytes: number): string {
	if (bytes > 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
	return `${Math.max(1, Math.round(bytes / 1024))} KB`
}
</script>

<template>
	<div class="flex flex-col gap-3">
		<div class="flex items-center justify-between">
			<h2 class="m-0 text-lg font-semibold text-contrast">
				Installed content
			</h2>
			<p class="m-0 text-xs text-secondary">
				Enable/disable takes effect after a server restart. Restart to apply.
			</p>
		</div>

		<div
			v-if="loading"
			class="flex justify-center py-12 text-secondary"
		>
			<SpinnerIcon class="size-8 animate-spin" />
		</div>

		<p
			v-else-if="items.length === 0"
			class="m-0 rounded-2xl bg-surface-2 p-6 text-center text-secondary"
		>
			No mods or plugins installed yet. Drop .jar files into the server's
			content folder (use the folder button in the header).
		</p>

		<div v-else class="flex flex-col gap-2">
			<div
				v-for="item in items"
				:key="item.file_name"
				class="flex flex-wrap items-center gap-3 rounded-2xl border-0 border-solid border-divider p-3 bg-surface-2"
			>
				<img
					v-if="item.icon_url"
					:src="item.icon_url"
					:alt="displayName(item)"
					class="size-10 rounded-xl object-cover"
				/>
				<div
					v-else
					class="flex size-10 items-center justify-center rounded-xl bg-surface-4 text-xs font-bold text-secondary"
				>
					{{ displayName(item).slice(0, 2).toUpperCase() }}
				</div>
				<div class="min-w-0 flex-1">
					<p class="m-0 truncate font-semibold text-contrast">
						{{ displayName(item) }}
						<span
							v-if="item.version"
							class="ml-1 text-xs font-normal text-secondary"
						>
							{{ item.version }}
						</span>
					</p>
					<p class="m-0 truncate text-xs text-secondary">
						{{ item.file_name }} · {{ formatSize(item.size) }}
					</p>
				</div>
				<div class="flex items-center gap-2">
					<span class="text-xs text-secondary">
						{{ item.enabled ? 'Enabled' : 'Disabled' }}
					</span>
					<Toggle
						:model-value="item.enabled"
						@update:model-value="() => toggleEnabled(item)"
					/>
					<Button icon-only color="red" @click="askDelete(item)">
						<TrashIcon aria-hidden="true" />
					</Button>
				</div>
			</div>
		</div>

		<ConfirmModal
			ref="deleteModal"
			:title="`Delete ${deleteTarget?.file_name ?? 'file'}?`"
			description="The file will be permanently deleted from the server."
			:has-to-type="false"
			proceed-label="Delete"
			@proceed="doDelete"
		/>
	</div>
</template>
