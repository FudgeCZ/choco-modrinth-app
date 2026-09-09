//! ChocoModrinth local server creator: downloads and manages local Minecraft
//! servers (jar downloads, EULA, server.properties, start scripts).

use crate::event::emit::{emit_loading, init_loading};
use crate::event::{LoadingBarId, LoadingBarType};
use crate::state::{ModLoader, State};
use crate::util::fetch;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

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

/// Resolves a server's folder: an absolute `path` is used as-is (servers
/// living in a custom servers directory), relative paths resolve against the
/// default servers directory.
pub fn server_dir(state: &State, path: &str) -> PathBuf {
    if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        state.directories.servers_dir().join(path)
    }
}

/// The default servers directory (used as a move target for "reset").
pub async fn default_servers_dir() -> crate::Result<String> {
    let state = State::get().await?;
    Ok(state
        .directories
        .servers_dir()
        .to_string_lossy()
        .to_string())
}

/// Moves the given servers into `target_dir` (creating it if needed),
/// updating the servers index afterwards. Cross-drive moves are supported
/// (copy + delete), with a progress bar per server. Running servers must be
/// stopped first (checked by the caller).
pub async fn move_servers_to_dir(
    server_ids: Vec<String>,
    target_dir: String,
) -> crate::Result<MoveServersReport> {
    let state = State::get().await?;

    let target = PathBuf::from(&target_dir);
    if !target.exists() {
        std::fs::create_dir_all(&target)
            .map_err(|e| crate::util::io::IOError::with_path(e, &target))?;
    }
    let target = crate::util::io::canonicalize(target)?;
    let default_dir =
        crate::util::io::canonicalize(state.directories.servers_dir().clone())?;
    // When moving back into the default directory, store relative paths again
    let into_default = target == default_dir;

    let mut servers = list_servers().await?;
    let mut moved = Vec::new();
    let mut failed = Vec::new();

    for server_id in server_ids {
        let Some(pos) = servers.iter().position(|s| s.id == server_id) else {
            continue;
        };
        let name = servers[pos].name.clone();

        let source = server_dir(&state, &servers[pos].path);
        if !source.is_dir() {
            failed.push(MoveServersFailure {
                server_id,
                name,
                error: "Server folder was not found".to_string(),
            });
            continue;
        }
        let folder_name = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| servers[pos].path.clone());
        let dest = target.join(&folder_name);

        if source != dest {
            if dest.exists() {
                failed.push(MoveServersFailure {
                    server_id,
                    name,
                    error: format!(
                        "A folder named '{folder_name}' already exists in the target location"
                    ),
                });
                continue;
            }

            let total = count_server_files(&source);
            let bar = init_loading(
                LoadingBarType::ZipExtract {
                    instance_id: String::new(),
                    instance_name: name.clone(),
                },
                total as f64,
                &format!("Moving {}...", name),
            )
            .await?;

            {
                let _permit = state.io_semaphore.0.acquire().await?;
                copy_dir_all(&source, &dest, Some(&bar)).await?;
                tokio::fs::remove_dir_all(&source)
                    .await
                    .map_err(|e| crate::util::io::IOError::with_path(e, &source))?;
            }
            drop(bar);
        }

        let new_path = if into_default {
            folder_name
        } else {
            dest.to_string_lossy().to_string()
        };
        servers[pos].path = new_path.clone();
        moved.push(MoveServersMoved {
            server_id,
            name,
            new_path,
        });
    }

    write_servers_index(&state, &servers).await?;

    Ok(MoveServersReport { moved, failed })
}

#[derive(Serialize)]
pub struct MoveServersMoved {
    pub server_id: String,
    pub name: String,
    pub new_path: String,
}

#[derive(Serialize)]
pub struct MoveServersFailure {
    pub server_id: String,
    pub name: String,
    pub error: String,
}

#[derive(Serialize)]
pub struct MoveServersReport {
    pub moved: Vec<MoveServersMoved>,
    pub failed: Vec<MoveServersFailure>,
}

fn count_server_files(path: &Path) -> u64 {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                count += count_server_files(&entry_path);
            } else {
                count += 1;
            }
        }
    }
    count
}

