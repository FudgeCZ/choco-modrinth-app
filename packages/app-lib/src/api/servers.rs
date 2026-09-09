//! ChocoModrinth local server creator: downloads and manages local Minecraft
//! servers (jar downloads, EULA, server.properties, start scripts).

use crate::state::{ModLoader, State};
use crate::util::fetch::{self, DownloadMeta};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const VANILLA_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const FORGE_PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const NEOFORGE_VERSIONS_URL: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
const NEOFORGE_MAVEN: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const QUILT_META: &str = "https://meta.quiltmc.org/v3";
const PAPER_API: &str = "https://api.papermc.io/v2/projects/paper";
const PURPUR_API: &str = "https://api.purpurmc.org/v2/purpur";
const SERVERS_INDEX_FILE: &str = "index.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerLoader {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
    Paper,
    Purpur,
}

impl ServerLoader {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Quilt => "quilt",
            Self::Paper => "paper",
            Self::Purpur => "purpur",
        }
    }

    pub fn from_str_value(value: &str) -> crate::Result<Self> {
        match value {
            "vanilla" => Ok(Self::Vanilla),
            "fabric" => Ok(Self::Fabric),
            "forge" => Ok(Self::Forge),
            "neoforge" => Ok(Self::NeoForge),
            "quilt" => Ok(Self::Quilt),
            "paper" => Ok(Self::Paper),
            "purpur" => Ok(Self::Purpur),
            other => Err(crate::ErrorKind::InputError(format!(
                "Unknown server loader {other}"
            ))
            .into()),
        }
    }

    /// Loaders whose server jar is run directly via `java -jar`
    pub fn uses_plain_jar(self) -> bool {
        matches!(
            self,
            Self::Vanilla | Self::Fabric | Self::Quilt | Self::Paper | Self::Purpur
        )
    }

    pub fn from_mod_loader(loader: ModLoader) -> Self {
        match loader {
            ModLoader::Vanilla => Self::Vanilla,
            ModLoader::Fabric => Self::Fabric,
            ModLoader::Forge => Self::Forge,
            ModLoader::Quilt => Self::Quilt,
            ModLoader::NeoForge => Self::NeoForge,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChocoServer {
    pub id: String,
    pub name: String,
    pub path: String,
    pub game_version: String,
    pub loader: ServerLoader,
    pub loader_version: Option<String>,
    /// Main jar file name inside the server folder (e.g. server.jar)
    pub jar_file: Option<String>,
    pub ram_mb: u32,
    pub port: u16,
    pub eula_accepted: bool,
    /// Path of the Java runtime managed by the launcher
    pub java_path: Option<String>,
    /// Instance this server was created from and stays linked to (mods/config sync)
    #[serde(default)]
    pub linked_instance_id: Option<String>,
    /// Server icon file inside the server folder (for the Servers page)
    #[serde(default)]
    pub icon_file: Option<String>,
    pub created: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateServerOptions {
    pub name: String,
    pub game_version: String,
    pub loader: ServerLoader,
    pub loader_version: Option<String>,
    pub ram_mb: u32,
    pub port: u16,
    pub motd: Option<String>,
    pub difficulty: Option<String>,
    pub gamemode: Option<String>,
    pub max_players: Option<u32>,
    pub online_mode: Option<bool>,
    pub accept_eula: bool,
    #[serde(default)]
    pub level_name: Option<String>,
    #[serde(default)]
    pub linked_instance_id: Option<String>,
    /// Source icon file (e.g. the profile icon) to copy into the server folder
    #[serde(default)]
    pub icon_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerLoaderVersion {
    pub id: String,
    pub recommended: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct ServersIndex {
    servers: Vec<ChocoServer>,
}

fn servers_index_path(state: &State) -> PathBuf {
    state.directories.servers_dir().join(SERVERS_INDEX_FILE)
}

pub async fn list_servers() -> crate::Result<Vec<ChocoServer>> {
    let state = State::get().await?;
    let path = servers_index_path(&state);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| crate::ErrorKind::FSError(format!("Failed to read servers index: {e}")))?;
    let index: ServersIndex = serde_json::from_str(&content)
        .map_err(|e| crate::ErrorKind::FSError(format!("Failed to parse servers index: {e}")))?;
    Ok(index.servers)
}

async fn write_servers_index(state: &State, servers: &[ChocoServer]) -> crate::Result<()> {
    let dir = state.directories.servers_dir();
    crate::util::io::create_dir_all(&dir).await?;
    let content = serde_json::to_string_pretty(&ServersIndex {
        servers: servers.to_vec(),
    })?;
    std::fs::write(servers_index_path(state), content)
        .map_err(|e| crate::ErrorKind::FSError(format!("Failed to write servers index: {e}")))?;
    Ok(())
}

fn sanitize_server_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | '?' | '*' | ':' | '"' | '<' | '>' | '|') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        "server".to_string()
    } else {
        trimmed.to_string()
    }
}

fn server_dir(state: &State, path: &str) -> PathBuf {
    state.directories.servers_dir().join(path)
}

pub async fn delete_server(server_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let mut servers = list_servers().await?;
    if let Some(pos) = servers.iter().position(|s| s.id == server_id) {
        let server = servers.remove(pos);
        let dir = server_dir(&state, &server.path);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await.map_err(|e| {
                crate::ErrorKind::FSError(format!("Failed to delete server folder: {e}"))
            })?;
        }
        write_servers_index(&state, &servers).await?;
    }
    Ok(())
}

