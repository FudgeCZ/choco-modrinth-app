<script setup lang="ts">
import {
	CrownIcon,
	HeartIcon,
	SpinnerIcon,
} from '@modrinth/assets'
import {
	Button,
	injectNotificationManager,
} from '@modrinth/ui'
import { computed, onMounted, onUnmounted, ref } from 'vue'

import {
	get_player_details,
	get_players_overview,
	ping_server,
	serverConsoleLines,
	set_player_flag,
	type ChocoServer,
	type KnownPlayer,
	type PlayerDetails,
	type ServerPing,
} from '@/helpers/servers'

const props = defineProps<{
	server: ChocoServer
	running: boolean
}>()

const { handleError } = injectNotificationManager()

type PlayerRow = {
	name: string
	online: boolean
	known: KnownPlayer | null
}

const overview = ref<KnownPlayer[]>([])
const loadingOverview = ref(true)
const ping = ref<ServerPing | null>(null)
const busyPlayer = ref<string | null>(null)
const expandedPlayer = ref<string | null>(null)
const details = ref<Record<string, PlayerDetails>>({})
const loadingDetails = ref<string | null>(null)
const avatarErrors = ref<Record<string, boolean>>({})

let overviewInterval: number | undefined
let pingInterval: number | undefined

const lines = computed(() => serverConsoleLines.value[props.server.id] ?? [])

// Track online players from console join/leave messages
const onlinePlayers = computed(() => {
	const online = new Set<string>()
	for (const line of lines.value) {
		const join = line.match(/\]:\s*(\S+) joined the game/)
		if (join) {
			online.add(join[1])
			continue
		}
		const leave = line.match(/\]:\s*(\S+) left the game/)
		if (leave) {
			online.delete(leave[1])
		}
	}
	return [...online]
})

// Online players (from console) merged over the overview list, then every
// known player that is currently offline.
const rows = computed<PlayerRow[]>(() => {
	const rows: PlayerRow[] = []
	for (const name of onlinePlayers.value) {
		rows.push({
			name,
			online: true,
			known: overview.value.find((p) => p.name === name) ?? null,
		})
	}
	for (const player of overview.value) {
		if (onlinePlayers.value.includes(player.name)) continue
		rows.push({ name: player.name, online: false, known: player })
	}
	return rows
})

const sections = computed(() => [
	{ key: 'online' as const, label: 'Online', rows: rows.value.filter((r) => r.online) },
	{ key: 'known' as const, label: 'Known players', rows: rows.value.filter((r) => !r.online) },
])

function initials(name: string): string {
	return name.slice(0, 2).toUpperCase()
}

async function loadOverview() {
	try {
		overview.value = (await get_players_overview(props.server.id)).players
	} catch (error) {
		handleError(error)
	} finally {
		loadingOverview.value = false
	}
}

async function pollPing() {
	if (!props.running) {
		ping.value = null
		return
	}
	try {
		ping.value = await ping_server(props.server.port)
	} catch {
		ping.value = null
	}
}

async function applyFlag(player: string, flag: 'op' | 'whitelist' | 'ban' | 'kick', value: boolean) {
	busyPlayer.value = player
	try {
		await set_player_flag(props.server.id, player, flag, value)
		await loadOverview()
		// the server persists its player json files asynchronously; refresh
		// again shortly after so the new flags actually show up
		window.setTimeout(() => {
			void loadOverview()
		}, 800)
	} catch (error) {
		handleError(error)
	} finally {
		busyPlayer.value = null
	}
}

async function toggleDetails(name: string) {
	if (expandedPlayer.value === name) {
		expandedPlayer.value = null
		return
	}
	expandedPlayer.value = name
	loadingDetails.value = name
	try {
		details.value[name] = await get_player_details(props.server.id, name)
	} catch (error) {
		details.value[name] = {
			available: false,
			name,
			uuid: '',
			hearts: null,
			food: null,
			game_mode: null,
			bed: null,
			inventory: [],
		}
		handleError(error)
	} finally {
		loadingDetails.value = null
	}
}

onMounted(() => {
	void loadOverview()
	void pollPing()
	overviewInterval = window.setInterval(loadOverview, 15000)
	pingInterval = window.setInterval(pollPing, 15000)
})