/// Writes an accepted EULA for the server and updates its index entry, so
/// the Run action becomes available.
pub async fn accept_server_eula(server_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let mut servers = list_servers().await?;
    if let Some(pos) = servers.iter().position(|s| s.id == server_id) {
        let dir = server_dir(&state, &servers[pos].path);
        crate::util::io::create_dir_all(&dir).await?;
        write_text_file(
            &dir.join("eula.txt"),
            &format!(
                "# Accepted via ChocoModrinth server creator on {}\neula=true\n",
                Utc::now().to_rfc3339()
            ),
        )?;
        servers[pos].eula_accepted = true;
        write_servers_index(&state, &servers).await?;
    }
    Ok(())
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
                .rev()
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
                .rev()
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

/// Creates a loading bar for server setup that shows up in the app's
/// download/notification area (bytes when `total > 0`, indeterminate spinner
/// when `total == 0`).
async fn init_server_progress_bar(
    server_name: &str,
    total: u64,
    message: &str,
) -> crate::Result<LoadingBarId> {
    init_loading(
        crate::event::LoadingBarType::ZipExtract {
            instance_id: String::new(),
            instance_name: server_name.to_string(),
        },
        total as f64,
        message,
    )
    .await
}

fn count_files_in_dir(path: &Path) -> u64 {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                count += count_files_in_dir(&entry_path);
            } else {
                count += 1;
            }
        }
    }
    count
}

/// Downloads a file, reporting progress on a loading bar in the app's
/// download/notification area. `progress` is `(server_name, message)`; when
/// the server sends a content length the bar is a byte progress bar,
/// otherwise it shows as an indeterminate spinner.
async fn download_to_file(
    state: &State,
    url: &str,
    sha1: Option<&str>,
    dest: &Path,
    progress: Option<(&str, &str)>,
) -> crate::Result<()> {
    if let Some(parent) = dest.parent() {
        crate::util::io::create_dir_all(parent).await?;
    }

    // Streamed manually so download progress can be reported; fetch_file
    // buffers the whole response before it can be read.
    let _permit = state.api_semaphore.0.acquire().await?;
    let client = reqwest::Client::new();
    let mut response = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| {
            crate::util::io::IOError::with_path(std::io::Error::other(e.to_string()), dest)
        })?;
    let total = response.content_length().unwrap_or(0);

    let bar = match progress {
        Some((server_name, message)) => {
            Some(init_server_progress_bar(server_name, total, message).await?)
        }
        None => None,
    };

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| crate::util::io::IOError::with_path(e, dest))?;
    let mut hasher = sha1.map(|_| sha1_smol::Sha1::new());

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| {
            crate::util::io::IOError::with_path(std::io::Error::other(e.to_string()), dest)
        })?
    {
        if let Some(bar) = &bar {
            if total > 0 {
                let _ = emit_loading(bar, chunk.len() as f64, None);
            }
        }
        if let Some(hasher) = hasher.as_mut() {
            hasher.update(&chunk);
        }
        file.write_all(&chunk)
            .await
            .map_err(|e| crate::util::io::IOError::with_path(e, dest))?;
    }
    file.flush()
        .await
        .map_err(|e| crate::util::io::IOError::with_path(e, dest))?;
    drop(bar);

    if let (Some(expected), Some(hasher)) = (sha1, hasher) {
        let actual = hasher
            .digest()
            .bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual != expected {
            return Err(crate::ErrorKind::InputError(format!(
                "Downloaded {url} has wrong SHA-1 (expected {expected}, got {actual})"
            ))
            .into());
        }
    }

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

/// Resolves the Java major version a game version actually requires, by
/// reading Mojang's version metadata (covers the post-1.x versioning era,
/// e.g. 26.2 requiring Java 25, which the legacy heuristic above misses).
/// Falls back to the heuristic when the metadata cannot be fetched.
async fn resolve_server_java_version(game_version: &str) -> u32 {
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
        #[serde(rename = "javaVersion")]
        java_version: Option<JavaVersion>,
    }
    #[derive(Deserialize)]
    struct JavaVersion {
        #[serde(rename = "majorVersion")]
        major_version: u32,
    }

    let state = match State::get().await {
        Ok(state) => state,
        Err(_) => return required_java_version(game_version),
    };
    let manifest: Result<Manifest, _> = fetch::fetch_json(
        reqwest::Method::GET,
        VANILLA_MANIFEST_URL,
        None,
        None,
        None,
        &state.api_semaphore,
        &state.pool,
    )
    .await;
    let Ok(manifest) = manifest else {
        return required_java_version(game_version);
    };
    let Some(version) = manifest.versions.into_iter().find(|v| v.id == game_version) else {
        return required_java_version(game_version);
    };
    let version_json: Result<VersionJson, _> = fetch::fetch_json(
        reqwest::Method::GET,
        &version.url,
        None,
        None,
        None,
        &state.api_semaphore,
        &state.pool,
    )
    .await;
    match version_json {
        Ok(json) => json
            .java_version
            .map(|j| j.major_version)
            .unwrap_or_else(|| required_java_version(game_version)),
        Err(_) => required_java_version(game_version),
    }
}

