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