/// Minecraft release versions available for server creation
pub async fn minecraft_server_versions() -> crate::Result<Vec<String>> {
    let manifest = crate::api::metadata::get_minecraft_versions().await?;
    Ok(manifest
        .versions
        .into_iter()
        .filter(|v| matches!(v.type_, daedalus::minecraft::VersionType::Release))
        .map(|v| v.id)
        .collect())
}

/// Loader versions offered for a given loader + game version
pub async fn server_loader_versions(
    loader: ServerLoader,
    game_version: &str,
) -> crate::Result<Vec<ServerLoaderVersion>> {
    let state = State::get().await?;
    let semaphore = &state.api_semaphore;
    let exec = &state.pool;

    let out = match loader {
        ServerLoader::Vanilla => Vec::new(),
        ServerLoader::Fabric => {
            #[derive(Deserialize)]
            struct FabricInstaller {
                version: String,
            }
            let installers: Vec<FabricInstaller> =
                fetch::fetch_json(reqwest::Method::GET, &format!("{FABRIC_META}/versions/installer"), None, None, None, semaphore, exec).await?;
            vec![ServerLoaderVersion {
                id: installers
                    .first()
                    .map(|i| i.version.clone())
                    .unwrap_or_else(|| "latest".to_string()),
                recommended: true,
            }]
        }
        ServerLoader::Quilt => {
            #[derive(Deserialize)]
            struct QuiltInstaller {
                version: String,
            }
            let installers: Vec<QuiltInstaller> =
                fetch::fetch_json(reqwest::Method::GET, &format!("{QUILT_META}/versions/installer"), None, None, None, semaphore, exec).await?;
            vec![ServerLoaderVersion {
                id: installers
                    .first()
                    .map(|i| i.version.clone())
                    .unwrap_or_else(|| "latest".to_string()),
                recommended: true,
            }]
        }
        ServerLoader::Forge => {
            #[derive(Deserialize)]
            struct Promotions {
                promos: std::collections::HashMap<String, String>,
            }
            let promos: Promotions =
                fetch::fetch_json(reqwest::Method::GET, FORGE_PROMOTIONS_URL, None, None, None, semaphore, exec).await?;
            let prefix = format!("{game_version}-");
            let mut versions: Vec<ServerLoaderVersion> = Vec::new();
            if let Some(recommended) = promos.promos.get(&format!("{prefix}recommended")) {
                versions.push(ServerLoaderVersion {
                    id: recommended.clone(),
                    recommended: true,
                });
            }
            if let Some(latest) = promos.promos.get(&format!("{prefix}latest")) {
                if !versions.iter().any(|v| v.id == *latest) {
                    versions.push(ServerLoaderVersion {
                        id: latest.clone(),
                        recommended: false,
                    });
                }
            }
            versions
        }
        ServerLoader::NeoForge => {
            // NeoForge versions are built per MC version, e.g. MC 1.21.1 -> 21.1.x
            let prefix = game_version
                .strip_prefix("1.")
                .unwrap_or(game_version)
                .to_string();
            #[derive(Deserialize)]
            struct MavenVersions {
                #[serde(rename = "versions")]
                list: Vec<String>,
            }
            let maven: MavenVersions =
                fetch::fetch_json(reqwest::Method::GET, NEOFORGE_VERSIONS_URL, None, None, None, semaphore, exec).await?;
            maven.list
                .into_iter()
                .filter(|v| v.starts_with(&format!("{prefix}.")) || v.starts_with(&format!("{prefix}-")))
                .map(|v| ServerLoaderVersion {
                    id: v,
                    recommended: false,
                })
                .collect()
        }
        ServerLoader::Paper => {
            #[derive(Deserialize)]
            struct BuildsResponse {
                builds: Vec<PaperBuild>,
            }
            #[derive(Deserialize)]
            struct PaperBuild {
                build: u32,
            }
            let builds: BuildsResponse = fetch::fetch_json(
                reqwest::Method::GET,
                &format!("{PAPER_API}/versions/{game_version}/builds"),
                None,
                None,
                None,
                semaphore,
                exec,
            )
            .await?;
            builds
                .builds
                .into_iter()
                .map(|b| ServerLoaderVersion {
                    id: b.build.to_string(),
                    recommended: false,
                })
                .collect()
        }
        ServerLoader::Purpur => {
            #[derive(Deserialize)]
            struct PurpurVersion {
                builds: PurpurBuilds,
            }
            #[derive(Deserialize)]
            struct PurpurBuilds {
                latest: String,
            }
            let version: PurpurVersion = fetch::fetch_json(
                reqwest::Method::GET,
                &format!("{PURPUR_API}/{game_version}"),
                None,
                None,
                None,
                semaphore,
                exec,
            )
            .await?;
            vec![ServerLoaderVersion {
                id: version.builds.latest,
                recommended: true,
            }]
        }
    };

    Ok(out)
}