async fn download_vanilla_server(
    state: &State,
    game_version: &str,
    dir: &Path,
    progress: Option<(&str, &str)>,
) -> crate::Result<()> {
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
        progress,
    )
    .await
}

async fn download_fabric_quilt_server(
    state: &State,
    is_quilt: bool,
    game_version: &str,
    loader_version: Option<&str>,
    dir: &Path,
    progress: Option<(&str, &str)>,
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
    download_to_file(state, &url, None, &dir.join("server.jar"), progress).await?;
    Ok(format!("{loader_version} (installer {installer})"))
}

async fn download_paper_server(
    state: &State,
    game_version: &str,
    build: Option<&str>,
    dir: &Path,
    progress: Option<(&str, &str)>,
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
        progress,
    )
    .await?;
    Ok(chosen.downloads.application.name)
}

async fn download_purpur_server(
    state: &State,
    game_version: &str,
    build: Option<&str>,
    dir: &Path,
    progress: Option<(&str, &str)>,
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
    download_to_file(state, &url, None, &dir.join(&file_name), progress).await?;
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
    server_name: &str,
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
    download_to_file(state, &url, None, &dir.join(&installer_name), None).await?;
    let bar = init_server_progress_bar(
        server_name,
        0,
        "Running the Forge installer (this can take a minute)...",
    )
    .await?;
    run_installer(dir, java_path, &installer_name).await?;
    drop(bar);
    let _ = std::fs::remove_file(dir.join(&installer_name));
    Ok(forge_version)
}

