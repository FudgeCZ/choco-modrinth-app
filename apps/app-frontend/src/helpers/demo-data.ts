// Demo view data: fake modpacks, servers, mods, players and console output
// shown when the "Demo view" developer flag is enabled. Pure data — the
// reactive runtime lives in demo-runtime.ts.
import type { ChocoServer, ServerContentItem } from '@/helpers/servers'

export type DemoMod = {
	name: string
	version: string
}

export type DemoModpack = {
	id: string
	name: string
	loader: string
	game_version: string
	mods: DemoMod[]
}

export function isDemoId(id: string | null | undefined): boolean {
	return !!id && id.startsWith('demo:')
}

export const DEMO_MODPACKS: DemoModpack[] = [
	{
		id: 'demo:pack-survival',
		name: 'Demo Pack: Survival',
		loader: 'fabric',
		game_version: '1.21.1',
		mods: [
			{ name: 'Fabric API', version: '0.116.8+1.21.1' },
			{ name: 'Sodium', version: '0.6.13+mc1.21.1' },
			{ name: 'Lithium', version: 'mc1.21.1-0.15.1-fabric' },
			{ name: 'Iris Shaders', version: '1.8.8+1.21.1-fabric' },
			{ name: 'Mod Menu', version: '11.0.3' },
		],
	},
	{
		id: 'demo:pack-tech',
		name: 'Demo Pack: Tech',
		loader: 'neoforge',
		game_version: '1.21.1',
		mods: [
			{ name: 'Applied Energistics 2', version: '19.2.10' },
			{ name: 'Create', version: '6.0.4' },
			{ name: 'JEI', version: '19.21.0.247' },
			{ name: 'FTB Chunks', version: '2101.1.2' },
		],
	},
	{
		id: 'demo:pack-creative',
		name: 'Demo Pack: Creative',
		loader: 'fabric',
		game_version: '26.2',
		mods: [
			{ name: 'Fabric API', version: '0.130.0+26.2' },
			{ name: 'WorldEdit', version: '7.3.9' },
		],
	},
]

export const DEMO_SERVERS: ChocoServer[] = [
	{
		id: 'demo:server-survival',
		name: 'Demo Survival',
		path: 'demo-survival',
		game_version: '26.2',
		loader: 'paper',
		loader_version: 'build #2331',
		jar_file: null,
		ram_mb: 4096,
		port: 25565,
		eula_accepted: true,
		java_path: null,
		linked_instance_id: null,
		icon_file: null,
		created: '2026-09-09T12:00:00.000Z',
	},
	{
		id: 'demo:server-modded',
		name: 'Demo Modded',
		path: 'demo-modded',
		game_version: '1.21.1',
		loader: 'fabric',
		loader_version: '0.16.9 (installer 1.1.0)',
		jar_file: 'server.jar',
		ram_mb: 6144,
		port: 25566,
		eula_accepted: true,
		java_path: null,
		linked_instance_id: null,
		icon_file: null,
		created: '2026-09-09T12:00:00.000Z',
	},
	{
		id: 'demo:server-creative',
		name: 'Demo Creative',
		path: 'demo-creative',
		game_version: '26.2',
		loader: 'vanilla',
		loader_version: null,
		jar_file: 'server.jar',
		ram_mb: 2048,
		port: 25567,
		eula_accepted: true,
		java_path: null,
		linked_instance_id: null,
		icon_file: null,
		created: '2026-09-09T12:00:00.000Z',
	},
]

