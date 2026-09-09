use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_opener::OpenerExt;
use theseus::servers::{
    server_dir, ChocoServer, CreateServerOptions, MoveServersReport, ServerLoader,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

use super::Result;

fn server_error(message: &str) -> super::TheseusSerializableError {
    theseus::ErrorKind::OtherError(message.to_string())
        .as_error()
        .into()
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("servers")
        .invoke_handler(tauri::generate_handler![
            servers_list,
            servers_create,
            servers_create_from_profile,
            servers_sync_profile_content,
            servers_delete,
            servers_minecraft_versions,
            servers_loader_versions,
            servers_run,
            servers_stop,
            servers_is_running,
            servers_open_folder,
            servers_move,
            servers_default_dir,
            servers_accept_eula,
        ])
        .build()
}

#[derive(Default)]
pub struct ServerProcessManager {
    processes: Mutex<HashMap<String, Arc<Mutex<ServerProcess>>>>,
}

struct ServerProcess {
    child: Child,
    stdin: Option<ChildStdin>,
}

#[derive(Serialize, Clone)]
struct ServerStatusPayload<'a> {
    server_id: &'a str,
    running: bool,
}

#[derive(Serialize, Clone)]
struct ServerConsolePayload<'a> {
    server_id: &'a str,
    line: String,
}

#[tauri::command]
pub async fn servers_list() -> Result<Vec<ChocoServer>> {
    Ok(theseus::servers::list_servers().await?)
}

#[tauri::command]
pub async fn servers_create(options: CreateServerOptions) -> Result<ChocoServer> {
    Ok(theseus::servers::create_server(options).await?)
}