async fn download_neoforge_server(
    state: &State,
    game_version: &str,
    neoforge_version: Option<&str>,
    java_path: &Path,
    dir: &Path,
    server_name: &str,
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
    download_to_file(state, &url, None, &dir.join(&installer_name), None).await?;
    let bar = init_server_progress_bar(
        server_name,
        0,
        "Running the NeoForge installer (this can take a minute)...",
    )
    .await?;
    run_installer(dir, java_path, &installer_name).await?;
    drop(bar);
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
    // When a custom servers directory is configured, new servers are created
    // there and the stored path is absolute
    let settings = crate::state::Settings::get(&state.pool).await?;
    let custom_root = settings
        .custom_servers_dir
        .as_deref()
        .map(PathBuf::from);
    if let Some(root) = &custom_root {
        crate::util::io::create_dir_all(root).await?;
    }
    let resolve_dir = |path: &str| -> PathBuf {
        match &custom_root {
            Some(root) => root.join(path),
            None => state.directories.servers_dir().join(path),
        }
    };

    let mut path = name.clone();
    let mut dir = resolve_dir(&path);
    let mut suffix = 2;
    while servers.iter().any(|s| s.path == path) || dir.exists() {
        path = format!("{name} ({suffix})");
        dir = resolve_dir(&path);
        suffix += 1;
    }
    if custom_root.is_some() {
        // Paths in the custom directory are stored absolute
        path = dir.to_string_lossy().to_string();
    }

    crate::util::io::create_dir_all(&dir).await?;

    // Resolve a Java runtime for installation and running the server
    let java_path = crate::api::jre::auto_install_java(
        resolve_server_java_version(&opts.game_version).await,
    )
    .await?;

    let (jar_file, resolved_loader_version) = match opts.loader {
        ServerLoader::Vanilla => {
            let message = format!("Downloading Minecraft {} server...", opts.game_version);
            let progress = (opts.name.as_str(), message.as_str());
            download_vanilla_server(&state, &opts.game_version, &dir, Some(progress)).await?;
            ("server.jar".to_string(), None)
        }
        ServerLoader::Fabric => {
            let message = format!("Downloading Fabric {} server...", opts.game_version);
            let progress = (opts.name.as_str(), message.as_str());
            let version =
                download_fabric_quilt_server(&state, false, &opts.game_version, opts.loader_version.as_deref(), &dir, Some(progress))
                    .await?;
            ("server.jar".to_string(), Some(version))
        }
        ServerLoader::Quilt => {
            let message = format!("Downloading Quilt {} server...", opts.game_version);
            let progress = (opts.name.as_str(), message.as_str());
            let version =
                download_fabric_quilt_server(&state, true, &opts.game_version, opts.loader_version.as_deref(), &dir, Some(progress))
                    .await?;
            ("server.jar".to_string(), Some(version))
        }
        ServerLoader::Paper => {
            let message = format!("Downloading Paper {} server...", opts.game_version);
            let progress = (opts.name.as_str(), message.as_str());
            let file = download_paper_server(
                &state,
                &opts.game_version,
                opts.loader_version.as_deref(),
                &dir,
                Some(progress),
            )
            .await?;
            (file, opts.loader_version.clone())
        }
        ServerLoader::Purpur => {
            let message = format!("Downloading Purpur {} server...", opts.game_version);
            let progress = (opts.name.as_str(), message.as_str());
            let file = download_purpur_server(
                &state,
                &opts.game_version,
                opts.loader_version.as_deref(),
                &dir,
                Some(progress),
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
                &opts.name,
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
                &opts.name,
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

    let server_dir = server_dir(&state, &server.path);
    let instance_dir = state
        .directories
        .instances_dir()
        .join(&metadata.instance.path);

    // Copy the selected save as the server world
    if let Some(save) = &save_name {
        let source = instance_dir.join("saves").join(save);
        if source.is_dir() {
            let total = count_files_in_dir(&source);
            let bar = init_server_progress_bar(
                &server.name,
                total,
                "Copying the world save...",
            )
            .await?;
            copy_dir_all(&source, &server_dir.join(save), Some(&bar)).await?;
            drop(bar);
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
    let server_dir = server_dir(&state, &server.path);

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
            let total = count_files_in_dir(&config_src);
            let bar = init_server_progress_bar(
                &server.name,
                total,
                "Copying the config folder...",
            )
            .await?;
            copy_dir_all(&config_src, &config_dest, Some(&bar)).await?;
            drop(bar);
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
            let check_bar = init_server_progress_bar(
                &server.name,
                0,
                "Checking which mods are client-only (via Modrinth)...",
            )
            .await?;
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
            drop(check_bar);

            for (path, project_id) in file_projects {
                if client_only.contains(&project_id) {
                    report.mods_skipped_client_only += 1;
                } else {
                    allowed_files.push(path);
                }
            }

            let copy_bar = init_server_progress_bar(
                &server.name,
                allowed_files.len() as u64,
                "Copying mods...",
            )
            .await?;
            for file in allowed_files {
                let file_name = file
                    .file_name()
                    .map(|n| n.to_os_string())
                    .unwrap_or_default();
                let dest = mods_dest.join(&file_name);
                tokio::fs::copy(&file, &dest)
                    .await
                    .map_err(|e| crate::util::io::IOError::with_path(e, &file))?;
                let _ = emit_loading(&copy_bar, 1.0, None);
                report.mods_copied += 1;
            }
            drop(copy_bar);
        }
    }

    Ok(report)
}

/// Recursively copies a directory, overwriting existing files. Each copied
/// file ticks the progress bar (when one is given).
pub fn copy_dir_all<'a>(
    src: &'a Path,
    dest: &'a Path,
    bar: Option<&'a LoadingBarId>,
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
                copy_dir_all(&source, &target, bar).await?;
            } else {
                tokio::fs::copy(&source, &target)
                    .await
                    .map_err(|e| crate::util::io::IOError::with_path(e, &source))?;
                if let Some(bar) = bar {
                    let _ = emit_loading(bar, 1.0, None);
                }
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
// region: server dashboard (ping, properties, content)

fn content_folder_name(loader: ServerLoader) -> &'static str {
    match loader {
        ServerLoader::Paper | ServerLoader::Purpur => "plugins",
        _ => "mods",
    }
}

async fn get_server_by_id(state: &State, server_id: &str) -> crate::Result<ChocoServer> {
    let server = list_servers()
        .await?
        .into_iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!("Server {server_id} was not found"))
        })?;
    Ok(server)
}