async fn download_to_file(
    state: &State,
    url: &str,
    sha1: Option<&str>,
    dest: &Path,
) -> crate::Result<()> {
    let file = fetch::fetch_file(
        url,
        sha1,
        None::<&DownloadMeta>,
        None,
        &state.api_semaphore,
        &state.pool,
        None,
    )
    .await?;
    if let Some(parent) = dest.parent() {
        crate::util::io::create_dir_all(parent).await?;
    }
    file.copy_to(dest, &state.io_semaphore).await?;
    Ok(())
}

/// Mojang java version requirement for a game version
fn required_java_version(game_version: &str) -> u32 {
    fn parse(version: &str) -> (u32, u32, u32) {
        let mut parts = version
            .split('.')
            .map(|p| p.split('-').next().unwrap_or("0").parse::<u32>().unwrap_or(0));
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    }
    let (major, minor, patch) = parse(game_version);
    if (major, minor, patch) >= (1, 20, 5) || (major, minor) >= (1, 21) {
        21
    } else if (major, minor) >= (1, 17) {
        17
    } else {
        8
    }
}

async fn download_vanilla_server(state: &State, game_version: &str, dir: &Path) -> crate::Result<()> {
    #[derive(Deserialize)]
    struct Manifest {
        versions: Vec<ManifestVersion>,
    }
    #[derive(Deserialize)]
    struct ManifestVersion {
        id: String,
        url: String,
    }
    #[derive(Deserialize)]
    struct VersionJson {
        downloads: Downloads,
    }
    #[derive(Deserialize)]
    struct Downloads {
        server: ServerDownload,
    }
    #[derive(Deserialize)]
    struct ServerDownload {
        url: String,
        sha1: String,
    }

    let semaphore = &state.api_semaphore;
    let exec = &state.pool;
    let manifest: Manifest =
        fetch::fetch_json(reqwest::Method::GET, VANILLA_MANIFEST_URL, None, None, None, semaphore, exec).await?;
    let version = manifest
        .versions
        .into_iter()
        .find(|v| v.id == game_version)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Unknown Minecraft version {game_version}"
            ))
        })?;
    let version_json: VersionJson =
        fetch::fetch_json(reqwest::Method::GET, &version.url, None, None, None, semaphore, exec).await?;
    download_to_file(
        state,
        &version_json.downloads.server.url,
        Some(&version_json.downloads.server.sha1),
        &dir.join("server.jar"),
    )
    .await
}