#[tauri::command]
pub async fn servers_delete(server_id: String) -> Result<()> {
    theseus::servers::delete_server(&server_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn servers_minecraft_versions() -> Result<Vec<String>> {
    Ok(theseus::servers::minecraft_server_versions().await?)
}

#[tauri::command]
pub async fn servers_loader_versions(
    loader: ServerLoader,
    game_version: String,
) -> Result<Vec<theseus::servers::ServerLoaderVersion>> {
    Ok(theseus::servers::server_loader_versions(loader, &game_version).await?)
}

#[tauri::command]
pub async fn servers_is_running(
    server_id: String,
    manager: State<'_, ServerProcessManager>,
) -> Result<bool> {
    Ok(manager.processes.lock().await.contains_key(&server_id))
}

#[tauri::command]
pub async fn servers_accept_eula(server_id: String) -> Result<()> {
    theseus::servers::accept_server_eula(&server_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn servers_move(
    server_ids: Vec<String>,
    target_dir: String,
    manager: State<'_, ServerProcessManager>,
) -> Result<MoveServersReport> {
    {
        let processes = manager.processes.lock().await;
        if server_ids.iter().any(|id| processes.contains_key(id)) {
            return Err(server_error(
                "A selected server is still running; stop it before moving it",
            ));
        }
    }
    Ok(theseus::servers::move_servers_to_dir(server_ids, target_dir).await?)
}

#[tauri::command]
pub async fn servers_default_dir() -> Result<String> {
    Ok(theseus::servers::default_servers_dir().await?)
}

#[tauri::command]
pub async fn servers_create_from_profile(
    instance_id: String,
    save_name: Option<String>,
    copy_mods: bool,
    copy_config: bool,
    accept_eula: bool,
    ram_mb: u32,
    port: u16,
) -> Result<ChocoServer> {
    Ok(theseus::servers::create_server_from_profile(
        instance_id,
        save_name,
        copy_mods,
        copy_config,
        accept_eula,
        ram_mb,
        port,
    )
    .await?)
}

#[tauri::command]
pub async fn servers_sync_profile_content(server_id: String) -> Result<()> {
    theseus::servers::sync_profile_server(server_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn servers_open_folder<R: Runtime>(
    app: AppHandle<R>,
    server_id: String,
) -> Result<()> {
    let state = theseus::State::get().await?;
    let servers = theseus::servers::list_servers().await?;
    let server = servers
        .into_iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| server_error("Server not found"))?;
    let dir = server_dir(&state, &server.path);
    if let Err(e) = app.opener().open_path(dir.to_string_lossy(), None::<&str>) {
        return Err(server_error(&format!("Failed to open server folder: {e}")));
    }
    Ok(())
}

async fn spawn_args(
    server: &ChocoServer,
) -> Result<(std::path::PathBuf, String, Vec<String>)> {
    let state = theseus::State::get().await?;
    let dir = server_dir(&state, &server.path);

    let java = server.java_path.clone().unwrap_or_else(|| "java".to_string());

    Ok(match server.loader {
        ServerLoader::Forge | ServerLoader::NeoForge => {
            #[cfg(windows)]
            {
                (dir, "cmd".to_string(), vec!["/c".to_string(), "run.bat".to_string()])
            }
            #[cfg(not(windows))]
            {
                (dir, "./run.sh".to_string(), Vec::new())
            }
        }
        _ => {
            let jar = server
                .jar_file
                .clone()
                .unwrap_or_else(|| "server.jar".to_string());
            (
                dir,
                java,
                vec![
                    format!("-Xmx{}M", server.ram_mb),
                    format!("-Xms{}M", server.ram_mb),
                    "-jar".to_string(),
                    jar,
                    "nogui".to_string(),
                ],
            )
        }
    })
}

#[tauri::command]
pub async fn servers_run<R: Runtime>(
    app: AppHandle<R>,
    server_id: String,
    manager: State<'_, ServerProcessManager>,
) -> Result<()> {
    {
        let processes = manager.processes.lock().await;
        if processes.contains_key(&server_id) {
            return Err(server_error("Server is already running"));
        }
    }

    let servers = theseus::servers::list_servers().await?;
    let server = servers
        .into_iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| server_error("Server not found"))?;

    let (dir, program, args) = spawn_args(&server).await?;

    let mut child = Command::new(&program)
        .args(&args)
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            server_error(&format!(
                "Failed to start server '{}': {e}. Is Java installed?",
                server.name
            ))
        })?;

    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let process = Arc::new(Mutex::new(ServerProcess { child, stdin }));
    manager
        .processes
        .lock()
        .await
        .insert(server_id.clone(), process.clone());

    let _ = app.emit(
        "server-status",
        ServerStatusPayload {
            server_id: &server_id,
            running: true,
        },
    );

    // Stream console output (stdout + stderr share one reader loop each)
    let streams: Vec<Box<dyn tokio::io::AsyncRead + Unpin + Send>> = [
        stdout.map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        stderr.map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
    ]
    .into_iter()
    .flatten()
    .collect();

    for stream in streams {
        let app = app.clone();
        let server_id = server_id.clone();
        tauri::async_runtime::spawn(async move {
            let reader = BufReader::new(stream);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app.emit(
                    "server-console",
                    ServerConsolePayload {
                        server_id: &server_id,
                        line,
                    },
                );
            }
        });
    }

    // Wait for process exit in the background: remove from manager + emit status.
    // Poll with short locks — holding the mutex across wait() would deadlock
    // servers_stop, which needs it to write the stop command.
    {
        let app = app.clone();
        let server_id = server_id.clone();
        let manager = app.state::<ServerProcessManager>();
        let process = manager
            .processes
            .lock()
            .await
            .get(&server_id)
            .cloned();
        if let Some(process) = process {
            tauri::async_runtime::spawn(async move {
                let app = app.clone();
                let server_id = server_id.clone();
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    let exited = {
                        let mut guard = process.lock().await;
                        guard.child.try_wait().map(|e| e.is_some()).unwrap_or(true)
                    };
                    if exited {
                        break;
                    }
                }
                let manager = app.state::<ServerProcessManager>();
                {
                    let mut processes = manager.processes.lock().await;
                    processes.remove(&server_id);
                }
                let _ = app.emit(
                    "server-status",
                    ServerStatusPayload {
                        server_id: &server_id,
                        running: false,
                    },
                );
            });
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn servers_stop(
    server_id: String,
    manager: State<'_, ServerProcessManager>,
) -> Result<()> {
    let process = {
        let processes = manager.processes.lock().await;
        processes.get(&server_id).cloned()
    };

    let process = process
        .ok_or_else(|| server_error("Server is not running"))?;

    {
        let mut guard = process.lock().await;
        if let Some(stdin) = guard.stdin.as_mut() {
            // Graceful shutdown: send the stop command to the server console
            let _ = stdin.write_all(b"stop\n").await;
            let _ = stdin.flush().await;
        }
    }

    // Some servers ignore stdin stop (e.g. 26.x autopause suspends command
    // processing) — fall back to killing the process after a grace period.
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let mut guard = process.lock().await;
        if guard.child.try_wait().map_err(|e| {
            server_error(&format!("Failed to check server status: {e}"))
        })?.is_some() {
            return Ok(());
        }
    }

    let mut guard = process.lock().await;
    if let Err(e) = guard.child.start_kill() {
        tracing::warn!("Failed to kill server process: {e}");
    }

    Ok(())
}