#[derive(Serialize, Debug, Clone)]
pub struct ServerPingResult {
    pub motd: Option<String>,
    pub players_online: i32,
    pub players_max: i32,
    pub players: Vec<String>,
    pub favicon: Option<String>,
    pub version: Option<String>,
}

/// Pings the local server's status endpoint (players online, MOTD, icon).
pub async fn ping_server(port: u16) -> crate::Result<ServerPingResult> {
    let status = crate::util::server_ping::get_server_status(
        &("127.0.0.1", port),
        ("127.0.0.1", port),
        None,
    )
    .await?;

    fn description_text(
        raw: &Option<Box<serde_json::value::RawValue>>,
    ) -> Option<String> {
        let raw = raw.as_ref()?;
        let value = serde_json::from_str::<serde_json::Value>(raw.get()).ok()?;
        if let Some(text) = value.as_str() {
            return Some(text.to_string());
        }
        // chat component: {"text": ...} possibly with extras
        if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
            let mut out = text.to_string();
            if let Some(extras) = value.get("extra").and_then(|v| v.as_array()) {
                for extra in extras {
                    if let Some(part) = extra.get("text").and_then(|v| v.as_str()) {
                        out.push_str(part);
                    }
                }
            }
            return Some(out);
        }
        None
    }

    let players = status
        .players
        .as_ref()
        .map(|p| p.sample.iter().map(|s| s.name.clone()).collect())
        .unwrap_or_default();

    Ok(ServerPingResult {
        motd: description_text(&status.description),
        players_online: status.players.as_ref().map(|p| p.online).unwrap_or(0),
        players_max: status.players.as_ref().map(|p| p.max).unwrap_or(0),
        players,
        favicon: status.favicon.as_ref().map(|url| url.to_string()),
        version: status.version.as_ref().map(|v| v.name.clone()),
    })
}

/// Reads server.properties as an ordered list of key-value pairs.
pub async fn get_server_properties(
    server_id: &str,
) -> crate::Result<Vec<(String, String)>> {
    let state = State::get().await?;
    let server = get_server_by_id(&state, server_id).await?;
    let path = server_dir(&state, &server.path).join("server.properties");
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return Ok(Vec::new()),
    };
    let mut props = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            props.push((key.to_string(), value.to_string()));
        }
    }
    Ok(props)
}

/// Writes server.properties from an ordered list of key-value pairs.
pub async fn set_server_properties(
    server_id: &str,
    props: Vec<(String, String)>,
) -> crate::Result<()> {
    let state = State::get().await?;
    let server = get_server_by_id(&state, server_id).await?;
    let path = server_dir(&state, &server.path).join("server.properties");
    let mut content = String::from("# server.properties (edited in ChocoModrinth)\n");
    for (key, value) in props {
        content.push_str(&format!("{key}={value}\n"));
    }
    std::fs::write(&path, content)
        .map_err(|e| crate::util::io::IOError::with_path(e, &path))?;
    Ok(())
}

#[derive(Serialize, Debug)]
pub struct ServerContentItem {
    pub file_name: String,
    pub title: Option<String>,
    pub version: Option<String>,
    pub icon_url: Option<String>,
    pub size: u64,
    pub enabled: bool,
}

fn content_folder(state: &State, server: &ChocoServer) -> crate::Result<PathBuf> {
    Ok(server_dir(state, &server.path).join(content_folder_name(server.loader)))
}