export const DEMO_CONTENT: Record<string, ServerContentItem[]> = {
	'demo:server-modded': [
		{
			file_name: 'fabric-api-0.116.8.jar',
			title: 'Fabric API',
			version: '0.116.8+1.21.1',
			icon_url: null,
			size: 2_212_352,
			enabled: true,
		},
		{
			file_name: 'sodium-0.6.13.jar',
			title: 'Sodium',
			version: '0.6.13+mc1.21.1',
			icon_url: null,
			size: 1_046_528,
			enabled: true,
		},
		{
			file_name: 'lithium-0.15.1.jar',
			title: 'Lithium',
			version: 'mc1.21.1-0.15.1-fabric',
			icon_url: null,
			size: 684_032,
			enabled: true,
		},
		{
			file_name: 'iris-1.8.8.jar.disabled',
			title: 'Iris Shaders',
			version: '1.8.8+1.21.1-fabric',
			icon_url: null,
			size: 2_547_712,
			enabled: false,
		},
	],
	'demo:server-survival': [
		{
			file_name: 'essentialsx-2.21.0.jar',
			title: 'EssentialsX',
			version: '2.21.0',
			icon_url: null,
			size: 2_129_920,
			enabled: true,
		},
		{
			file_name: 'worldedit-bukkit-7.3.9.jar',
			title: 'WorldEdit',
			version: '7.3.9',
			icon_url: null,
			size: 8_912_896,
			enabled: true,
		},
	],
	'demo:server-creative': [],
}

export const DEMO_PROPERTIES: Record<string, [string, string][]> = {
	'demo:server-survival': [
		['motd', 'Demo Survival — powered by ChocoModrinth'],
		['server-port', '25565'],
		['max-players', '20'],
		['difficulty', 'normal'],
		['gamemode', 'survival'],
		['view-distance', '10'],
		['online-mode', 'true'],
		['pvp', 'true'],
		['white-list', 'false'],
	],
	'demo:server-modded': [
		['motd', 'Demo Modded server'],
		['server-port', '25566'],
		['max-players', '10'],
		['difficulty', 'hard'],
		['gamemode', 'survival'],
		['online-mode', 'true'],
	],
	'demo:server-creative': [
		['motd', 'Demo Creative plot server'],
		['server-port', '25567'],
		['max-players', '30'],
		['gamemode', 'creative'],
	],
}

export const DEMO_PLAYERS: Record<string, string[]> = {
	'demo:server-survival': ['Steve', 'Alex', 'jeb_'],
	'demo:server-modded': ['DemoPlayer'],
	'demo:server-creative': [],
}

export const DEMO_CONSOLE_SEED: Record<string, string[]> = {
	'demo:server-survival': [
		'[21:00:01] [ServerMain/INFO]: Starting minecraft server version 26.2',
		'[21:00:03] [Server thread/INFO]: Starting minecraft server on *:25565',
		'[21:00:04] [Server thread/INFO]: Preparing level "world"',
		'[21:00:06] [Server thread/INFO]: Done (12.480s)! For help, type "help"',
		'[21:00:11] [Server thread/INFO]: Steve joined the game',
		'[21:00:42] [Server thread/INFO]: <Steve> anyone at spawn?',
		'[21:01:02] [Server thread/INFO]: Alex joined the game',
		'[21:01:20] [Server thread/INFO]: <Alex> hey, nice base',
	],
	'demo:server-modded': [
		'[20:12:00] [ServerMain/INFO]: Starting minecraft server version 1.21.1',
		'[20:12:31] [Server thread/INFO]: Done (24.113s)! For help, type "help"',
		'[20:13:02] [Server thread/INFO]: DemoPlayer joined the game',
	],
	'demo:server-creative': [],
}

export const DEMO_CONSOLE_IDLE_LINES: string[] = [
	'[Server thread/INFO]: Saving the game (this will take a moment!)',
	'[Server thread/INFO]: Saving chunks for level',
	'[Server thread/INFO]: ThreadedAnvilChunkStorage: All dimensions are saved',
	'[Server thread/INFO]: Server empty for 60 seconds, pausing',
]

export const DEMO_CHAT_ROTATION: { speaker: string; message: string }[] = [
	{ speaker: 'Steve', message: 'anyone want to trade?' },
	{ speaker: 'Alex', message: 'brb getting diamonds' },
	{ speaker: 'jeb_', message: 'the nether portal is done' },
	{ speaker: 'Steve', message: 'meeting at spawn in 5' },
]

export const DEMO_MODPACK_MODS_MODAL_NOTE =
	'This is a demo modpack shown in Demo view. Content is read-only.'