async fn download_fabric_quilt_server(
    state: &State,
    is_quilt: bool,
    game_version: &str,
    loader_version: Option<&str>,
    dir: &Path,
) -> crate::Result<String> {
    let (meta, loader_name) = if is_quilt {
        (QUILT_META, "quilt")
    } else {
        (FABRIC_META, "fabric")
    };

    // installer endpoint shape is shared between fabric v2 and quilt v3
    #[derive(Deserialize)]
    struct InstallerVersion {
        version: String,
    }
    #[derive(Deserialize)]
    struct LoaderList {
        loader: LoaderVersion,
    }
    #[derive(Deserialize)]
    struct LoaderVersion {
        version: String,
    }
    let semaphore = &state.api_semaphore;
    let exec = &state.pool;

    let installer: String = match fetch::fetch_json::<Vec<InstallerVersion>>(
        reqwest::Method::GET,
        &format!("{meta}/versions/installer"),
        None,
        None,
        None,
        semaphore,
        exec,
    )
    .await
    .map(|list| list.first().map(|i| i.version.clone()))
    {
        Ok(Some(v)) => v,
        Ok(None) => "latest".to_string(),
        Err(_) => "latest".to_string(),
    };

    let loader_version = match loader_version {
        Some(v) => v.to_string(),
        None => match fetch::fetch_json::<Vec<LoaderList>>(
            reqwest::Method::GET,
            &format!("{meta}/versions/loader/{game_version}"),
            None,
            None,
            None,
            semaphore,
            exec,
        )
        .await
        .map(|list| list.first().map(|l| l.loader.version.clone()))
        {
            Ok(Some(v)) => v,
            _ => {
                return Err(crate::ErrorKind::InputError(format!(
                    "No {loader_name} loader version found for Minecraft {game_version}"
                ))
                .into())
            }
        },
    };

    let url = format!(
        "{meta}/versions/loader/{game_version}/{loader_version}/{installer}/server/jar"
    );
    download_to_file(state, &url, None, &dir.join("server.jar")).await?;
    Ok(format!("{loader_version} (installer {installer})"))
}

async fn download_paper_server(
    state: &State,
    game_version: &str,
    build: Option<&str>,
    dir: &Path,
) -> crate::Result<String> {
    #[derive(Deserialize)]
    struct BuildsResponse {
        builds: Vec<PaperBuild>,
    }
    #[derive(Deserialize)]
    struct PaperBuild {
        build: u32,
        downloads: PaperDownloads,
    }
    #[derive(Deserialize)]
    struct PaperDownloads {
        application: PaperApplication,
    }
    #[derive(Deserialize)]
    struct PaperApplication {
        name: String,
        url: String,
    }
    let semaphore = &state.api_semaphore;
    let exec = &state.pool;
    let builds: BuildsResponse = fetch::fetch_json(
        reqwest::Method::GET,
        &format!("{PAPER_API}/versions/{game_version}/builds"),
        None,
        None,
        None,
        semaphore,
        exec,
    )
    .await?;
    let chosen = match build {
        Some(b) => builds
            .builds
            .into_iter()
            .find(|x| x.build.to_string() == b),
        None => builds.builds.into_iter().last(),
    }
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "No Paper build found for Minecraft {game_version}"
        ))
    })?;
    download_to_file(
        state,
        &chosen.downloads.application.url,
        None,
        &dir.join(&chosen.downloads.application.name),
    )
    .await?;
    Ok(chosen.downloads.application.name)
}

async fn download_purpur_server(
    state: &State,
    game_version: &str,
    build: Option<&str>,
    dir: &Path,
) -> crate::Result<String> {
    #[derive(Deserialize)]
    struct PurpurVersion {
        builds: PurpurBuilds,
    }
    #[derive(Deserialize)]
    struct PurpurBuilds {
        latest: String,
    }
    let semaphore = &state.api_semaphore;
    let exec = &state.pool;
    let version: PurpurVersion = fetch::fetch_json(
        reqwest::Method::GET,
        &format!("{PURPUR_API}/{game_version}"),
        None,
        None,
        None,
        semaphore,
        exec,
    )
    .await?;
    let build = build.unwrap_or(&version.builds.latest).to_string();
    let url = format!("{PURPUR_API}/{game_version}/{build}/download");
    let file_name = format!("purpur-{game_version}-{build}.jar");
    download_to_file(state, &url, None, &dir.join(&file_name)).await?;
    Ok(file_name)
}

async fn run_installer(
    dir: &Path,
    java_path: &Path,
    installer_file: &str,
) -> crate::Result<()> {
    let output = tokio::process::Command::new(java_path)
        .arg("-jar")
        .arg(installer_file)
        .arg("--installServer")
        .current_dir(dir)
        .output()
        .await
        .map_err(|e| {
            crate::ErrorKind::LauncherError(format!("Failed to run server installer: {e}"))
        })?;

    if !output.status.success() {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Server installer failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
        .as_error());
    }
    Ok(())
}

