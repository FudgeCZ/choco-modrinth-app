import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { ref } from 'vue'

export type ServerLoader =
	| 'vanilla'
	| 'fabric'
	| 'forge'
	| 'neoforge'
	| 'quilt'
	| 'paper'
	| 'purpur'

export type ChocoServer = {
	id: string
	name: string
	path: string
	game_version: string
	loader: ServerLoader
	loader_version?: string | null
	jar_file?: string | null
	ram_mb: number
	port: number
	eula_accepted: boolean
	java_path?: string | null
	linked_instance_id?: string | null
	icon_file?: string | null
	created: string
}

export type CreateServerOptions = {
	name: string
	game_version: string
	loader: ServerLoader
	loader_version?: string | null
	ram_mb: number
	port: number
	motd?: string | null
	difficulty?: string | null
	gamemode?: string | null
	max_players?: number | null
	online_mode?: boolean | null
	accept_eula: boolean
	level_name?: string | null
	linked_instance_id?: string | null
	icon_path?: string | null
}

export type ServerLoaderVersion = {
	id: string
	recommended: boolean
}

export async function list_servers(): Promise<ChocoServer[]> {
	return await invoke('plugin:servers|servers_list')
}

export async function create_server(options: CreateServerOptions): Promise<ChocoServer> {
	return await invoke('plugin:servers|servers_create', { options })
}

export async function create_server_from_profile(options: {
	instanceId: string
	saveName?: string | null
	copyMods: boolean
	copyConfig: boolean
	acceptEula: boolean
	ramMb: number
	port: number
}): Promise<ChocoServer> {
	return await invoke('plugin:servers|servers_create_from_profile', options)
}

export async function sync_profile_content(serverId: string): Promise<void> {
	return await invoke('plugin:servers|servers_sync_profile_content', { serverId })
}

export async function delete_server(serverId: string): Promise<void> {
	return await invoke('plugin:servers|servers_delete', { serverId })
}

export async function minecraft_versions(): Promise<string[]> {
	return await invoke('plugin:servers|servers_minecraft_versions')
}

export async function loader_versions(
	loader: ServerLoader,
	gameVersion: string,
): Promise<ServerLoaderVersion[]> {
	return await invoke('plugin:servers|servers_loader_versions', {
		loader,
		gameVersion,
	})
}

export async function run_server(serverId: string): Promise<void> {
	return await invoke('plugin:servers|servers_run', { serverId })
}

export async function stop_server(serverId: string): Promise<void> {
	return await invoke('plugin:servers|servers_stop', { serverId })
}

export async function is_server_running(serverId: string): Promise<boolean> {
	return await invoke('plugin:servers|servers_is_running', { serverId })
}

export async function open_server_folder(serverId: string): Promise<void> {
	return await invoke('plugin:servers|servers_open_folder', { serverId })
}

export type MoveServersReport = {
	moved: { server_id: string; name: string; new_path: string }[]
	failed: { server_id: string; name: string; error: string }[]
}

export async function move_servers(
	serverIds: string[],
	targetDir: string,
): Promise<MoveServersReport> {
	return await invoke('plugin:servers|servers_move', { serverIds, targetDir })
}

export async function default_servers_dir(): Promise<string> {
	return await invoke('plugin:servers|servers_default_dir')
}

export async function accept_server_eula(serverId: string): Promise<void> {
	return await invoke('plugin:servers|servers_accept_eula', { serverId })
}

export type ServerStats = {
	cpu_percent: number
	ram_mb: number
	ram_percent: number
}

export type ServerPing = {
	motd: string | null
	players_online: number
	players_max: number
	players: string[]
	favicon: string | null
	version: string | null
}

export type ServerContentItem = {
	file_name: string
	title: string | null
	version: string | null
	icon_url: string | null
	size: number
	enabled: boolean
}

export async function get_server_stats(serverId: string): Promise<ServerStats> {
	return await invoke('plugin:servers|servers_stats', { serverId })
}

export async function send_server_command(serverId: string, command: string): Promise<void> {
	return await invoke('plugin:servers|servers_command', { serverId, command })
}

export async function ping_server(port: number): Promise<ServerPing> {
	return await invoke('plugin:servers|servers_ping', { port })
}

export async function get_server_properties(
	serverId: string,
): Promise<[string, string][]> {
	return await invoke('plugin:servers|servers_get_properties', { serverId })
}

export async function set_server_properties(
	serverId: string,
	props: [string, string][],
): Promise<void> {
	return await invoke('plugin:servers|servers_set_properties', { serverId, props })
}

export async function list_server_content(serverId: string): Promise<ServerContentItem[]> {
	return await invoke('plugin:servers|servers_list_content', { serverId })
}

export async function set_server_content_enabled(
	serverId: string,
	fileName: string,
	enabled: boolean,
): Promise<void> {
	return await invoke('plugin:servers|servers_set_content_enabled', {
		serverId,
		fileName,
		enabled,
	})
}

export async function delete_server_content(serverId: string, fileName: string): Promise<void> {
	return await invoke('plugin:servers|servers_delete_content', { serverId, fileName })
}

// Global reactive state for server processes and console output
export const runningServers = ref<Record<string, boolean>>({})
export const serverConsoleLines = ref<Record<string, string[]>>({})

let listenersInitialized = false

export async function init_server_listeners(): Promise<void> {
	if (listenersInitialized) return
	listenersInitialized = true

	await listen<{ server_id: string; running: boolean }>('server-status', (event) => {
		runningServers.value[event.payload.server_id] = event.payload.running
		if (!event.payload.running) {
			// leave console lines visible after stop
		}
	})

	await listen<{ server_id: string; line: string }>('server-console', (event) => {
		const { server_id, line } = event.payload
		const lines = serverConsoleLines.value[server_id] ?? []
		lines.push(line)
		// Keep the buffer bounded
		if (lines.length > 500) lines.splice(0, lines.length - 500)
		serverConsoleLines.value = { ...serverConsoleLines.value, [server_id]: lines }
	})
}