/// Extracts mod name/version/icon from jar metadata (fabric/quilt json, forge
/// toml). Returns defaults when nothing readable is found.
fn read_jar_metadata(path: &Path) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(file) = std::fs::File::open(path) else {
        return (None, None, None);
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(_) => return (None, None, None),
    };

    fn read_entry(
        archive: &mut zip::ZipArchive<std::fs::File>,
        name: &str,
    ) -> Option<Vec<u8>> {
        let mut entry = archive.by_name(name).ok()?;
        use std::io::Read;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).ok()?;
        Some(buf)
    }

    // fabric.mod.json / quilt.mod.json: name, version, icon path
    for meta_name in ["fabric.mod.json", "quilt.mod.json"] {
        if let Some(bytes) = read_entry(&mut archive, meta_name) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let (title, version, icon_path) = if meta_name == "fabric.mod.json" {
                    (
                        value.get("name").and_then(|v| v.as_str()),
                        value.get("version").and_then(|v| v.as_str()),
                        value.get("icon").and_then(|v| v.as_str()),
                    )
                } else {
                    let loader = value.get("quilt_loader");
                    (
                        loader
                            .and_then(|l| l.get("metadata"))
                            .and_then(|m| m.get("name"))
                            .and_then(|v| v.as_str()),
                        loader
                            .and_then(|l| l.get("version"))
                            .and_then(|v| v.as_str()),
                        loader
                            .and_then(|l| l.get("metadata"))
                            .and_then(|m| m.get("icon"))
                            .and_then(|v| v.as_str()),
                    )
                };
                let icon_url = icon_path.and_then(|icon_path| {
                    read_entry(&mut archive, icon_path).map(|bytes| {
                        format!(
                            "data:image/png;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(bytes)
                        )
                    })
                });
                return (
                    title.map(|s| s.to_string()),
                    version.map(|s| s.to_string()),
                    icon_url,
                );
            }
        }
    }

    // forge: META-INF/mods.toml — grab the first displayName/version
    if let Some(bytes) = read_entry(&mut archive, "META-INF/mods.toml") {
        if let Ok(text) = String::from_utf8(bytes) {
            let mut title = None;
            let mut version = None;
            for line in text.lines() {
                let line = line.trim();
                if title.is_none()
                    && let Some(rest) = line.strip_prefix("displayName=")
                {
                    title = Some(rest.trim_matches('"').to_string());
                }
                if version.is_none()
                    && let Some(rest) = line.strip_prefix("version=")
                {
                    let rest = rest.trim_matches('"');
                    // forge uses ranges like "${file.jarVersion}" — only literals
                    if !rest.starts_with("${") {
                        version = Some(rest.to_string());
                    }
                }
                if title.is_some() && version.is_some() {
                    break;
                }
            }
            if title.is_some() || version.is_some() {
                return (title, version, None);
            }
        }
    }

    (None, None, None)
}

/// Lists the mods/plugins installed on a server.
pub async fn list_server_content(
    server_id: &str,
) -> crate::Result<Vec<ServerContentItem>> {
    let state = State::get().await?;
    let server = get_server_by_id(&state, server_id).await?;
    let folder = content_folder(&state, &server)?;
    if !folder.is_dir() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    let entries = std::fs::read_dir(&folder)
        .map_err(|e| crate::util::io::IOError::with_path(e, &folder))?;
    for entry in entries {
        let entry = entry.map_err(|e| crate::util::io::IOError::with_path(e, &folder))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let raw_name = entry.file_name().to_string_lossy().to_string();
        let enabled = !raw_name.ends_with(".disabled");
        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
        let (title, version, icon_url) = if enabled {
            read_jar_metadata(&path)
        } else {
            (None, None, None)
        };
        items.push(ServerContentItem {
            file_name: raw_name,
            title,
            version,
            icon_url,
            size,
            enabled,
        });
    }
    items.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(items)
}

/// Guards against path traversal: the file must live directly in the
/// content folder.
fn checked_content_path(
    state: &State,
    server: &ChocoServer,
    file_name: &str,
) -> crate::Result<PathBuf> {
    if file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") {
        return Err(
            crate::ErrorKind::InputError("Invalid file name".to_string()).into(),
        );
    }
    Ok(content_folder(state, server)?.join(file_name))
}

/// Enables or disables a mod/plugin by renaming it with a `.disabled` suffix.
pub async fn set_server_content_enabled(
    server_id: &str,
    file_name: &str,
    enabled: bool,
) -> crate::Result<()> {
    let state = State::get().await?;
    let server = get_server_by_id(&state, server_id).await?;
    let path = checked_content_path(&state, &server, file_name)?;
    let new_name = if enabled {
        file_name
            .strip_suffix(".disabled")
            .unwrap_or(file_name)
            .to_string()
    } else {
        format!("{file_name}.disabled")
    };
    let dest = path.with_file_name(new_name);
    std::fs::rename(&path, &dest)
        .map_err(|e| crate::util::io::IOError::with_path(e, &path))?;
    Ok(())
}

/// Permanently deletes a mod/plugin file.
pub async fn delete_server_content(
    server_id: &str,
    file_name: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let server = get_server_by_id(&state, server_id).await?;
    let path = checked_content_path(&state, &server, file_name)?;
    std::fs::remove_file(&path)
        .map_err(|e| crate::util::io::IOError::with_path(e, &path))?;
    Ok(())
}

// endregion