async fn download_forge_server(
    state: &State,
    game_version: &str,
    forge_version: Option<&str>,
    java_path: &Path,
    dir: &Path,
) -> crate::Result<String> {
    #[derive(Deserialize)]
    struct Promotions {
        promos: std::collections::HashMap<String, String>,
    }
    let semaphore = &state.api_semaphore;
    let exec = &state.pool;

    let forge_version = match forge_version {
        Some(v) => v.to_string(),
        None => {
            let promos: Promotions =
                fetch::fetch_json(reqwest::Method::GET, FORGE_PROMOTIONS_URL, None, None, None, semaphore, exec).await?;
            let prefix = format!("{game_version}-");
            promos
                .promos
                .get(&format!("{prefix}recommended"))
                .or_else(|| promos.promos.get(&format!("{prefix}latest")))
                .cloned()
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "No Forge version found for Minecraft {game_version}"
                    ))
                })?
        }
    };

    let installer_name = format!("forge-{game_version}-{forge_version}-installer.jar");
    let url = format!("{FORGE_MAVEN}/{game_version}-{forge_version}/{installer_name}");
    download_to_file(state, &url, None, &dir.join(&installer_name)).await?;
    run_installer(dir, java_path, &installer_name).await?;
    let _ = std::fs::remove_file(dir.join(&installer_name));
    Ok(forge_version)
}

async fn download_neoforge_server(
    state: &State,
    game_version: &str,
    neoforge_version: Option<&str>,
    java_path: &Path,
    dir: &Path,
) -> crate::Result<String> {
    let neoforge_version = match neoforge_version {
        Some(v) => v.to_string(),
        None => {
            let prefix = game_version
                .strip_prefix("1.")
                .unwrap_or(game_version)
                .to_string();
            #[derive(Deserialize)]
            struct MavenVersions {
                #[serde(rename = "versions")]
                list: Vec<String>,
            }
            let semaphore = &state.api_semaphore;
            let exec = &state.pool;
            let maven: MavenVersions = fetch::fetch_json(
                reqwest::Method::GET,
                NEOFORGE_VERSIONS_URL,
                None,
                None,
                None,
                semaphore,
                exec,
            )
            .await?;
            maven
                .list
                .into_iter()
                .filter(|v| v.starts_with(&format!("{prefix}.")) || v.starts_with(&format!("{prefix}-")))
                .last()
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "No NeoForge version found for Minecraft {game_version}"
                    ))
                })?
        }
    };

    let installer_name = format!("neoforge-{neoforge_version}-installer.jar");
    let url = format!("{NEOFORGE_MAVEN}/{neoforge_version}/{installer_name}");
    download_to_file(state, &url, None, &dir.join(&installer_name)).await?;
    run_installer(dir, java_path, &installer_name).await?;
    let _ = std::fs::remove_file(dir.join(&installer_name));
    Ok(neoforge_version)
}

fn write_text_file(path: &Path, content: &str) -> crate::Result<()> {
    std::fs::write(path, content).map_err(|e| {
        crate::ErrorKind::FSError(format!("Failed to write {}: {e}", path.display()))
            .as_error()
    })?;
    Ok(())
}

