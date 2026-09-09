//! Moving profiles (instances) between the default profiles directory and a
//! user-chosen folder. Moved instances store an absolute path so they can be
//! spread across multiple roots; the default root stays relative so nothing
//! breaks if the whole app data folder is relocated later.

use crate::event::emit::{emit_instance, init_loading};
use crate::event::{InstancePayloadType, LoadingBarType};
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{Instance, State};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct RelocatedInstance {
    pub instance_id: String,
    pub name: String,
    pub new_path: String,
}

#[derive(Debug, Serialize)]
pub struct MoveFailure {
    pub instance_id: String,
    pub name: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct MoveProfilesReport {
    pub moved: Vec<RelocatedInstance>,
    pub failed: Vec<MoveFailure>,
}

/// The default profiles directory (used as a move target for "reset").
pub async fn default_profiles_dir() -> crate::Result<String> {
    let state = State::get().await?;
    Ok(state
        .directories
        .instances_dir()
        .to_string_lossy()
        .to_string())
}

/// Moves the given instances into `target_dir` (creating it if needed),
/// updating each instance's stored path afterwards. Cross-drive moves are
/// supported (copy + delete), with a progress bar per instance.
pub async fn move_instances_to_dir(
    instance_ids: Vec<String>,
    target_dir: String,
) -> crate::Result<MoveProfilesReport> {
    let state = State::get().await?;

    let target = PathBuf::from(&target_dir);
    if !target.exists() {
        std::fs::create_dir_all(&target).map_err(|e| {
            crate::util::io::IOError::with_path(e, &target)
        })?;
    }
    let target = crate::util::io::canonicalize(target)?;
    let default_dir =
        crate::util::io::canonicalize(state.directories.instances_dir().clone())?;
    // When moving back into the default directory, store relative paths again
    let into_default = target == default_dir;

    let mut report = MoveProfilesReport {
        moved: Vec::new(),
        failed: Vec::new(),
    };

    for instance_id in instance_ids {
        let metadata = crate::state::get_instance(&instance_id, &state.pool).await?;
        let Some(metadata) = metadata else {
            continue;
        };
        let mut instance = metadata.instance;
        let name = instance.name.clone();

        if let Err(error) = move_single(&state, &mut instance, &target, into_default).await {
            report.failed.push(MoveFailure {
                instance_id: instance_id.clone(),
                name,
                error: error.to_string(),
            });
        } else {
            emit_instance(&instance_id, InstancePayloadType::Edited).await?;
            report.moved.push(RelocatedInstance {
                instance_id,
                name,
                new_path: instance.path,
            });
        }
    }

    Ok(report)
}

async fn move_single(
    state: &State,
    instance: &mut Instance,
    target: &Path,
    into_default: bool,
) -> crate::Result<()> {
    let processes = crate::api::process::get_by_instance_id(&instance.id).await?;
    if !processes.is_empty() {
        return Err(crate::ErrorKind::InputError(format!(
            "'{}' is currently running; stop it before moving it",
            instance.name
        ))
        .into());
    }
    if instance.compressed {
        return Err(crate::ErrorKind::InputError(format!(
            "'{}' is compressed; decompress it before moving it",
            instance.name
        ))
        .into());
    }

    let source = crate::util::io::canonicalize(
        state.directories.instances_dir().join(&instance.path),
    )?;
    let Some(folder_name) = source.file_name() else {
        return Err(crate::ErrorKind::InputError(format!(
            "Cannot determine folder name for '{}'",
            instance.name
        ))
        .into());
    };
    let dest = target.join(folder_name);

    if source == dest {
        // Already in the target directory; just normalize the stored path
        let new_path = if into_default {
            folder_name.to_string_lossy().to_string()
        } else {
            dest.to_string_lossy().to_string()
        };
        instance_rows::set_instance_path(&instance.id, &new_path, &state.pool).await?;
        instance.path = new_path;
        return Ok(());
    }
    if dest.exists() {
        return Err(crate::ErrorKind::InputError(format!(
            "A folder named '{}' already exists in the target location",
            folder_name.to_string_lossy()
        ))
        .into());
    }

    let total = count_files(&source);
    let bar = init_loading(
        LoadingBarType::ZipExtract {
            instance_id: instance.id.clone(),
            instance_name: instance.name.clone(),
        },
        total as f64,
        &format!("Moving {}...", instance.name),
    )
    .await?;

    {
        let _permit = state.io_semaphore.0.acquire().await?;
        crate::api::servers::copy_dir_all(&source, &dest, Some(&bar)).await?;
        crate::util::io::remove_dir_all(&source).await?;
    }
    drop(bar);

    let new_path = if into_default {
        folder_name.to_string_lossy().to_string()
    } else {
        dest.to_string_lossy().to_string()
    };
    instance_rows::set_instance_path(&instance.id, &new_path, &state.pool).await?;
    instance.path = new_path;

    crate::state::instances::watcher::watch_instance_folder(
        &instance.id,
        &instance.path,
        &state.file_watcher,
        &state.directories,
    )
    .await;

    Ok(())
}

fn count_files(path: &Path) -> u64 {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                count += count_files(&entry_path);
            } else {
                count += 1;
            }
        }
    }
    count
}
