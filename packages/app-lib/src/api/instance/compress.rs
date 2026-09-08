use crate::event::emit::emit_instance;
use crate::event::InstancePayloadType;
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{Instance, State};
use crate::util::io::IOError;
use sevenz_rust2::{
    ArchiveWriter, EncoderConfiguration, EncoderMethod, encoder_options::Lzma2Options,
};
use std::path::{Path, PathBuf};

/// Name of the 7z archive stored inside an instance folder when it is compressed.
pub const ARCHIVE_FILE_NAME: &str = "choco-profile.7z";

/// Screenshots are kept outside the archive so they remain viewable while an
/// instance is compressed.
const EXCLUDED_DIR_NAME: &str = "screenshots";

fn is_excluded_from_archive(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == Path::new(ARCHIVE_FILE_NAME).file_name().unwrap())
        || path
            .components()
            .any(|component| component.as_os_str() == EXCLUDED_DIR_NAME)
}

fn resolve_instance_dir(state: &State, instance: &Instance) -> crate::Result<PathBuf> {
    let dir = state
        .directories
        .instances_dir()
        .join(&instance.path);
    Ok(crate::util::io::canonicalize(dir)?)
}

async fn get_running_instance_error(instance_id: &str) -> crate::Result<()> {
    let processes = crate::api::process::get_by_instance_id(instance_id).await?;
    if processes.is_empty() {
        Ok(())
    } else {
        Err(crate::ErrorKind::InputError(
            "Instance is currently running; stop it before changing compression".to_string(),
        )
        .into())
    }
}

fn create_archive(instance_dir: &Path, archive_path: &Path) -> crate::Result<()> {
    let lzma2 = Lzma2Options::from_level(9);
    let config = EncoderConfiguration::new(EncoderMethod::LZMA2).with_options(lzma2.into());

    let mut writer = ArchiveWriter::create(archive_path)
        .map_err(|e| IOError::with_path(std::io::Error::other(e.to_string()), archive_path))?;
    writer.set_content_methods(vec![config]);

    writer
        .push_source_path(instance_dir, |path| !is_excluded_from_archive(path))
        .map_err(|e| IOError::with_path(std::io::Error::other(e.to_string()), instance_dir))?;

    writer
        .finish()
        .map_err(|e| IOError::with_path(std::io::Error::other(e.to_string()), archive_path))?;

    if !archive_path.exists() || archive_path.metadata().map_err(IOError::from)?.len() == 0 {
        return Err(crate::ErrorKind::OtherError(format!(
            "Failed to write archive {}",
            archive_path.display()
        ))
        .into());
    }

    Ok(())
}

fn extract_archive(archive_path: &Path, instance_dir: &Path) -> crate::Result<()> {
    sevenz_rust2::decompress_file(archive_path, instance_dir).map_err(|e| {
        IOError::with_path(
            std::io::Error::other(e.to_string()),
            archive_path,
        )
    })?;
    std::fs::remove_file(archive_path)
        .map_err(|e| IOError::with_path(e, archive_path))?;
    Ok(())
}

/// Removes everything from the instance folder except the archive and the
/// screenshots directory. Only runs when a valid archive is present.
fn prune_extracted_files(instance_dir: &Path, archive_path: &Path) -> crate::Result<()> {
    let entries = std::fs::read_dir(instance_dir)
        .map_err(|e| IOError::with_path(e, instance_dir))?;

    for entry in entries {
        let entry = entry.map_err(|e| IOError::with_path(e, instance_dir))?;
        let path = entry.path();
        if path == archive_path {
            continue;
        }
        if path.file_name().is_some_and(|name| name == EXCLUDED_DIR_NAME) {
            continue;
        }
        if path.is_dir() {
            std::fs::remove_dir_all(&path)
                .map_err(|e| IOError::with_path(e, &path))?;
        } else {
            std::fs::remove_file(&path).map_err(|e| IOError::with_path(e, &path))?;
        }
    }

    Ok(())
}

fn archive_path_for(instance_dir: &Path) -> PathBuf {
    instance_dir.join(ARCHIVE_FILE_NAME)
}

pub async fn compress(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    get_running_instance_error(instance_id).await?;

    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Tried to compress nonexistent instance {instance_id}"
            ))
        })?;
    if metadata.instance.compressed {
        return Ok(());
    }

    let instance_dir = resolve_instance_dir(&state, &metadata.instance)?;
    let archive_path = archive_path_for(&instance_dir);

    let permit = state.io_semaphore.0.acquire().await?;
    let dir = instance_dir.clone();
    tokio::task::spawn_blocking(move || {
        create_archive(&dir, &archive_path)?;
        prune_extracted_files(&dir, &archive_path)
    })
    .await
    .map_err(|e| crate::ErrorKind::OtherError(e.to_string()))??;
    drop(permit);

    instance_rows::set_instance_compressed(instance_id, true, &state.pool).await?;
    emit_instance(instance_id, InstancePayloadType::Edited).await?;

    Ok(())
}

pub async fn decompress(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    get_running_instance_error(instance_id).await?;

    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Tried to decompress nonexistent instance {instance_id}"
            ))
        })?;

    let instance_dir = resolve_instance_dir(&state, &metadata.instance)?;
    let archive_path = archive_path_for(&instance_dir);

    if archive_path.exists() {
        let permit = state.io_semaphore.0.acquire().await?;
        let dir = instance_dir.clone();
        tokio::task::spawn_blocking(move || extract_archive(&archive_path, &dir))
            .await
            .map_err(|e| crate::ErrorKind::OtherError(e.to_string()))??;
        drop(permit);
    }

    instance_rows::set_instance_compressed(instance_id, false, &state.pool).await?;
    emit_instance(instance_id, InstancePayloadType::Edited).await?;

    Ok(())
}

/// Called before launching: extracts the archive if the instance is stored
/// compressed. The `compressed` flag stays set so the instance is compressed
/// again once the game exits.
pub async fn ensure_extracted_for_launch(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;

    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Tried to launch nonexistent instance {instance_id}"
            ))
        })?;
    if !metadata.instance.compressed {
        return Ok(());
    }

    let instance_dir = resolve_instance_dir(&state, &metadata.instance)?;
    let archive_path = archive_path_for(&instance_dir);
    if !archive_path.exists() {
        // Already extracted (e.g. the launcher closed mid-session); it will be
        // recompressed when the game exits.
        return Ok(());
    }

    let permit = state.io_semaphore.0.acquire().await?;
    let dir = instance_dir.clone();
    tokio::task::spawn_blocking(move || extract_archive(&archive_path, &dir))
        .await
        .map_err(|e| crate::ErrorKind::OtherError(e.to_string()))??;
    drop(permit);

    Ok(())
}

/// Called after the game exits: re-creates the archive for instances flagged
/// as compressed.
pub async fn recompress_after_exit(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;

    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Tried to recompress nonexistent instance {instance_id}"
            ))
        })?;
    if !metadata.instance.compressed {
        return Ok(());
    }

    let instance_dir = resolve_instance_dir(&state, &metadata.instance)?;
    let archive_path = archive_path_for(&instance_dir);
    if archive_path.exists() {
        return Ok(());
    }

    let permit = state.io_semaphore.0.acquire().await?;
    let dir = instance_dir.clone();
    tokio::task::spawn_blocking(move || {
        create_archive(&dir, &archive_path)?;
        prune_extracted_files(&dir, &archive_path)
    })
    .await
    .map_err(|e| crate::ErrorKind::OtherError(e.to_string()))??;
    drop(permit);

    Ok(())
}