fn write_server_config(
    dir: &Path,
    opts: &CreateServerOptions,
    jar_file: &str,
    java_path: &Path,
) -> crate::Result<()> {
    // EULA: only accepted when explicitly confirmed by the user
    write_text_file(
        &dir.join("eula.txt"),
        &format!(
            "# Accepted via ChocoModrinth server creator on {}\neula={}\n",
            Utc::now().to_rfc3339(),
            opts.accept_eula
        ),
    )?;

    let properties = format!(
        "#server.properties generated by ChocoModrinth\n\
         server-port={}\n\
         motd={}\n\
         difficulty={}\n\
         gamemode={}\n\
         max-players={}\n\
         online-mode={}\n\
         enable-rcon=false\n\
         view-distance=10\n\
         level-name={}\n",
        opts.port,
        opts.motd.as_deref().unwrap_or("A ChocoModrinth server"),
        opts.difficulty.as_deref().unwrap_or("normal"),
        opts.gamemode.as_deref().unwrap_or("survival"),
        opts.max_players.unwrap_or(20),
        opts.online_mode.unwrap_or(true),
        opts.level_name.as_deref().unwrap_or("world"),
    );
    write_text_file(&dir.join("server.properties"), &properties)?;

    let java_arg = java_path.to_string_lossy();
    let uses_plain_jar = opts.loader.uses_plain_jar();
    let java_command = if uses_plain_jar {
        format!("-Xmx{}M -Xms{}M -jar {jar_file} nogui", opts.ram_mb, opts.ram_mb)
    } else {
        // Forge / NeoForge generate their own run scripts during installation
        "nogui".to_string()
    };

    #[cfg(windows)]
    let script = if uses_plain_jar {
        format!(
            "@echo off\r\ncd /d \"%~dp0\"\r\n\"{}\" {}\r\npause\r\n",
            java_arg, java_command
        )
    } else {
        "call run.bat\r\npause\r\n".to_string()
    };
    #[cfg(not(windows))]
    let script = if uses_plain_jar {
        format!(
            "#!/usr/bin/env bash\ncd \"$(dirname \"$0\")\"\n\"{}\" {}\n",
            java_arg, java_command
        )
    } else {
        "./run.sh\n".to_string()
    };
    write_text_file(&dir.join("start-chocomodrinth.txt"), &script)?;

    Ok(())
}

pub async fn create_server(opts: CreateServerOptions) -> crate::Result<ChocoServer> {
    let state = State::get().await?;
    let mut servers = list_servers().await?;

    let name = sanitize_server_name(&opts.name);
    let mut path = name.clone();
    let mut suffix = 2;
    while servers.iter().any(|s| s.path == path) {
        path = format!("{name} ({suffix})");
        suffix += 1;
    }

    let dir = server_dir(&state, &path);
    crate::util::io::create_dir_all(&dir).await?;

    // Resolve a Java runtime for installation and running the server
    let java_path =
        crate::api::jre::auto_install_java(required_java_version(&opts.game_version)).await?;

    let (jar_file, resolved_loader_version) = match opts.loader {
        ServerLoader::Vanilla => {
            download_vanilla_server(&state, &opts.game_version, &dir).await?;
            ("server.jar".to_string(), None)
        }
        ServerLoader::Fabric => {
            let version =
                download_fabric_quilt_server(&state, false, &opts.game_version, opts.loader_version.as_deref(), &dir)
                    .await?;
            ("server.jar".to_string(), Some(version))
        }
        ServerLoader::Quilt => {
            let version =
                download_fabric_quilt_server(&state, true, &opts.game_version, opts.loader_version.as_deref(), &dir)
                    .await?;
            ("server.jar".to_string(), Some(version))
        }
        ServerLoader::Paper => {
            let file = download_paper_server(
                &state,
                &opts.game_version,
                opts.loader_version.as_deref(),
                &dir,
            )
            .await?;
            (file, opts.loader_version.clone())
        }
        ServerLoader::Purpur => {
            let file = download_purpur_server(
                &state,
                &opts.game_version,
                opts.loader_version.as_deref(),
                &dir,
            )
            .await?;
            (file, opts.loader_version.clone())
        }
        ServerLoader::Forge => {
            let version = download_forge_server(
                &state,
                &opts.game_version,
                opts.loader_version.as_deref(),
                &java_path,
                &dir,
            )
            .await?;
            (String::new(), Some(version))
        }
        ServerLoader::NeoForge => {
            let version = download_neoforge_server(
                &state,
                &opts.game_version,
                opts.loader_version.as_deref(),
                &java_path,
                &dir,
            )
            .await?;
            (String::new(), Some(version))
        }
    };

    write_server_config(&dir, &opts, &jar_file, &java_path)?;

    // Profile icon → server-icon.png (shown in the multiplayer server list)
    let mut icon_file = None;
    if let Some(icon_src) = &opts.icon_path {
        let src = Path::new(icon_src);
        let is_png = src
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"));
        if src.exists() && is_png {
            let dest = dir.join("server-icon.png");
            if tokio::fs::copy(src, &dest).await.is_ok() {
                icon_file = Some("server-icon.png".to_string());
            }
        }
    }

    let server = ChocoServer {
        id: format!("local:{}", uuid::Uuid::new_v4()),
        name: opts.name.clone(),
        path,
        game_version: opts.game_version.clone(),
        loader: opts.loader,
        loader_version: resolved_loader_version.or(opts.loader_version),
        jar_file: if jar_file.is_empty() { None } else { Some(jar_file) },
        ram_mb: opts.ram_mb,
        port: opts.port,
        eula_accepted: opts.accept_eula,
        java_path: Some(java_path.to_string_lossy().to_string()),
        linked_instance_id: opts.linked_instance_id.clone(),
        icon_file,
        created: Utc::now(),
    };

    servers.push(server.clone());
    write_servers_index(&state, &servers).await?;

    Ok(server)
}