onUnmounted(() => {
	if (overviewInterval) window.clearInterval(overviewInterval)
	if (pingInterval) window.clearInterval(pingInterval)
})
</script>

<template>
	<div class="flex flex-col gap-4">
		<div class="flex flex-wrap items-center justify-between gap-3">
			<h2 class="m-0 text-lg font-semibold text-contrast">Players</h2>
			<p v-if="running" class="m-0 text-sm text-secondary">
				{{ onlinePlayers.length }} online · {{ overview.length }} known
				<template v-if="ping">
					· {{ ping.players_online }}/{{ ping.players_max }} via ping
				</template>
			</p>
			<p v-else class="m-0 text-sm text-secondary">
				Start the server to see who is online.
			</p>
		</div>

		<p
			v-if="loadingOverview && overview.length === 0"
			class="m-0 text-sm text-secondary"
		>
			Loading players…
		</p>

		<p
			v-else-if="rows.length === 0"
			class="m-0 rounded-2xl bg-surface-2 p-6 text-center text-secondary"
		>
			No players yet. Players appear here after they join the server once.
		</p>

		<div v-else class="flex flex-col gap-5">
			<section
				v-for="section in sections"
				:key="section.key"
				class="flex flex-col gap-2"
			>
				<div class="flex items-center gap-2">
					<h3 class="m-0 text-sm font-semibold uppercase tracking-wider text-secondary">
						{{ section.label }}
					</h3>
					<span class="rounded-full bg-surface-4 px-2 py-0.5 text-xs font-bold text-contrast">
						{{ section.rows.length }}
					</span>
				</div>

				<p
					v-if="section.rows.length === 0"
					class="m-0 text-sm text-secondary"
				>
					{{
						section.key === 'online'
							? 'No one is online right now.'
							: 'No known players yet — they appear here after their first join.'
					}}
				</p>

				<div v-else class="flex flex-col gap-2">
					<div
						v-for="row in section.rows"
						:key="row.name"
						class="rounded-2xl border-0 border-solid border-divider bg-surface-2"
					>
						<div class="flex flex-wrap items-center gap-3 p-3">
							<div class="flex min-w-0 flex-1 flex-wrap items-center gap-2">
								<span class="relative size-8 shrink-0">
									<span
										v-if="avatarErrors[row.name]"
										class="flex size-8 items-center justify-center rounded-full bg-surface-4 text-xs font-bold text-contrast"
									>
										{{ initials(row.name) }}
									</span>
									<img
										v-else
										:src="`https://mc-heads.net/avatar/${encodeURIComponent(row.name)}/32`"
										:alt="row.name"
										class="size-8 rounded-full bg-surface-4 object-cover"
										@error="avatarErrors[row.name] = true"
									/>
								</span>
								<span class="font-bold text-contrast">{{ row.name }}</span>
								<span
									v-if="row.online"
									class="flex items-center gap-1 rounded-full bg-highlight-green px-2 py-0.5 text-xs font-semibold text-green"
								>
									<span class="size-1.5 rounded-full bg-green" />
									Online
								</span>
								<span
									v-if="row.known?.op"
									class="flex items-center gap-1 rounded-full bg-surface-4 px-2 py-0.5 text-xs text-contrast"
								>
									<CrownIcon class="size-3.5 text-brand" />
									OP
								</span>
								<span
									v-if="row.known?.whitelisted"
									class="rounded-full bg-surface-4 px-2 py-0.5 text-xs text-contrast"
								>
									Whitelisted
								</span>
								<span
									v-if="row.known?.banned"
									:title="row.known.ban_reason ?? 'Banned'"
									class="rounded-full bg-highlight-red px-2 py-0.5 text-xs font-semibold text-red"
								>
									Banned
								</span>
							</div>

							<div class="flex flex-wrap items-center gap-2">
								<SpinnerIcon
									v-if="busyPlayer === row.name"
									class="size-4 animate-spin text-secondary"
								/>
								<Button
									size="sm"
									:disabled="busyPlayer === row.name"
									@click="applyFlag(row.name, 'op', !row.known?.op)"
								>
									{{ row.known?.op ? 'Deop' : 'Op' }}
								</Button>
								<Button
									size="sm"
									:disabled="busyPlayer === row.name"
									@click="applyFlag(row.name, 'whitelist', !row.known?.whitelisted)"
								>
									{{ row.known?.whitelisted ? 'Remove whitelist' : 'Whitelist' }}
								</Button>
								<Button
									v-if="row.online"
									size="sm"
									:disabled="busyPlayer === row.name"
									@click="applyFlag(row.name, 'kick', true)"
								>
									Kick
								</Button>
								<Button
									v-if="!row.known?.banned"
									size="sm"
									color="red"
									:disabled="busyPlayer === row.name"
									@click="applyFlag(row.name, 'ban', true)"
								>
									Ban
								</Button>
								<Button
									v-else
									size="sm"
									:disabled="busyPlayer === row.name"
									@click="applyFlag(row.name, 'ban', false)"
								>
									Pardon
								</Button>
								<Button
									size="sm"
									@click="toggleDetails(row.name)"
								>
									{{ expandedPlayer === row.name ? 'Hide data' : 'Player data' }}
								</Button>
							</div>
						</div>

						<div
							v-if="expandedPlayer === row.name"
							class="border-t-0 border-solid border-divider px-3 pb-3"
						>
							<div
								v-if="loadingDetails === row.name"
								class="flex items-center gap-2 pt-3 text-sm text-secondary"
							>
								<SpinnerIcon class="size-4 animate-spin" />
								Loading player data…
							</div>
							<template v-else-if="details[row.name]">
								<p
									v-if="!details[row.name].available"
									class="m-0 pt-3 text-sm text-secondary"
								>
									No saved player data yet — the player must join the server at least once.
								</p>
								<div v-else class="grid grid-cols-1 gap-3 pt-3 md:grid-cols-3">
									<div class="rounded-xl bg-surface-3 p-3">
										<p class="m-0 flex items-center gap-1 text-xs font-semibold text-secondary">
											<HeartIcon class="size-3.5 text-red" />
											Health
										</p>
										<p class="m-0 text-lg font-bold text-contrast">
											<template v-if="details[row.name].hearts != null">
												{{ (details[row.name].hearts! / 2).toFixed(1) }} ❤
												<span class="text-xs font-normal text-secondary">
													({{ details[row.name].hearts }}/20)
												</span>
											</template>
											<template v-else>—</template>
										</p>
									</div>
									<div class="rounded-xl bg-surface-3 p-3">
										<p class="m-0 text-xs font-semibold text-secondary">Hunger</p>
										<p class="m-0 text-lg font-bold text-contrast">
											<template v-if="details[row.name].food != null">
												{{ details[row.name].food }}/20
											</template>
											<template v-else>—</template>
										</p>
									</div>
									<div class="rounded-xl bg-surface-3 p-3">
										<p class="m-0 text-xs font-semibold text-secondary">Game mode</p>
										<p class="m-0 text-lg font-bold text-contrast">
											{{ details[row.name].game_mode ?? '—' }}
										</p>
									</div>
									<div class="rounded-xl bg-surface-3 p-3 md:col-span-3">
										<p class="m-0 text-xs font-semibold text-secondary">
											Bed / respawn point
										</p>
										<p class="m-0 font-semibold text-contrast">
											<template v-if="details[row.name].bed">
												X {{ details[row.name].bed![0] }} · Y
												{{ details[row.name].bed![1] }} · Z
												{{ details[row.name].bed![2] }}
											</template>
											<template v-else>Not set</template>
										</p>
									</div>
									<div class="rounded-xl bg-surface-3 p-3 md:col-span-3">
										<p class="m-0 text-xs font-semibold text-secondary">
											Inventory ({{ details[row.name].inventory.length }} stacks)
										</p>
										<div
											v-if="details[row.name].inventory.length > 0"
											class="mt-1 flex flex-wrap gap-1.5"
										>
											<span
												v-for="item in details[row.name].inventory"
												:key="`${item.slot}-${item.name}`"
												class="rounded-full bg-surface-4 px-2.5 py-0.5 text-xs text-contrast"
											>
												{{ item.name.replace('minecraft:', '') }}
												<span class="text-secondary">×{{ item.count }}</span>
											</span>
										</div>
										<p v-else class="m-0 mt-1 text-sm text-secondary">Empty</p>
									</div>
								</div>
							</template>
						</div>
					</div>
				</div>
			</section>
		</div>
	</div>
</template>
