<script setup lang="ts">
import { PlayIcon, PlusIcon } from '@modrinth/assets'
import {
	Avatar,
	ContextMenu,
	defineMessages,
	injectNotificationManager,
	NewModal,
	TagItem,
	useVIntl,
} from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import dayjs from 'dayjs'
import { computed, inject, ref } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'

import LibrarySection from '@/components/ui/library/index.vue'
import { libraryScrollTop } from '@/components/ui/library/view-state'
import WelcomeScreen from '@/components/ui/WelcomeScreen.vue'
import RecentWorldsList from '@/components/ui/world/RecentWorldsList.vue'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import {
	DEMO_MODPACKS,
	DEMO_MODPACK_MODS_MODAL_NOTE,
	type DemoModpack,
} from '@/helpers/demo-data.ts'
import { instanceListQueryOptions } from '@/pages/instance/query-options'
import { useRootBreadcrumb } from '@/providers/breadcrumbs'
import { injectOnboardingChecklist } from '@/providers/onboarding-checklist'

defineOptions({
	name: 'LibraryPage',
})

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { hasCreatedInstance, isReady } = injectOnboardingChecklist()
const showCreationModal = inject<() => void>('showCreationModal')
const pageOptions = ref<InstanceType<typeof ContextMenu>>()
const appSettings = useAppSettings()
onBeforeRouteLeave(() => {
	libraryScrollTop.value = document.querySelector('.app-viewport')?.scrollTop ?? 0
})

const messages = defineMessages({
	home: {
		id: 'app.navigation.home',
		defaultMessage: 'Home',
	},
	newInstance: {
		id: 'app.library.context-menu.create-instance',
		defaultMessage: 'New instance',
	},
	libraryActionsLabel: {
		id: 'app.library.actions.label',
		defaultMessage: 'Library actions',
	},
})

useRootBreadcrumb({
	slot: 'root',
	id: 'home',
	label: formatMessage(messages.home),
	to: '/',
	visual: { type: 'icon', component: PlayIcon },
})

const instancesQuery = useQuery(instanceListQueryOptions())
const instances = computed(() => instancesQuery.data.value ?? [])
if (hasCreatedInstance.value) {
	await instancesQuery.suspense().catch(handleError)
}

const recentInstances = computed(() =>
	instances.value
		.slice()
		.sort((a, b) => dayjs(b.last_played ?? b.created).diff(dayjs(a.last_played ?? a.created))),
)

function openPageContextMenu(event: MouseEvent) {
	if (
		!(event.target instanceof HTMLElement) ||
		!event.target.hasAttribute('data-library-page-background')
	) {
		return
	}

	event.preventDefault()
	event.stopPropagation()
	pageOptions.value?.open(event, [
		{
			id: 'new_instance',
			label: formatMessage(messages.newInstance),
			icon: PlusIcon,
			action: () => showCreationModal?.(),
		},
	])
}

// Demo view: fake modpacks shown alongside the library when the
// "demo_view" feature flag is enabled. They are not real instances,
// so they open an informational modal instead of an instance page.
const demoModsModal = ref<InstanceType<typeof NewModal>>()
const selectedDemoModpack = ref<DemoModpack | null>(null)
const demoModpacks = computed(() => (appSettings.getFeatureFlag('demo_view') ? DEMO_MODPACKS : []))
const selectedDemoMods = computed(() => selectedDemoModpack.value?.mods ?? [])

function openDemoModpackModal(pack: DemoModpack) {
	selectedDemoModpack.value = pack
	demoModsModal.value?.show()
}
</script>

<template>
	<WelcomeScreen v-if="isReady && !hasCreatedInstance" />
	<div
		v-else-if="isReady"
		data-library-page-background
		class="flex flex-col gap-3 p-6"
		@contextmenu="openPageContextMenu"
	>
		<RecentWorldsList
			v-if="recentInstances?.length > 0 && appSettings.getFeatureFlag('worlds_in_home')"
			:recent-instances="recentInstances"
		/>
		<div
			v-if="demoModpacks.length > 0"
			class="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-3"
		>
			<button
				v-for="pack in demoModpacks"
				:key="pack.id"
				type="button"
				class="relative flex w-full cursor-pointer select-none flex-col items-start justify-end gap-3 overflow-clip rounded-[20px] border border-solid border-surface-4 bg-surface-3 p-3 text-left transition-[background-color,border-color,filter] hover:brightness-110 -outline-offset-2 focus-visible:!outline-2"
				@click="openDemoModpackModal(pack)"
			>
				<div
					class="relative flex aspect-square min-w-full shrink-0 items-center overflow-clip rounded-2xl"
				>
					<Avatar
						class="pointer-events-none outline-none !rounded-2xl"
						size="100%"
						:tint-by="pack.id"
						alt=""
						no-shadow
						pad-transparent-corners
					/>
					<TagItem
						class="absolute right-2 top-2 z-[1] border-surface-5 bg-surface-2 font-semibold text-contrast"
					>
						Demo
					</TagItem>
				</div>
				<div class="flex w-full min-w-0 flex-col items-start justify-center gap-1 px-0.5">
					<p class="m-0 w-full truncate text-base font-semibold leading-5 text-contrast">
						{{ pack.name }}
					</p>
					<p class="m-0 w-full truncate text-sm font-medium capitalize leading-[18px] text-primary">
						{{ pack.loader }} {{ pack.game_version }}
					</p>
				</div>
			</button>
		</div>
		<LibrarySection :instances="instances" />
		<ContextMenu ref="pageOptions" :label="formatMessage(messages.libraryActionsLabel)" />
		<NewModal
			v-if="demoModpacks.length > 0"
			ref="demoModsModal"
			:header="selectedDemoModpack?.name"
			width="440px"
		>
			<div class="m-0 flex flex-col gap-1.5">
				<div v-for="mod in selectedDemoMods" :key="mod.name" class="flex items-baseline justify-between gap-4">
					<span class="text-contrast">{{ mod.name }}</span>
					<span class="shrink-0 text-secondary tabular-nums">{{ mod.version }}</span>
				</div>
			</div>
			<p class="m-0 mt-4 text-sm text-secondary">{{ DEMO_MODPACK_MODS_MODAL_NOTE }}</p>
		</NewModal>
	</div>
</template>