#[derive(Debug, Serialize)]
pub struct ProfileSyncReport {
    pub mods_copied: u32,
    pub mods_skipped_client_only: u32,
    pub config_copied: bool,
    pub world_copied: bool,
}

fn sanitize_level_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if cleaned.is_empty() { "world".to_string() } else { cleaned }
}

/// Creates a server from an existing profile (instance): inherits the game
/// version and loader, optionally copies a save as the server world plus mods
/// (client-only mods excluded) and the config folder.
#[allow(clippy::too_many_arguments)]
pub async fn create_server_from_profile(
    instance_id: String,
    save_name: Option<String>,
    copy_mods: bool,
    copy_config: bool,
    accept_eula: bool,
    ram_mb: u32,
    port: u16,
) -> crate::Result<ChocoServer> {
    let state = State::get().await?;
    let metadata = crate::state::get_instance(&instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Profile {instance_id} was not found"
            ))
        })?;

    let save_name = match &save_name {
        Some(name) => Some(sanitize_level_name(name)),
        None => None,
    };

    let options = CreateServerOptions {
        name: format!("{} Server", metadata.instance.name),
        game_version: metadata.applied_content_set.game_version.clone(),
        loader: ServerLoader::from_mod_loader(metadata.applied_content_set.loader),
        loader_version: metadata.applied_content_set.loader_version.clone(),
        ram_mb,
        port,
        motd: save_name.clone(),
        difficulty: Some("normal".to_string()),
        gamemode: Some("survival".to_string()),
        max_players: Some(20),
        online_mode: Some(true),
        accept_eula,
        level_name: save_name.clone(),
        linked_instance_id: Some(instance_id.clone()),
        icon_path: metadata.instance.icon_path.clone(),
    };

    let server = create_server(options).await?;

    let server_dir = state.directories.servers_dir().join(&server.path);
    let instance_dir = state
        .directories
        .instances_dir()
        .join(&metadata.instance.path);

    // Copy the selected save as the server world
    if let Some(save) = &save_name {
        let source = instance_dir.join("saves").join(save);
        if source.is_dir() {
            copy_dir_all(&source, &server_dir.join(save)).await?;
        }
    }

    // Copy mods (client-only excluded) and config folder
    sync_profile_content(
        &server,
        &instance_id,
        copy_mods,
        copy_config,
    )
    .await?;

    Ok(server)
}

/// Re-copies mods (client-only excluded) and config from the linked profile
pub async fn sync_profile_server(server_id: String) -> crate::Result<ProfileSyncReport> {
    let servers = list_servers().await?;
    let server = servers
        .into_iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Server not found".to_string())
        })?;

    let instance_id = server.linked_instance_id.clone().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "This server is not linked to a profile".to_string(),
        )
    })?;

    sync_profile_content(&server, &instance_id, true, true).await
}

/// Copies mods (excluding client-only mods reported by Modrinth) and the
/// config folder from the linked profile into the server folder
async fn sync_profile_content(
    server: &ChocoServer,
    instance_id: &str,
    copy_mods: bool,
    copy_config: bool,
) -> crate::Result<ProfileSyncReport> {
    let state = State::get().await?;
    let instance_dir = state
        .directories
        .instances_dir()
        .join(&server.path)
        .clone();
    let _ = instance_dir;
    let instance_path = crate::api::instance::get_full_path(instance_id).await?;
    let server_dir = state.directories.servers_dir().join(&server.path);

    let mut report = ProfileSyncReport {
        mods_copied: 0,
        mods_skipped_client_only: 0,
        config_copied: false,
        world_copied: false,
    };

    if copy_config {
        let config_src = instance_path.join("config");
        if config_src.is_dir() {
            let config_dest = server_dir.join("config");
            if config_dest.exists() {
                tokio::fs::remove_dir_all(&config_dest)
                    .await
                    .map_err(|e| crate::util::io::IOError::with_path(e, &config_dest))?;
            }
            copy_dir_all(&config_src, &config_dest).await?;
            report.config_copied = true;
        }
    }

    if copy_mods {
        let mods_src = instance_path.join("mods");
        if mods_src.is_dir() {
            let mods_dest = server_dir.join("mods");
            if mods_dest.exists() {
                tokio::fs::remove_dir_all(&mods_dest)
                    .await
                    .map_err(|e| crate::util::io::IOError::with_path(e, &mods_dest))?;
            }
            tokio::fs::create_dir_all(&mods_dest)
                .await
                .map_err(|e| crate::util::io::IOError::with_path(e, &mods_dest))?;

            // Map mod files to their Modrinth projects to read the mod environment
            let projects = crate::state::get_content_projects(
                instance_id,
                None,
                None,
                &state,
            )
            .await?;

            let mut allowed_files: Vec<PathBuf> = Vec::new();
            let mut project_ids: Vec<String> = Vec::new();
            let mut file_projects: Vec<(PathBuf, String)> = Vec::new();

            let mut entries = tokio::fs::read_dir(&mods_src)
                .await
                .map_err(|e| crate::util::io::IOError::with_path(e, &mods_src))?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| crate::util::io::IOError::with_path(e, &mods_src))?
            {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !file_name.ends_with(".jar") || file_name.ends_with(".disabled") {
                    continue;
                }

                // Look up this file's Modrinth project (if it came from Modrinth)
                let project_id = projects
                    .iter()
                    .find(|entry| {
                        entry.key().ends_with(&file_name)
                            && entry
                                .value()
                                .metadata
                                .as_ref()
                                .map(|m| !m.project_id.is_empty())
                                .unwrap_or(false)
                    })
                    .and_then(|entry| {
                        entry.value().metadata.as_ref().map(|m| m.project_id.clone())
                    });

                if let Some(project_id) = project_id {
                    project_ids.push(project_id.clone());
                    file_projects.push((path, project_id));
                } else {
                    // Not tracked as a Modrinth project: include it (user-added
                    // mods are assumed intentional) unless we can prove otherwise
                    allowed_files.push(path);
                }
            }

            // Ask Modrinth which projects are client-side only
            let mut client_only: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            if !project_ids.is_empty() {
                project_ids.dedup();
                let url = format!("{}v2/projects", env!("MODRINTH_API_BASE_URL"));
                match fetch::fetch_json::<Vec<ModrinthProject>>(
                    reqwest::Method::POST,
                    &url,
                    None,
                    Some(serde_json::json!(project_ids)),
                    None,
                    &state.api_semaphore,
                    &state.pool,
                )
                .await
                {
                    Ok(projects) => {
                        for project in projects {
                            if project.server_side.as_deref() == Some("unsupported") {
                                if let Some(id) = project.id {
                                    client_only.insert(id);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Could not query Modrinth for mod environments: {e}; copying all mods"
                        );
                    }
                }
            }

            for (path, project_id) in file_projects {
                if client_only.contains(&project_id) {
                    report.mods_skipped_client_only += 1;
                } else {
                    allowed_files.push(path);
                }
            }

            for file in allowed_files {
                let file_name = file
                    .file_name()
                    .map(|n| n.to_os_string())
                    .unwrap_or_default();
                let dest = mods_dest.join(&file_name);
                tokio::fs::copy(&file, &dest)
                    .await
                    .map_err(|e| crate::util::io::IOError::with_path(e, &file))?;
                report.mods_copied += 1;
            }
        }
    }

    Ok(report)
}

/// Recursively copies a directory, overwriting existing files
pub fn copy_dir_all<'a>(
    src: &'a Path,
    dest: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        if !src.is_dir() {
            return Ok(());
        }
        tokio::fs::create_dir_all(dest)
            .await
            .map_err(|e| crate::util::io::IOError::with_path(e, dest))?;
        let mut entries = tokio::fs::read_dir(src)
            .await
            .map_err(|e| crate::util::io::IOError::with_path(e, src))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| crate::util::io::IOError::with_path(e, src))?
        {
            let target = dest.join(entry.file_name());
            let source = entry.path();
            if source.is_dir() {
                copy_dir_all(&source, &target).await?;
            } else {
                tokio::fs::copy(&source, &target)
                    .await
                    .map_err(|e| crate::util::io::IOError::with_path(e, &source))?;
            }
        }
        Ok(())
    })
}

#[derive(Debug, Deserialize)]
struct ModrinthProject {
    id: Option<String>,
    #[serde(default)]
    server_side: Option<String>,
}
