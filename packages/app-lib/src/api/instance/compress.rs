use crate::event::emit::{emit_instance, emit_loading, init_loading};
use crate::event::{InstancePayloadType, LoadingBarId, LoadingBarType};
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{Instance, State};
use crate::util::io::IOError;
use sevenz_rust2::{
    ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, SourceReader,
    encoder_options::Lzma2Options,
};
use std::fs::File;
use std::io::Read;
use std::mem::take;
use std::path::{Path, PathBuf};

/// Name of the 7z archive stored inside an instance folder when it is compressed.
pub const ARCHIVE_FILE_NAME: &str = "choco-profile.7z";

/// Screenshots are kept outside the archive so they remain viewable while an
/// instance is compressed.
const EXCLUDED_DIR_NAME: &str = "screenshots";

/// Solid blocks larger than this are split, mirroring sevenz-rust2's own
/// batching limit for solid archives.
const MAX_BLOCK_SIZE: u64 = 4 * 1024 * 1024 * 1024;

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

fn io_err(e: std::io::Error, path: &Path) -> IOError {
    IOError::with_path(e, path)
}

/// Recursively collects the files that belong in the archive (excluding the
/// archive itself and the screenshots directory).
fn collect_archive_files(instance_dir: &Path) -> crate::Result<Vec<PathBuf>> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> crate::Result<()> {
        let entries =
            std::fs::read_dir(dir).map_err(|e| io_err(e, dir))?;
        for entry in entries {
            let entry = entry.map_err(|e| io_err(e, dir))?;
            let path = entry.path();
            if is_excluded_from_archive(&path) {
                continue;
            }
            if path.is_dir() {
                walk(&path, out)?;
            } else if path.is_file() {
                out.push(path);
            }
        }
        Ok(())
    }

    let mut out = Vec::new();
    walk(instance_dir, &mut out)?;
    Ok(out)
}

fn total_size(paths: &[PathBuf]) -> u64 {
    paths
        .iter()
        .filter_map(|path| path.metadata().ok())
        .map(|metadata| metadata.len())
        .sum()
}

/// Counts bytes as they are pulled into the encoder and reports them as
/// loading-bar progress, so solid compression shows real input progress.
struct ProgressReader<'a> {
    inner: File,
    bar: &'a LoadingBarId,
}

impl Read for ProgressReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            let _ = emit_loading(self.bar, n as f64, None);
        }
        Ok(n)
    }
}

/// Same, but for the archive's own bytes while extracting: block decoding
/// reads the source sequentially, so consumed archive bytes are a good
/// extraction progress measure.
struct ProgressingSourceReader<'a> {
    inner: File,
    bar: &'a LoadingBarId,
}

impl Read for ProgressingSourceReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            let _ = emit_loading(self.bar, n as f64, None);
        }
        Ok(n)
    }
}

impl std::io::Seek for ProgressingSourceReader<'_> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

fn create_archive(
    instance_dir: &Path,
    archive_path: &Path,
    bar: &LoadingBarId,
) -> crate::Result<()> {
    let lzma2 = Lzma2Options::from_level(9);
    let config = EncoderConfiguration::new(EncoderMethod::LZMA2).with_options(lzma2.into());

    let mut writer = ArchiveWriter::create(archive_path)
        .map_err(|e| IOError::with_path(std::io::Error::other(e.to_string()), archive_path))?;
    writer.set_content_methods(vec![config]);

    // Push files in solid batches (same as push_source_path's solid mode),
    // but wrap each file so progress is emitted while it is read.
    let paths = collect_archive_files(instance_dir)?;
    let mut entries: Vec<ArchiveEntry> = Vec::new();
    let mut readers: Vec<SourceReader<ProgressReader>> = Vec::new();
    let mut batch_size = 0u64;

    for path in paths {
        let size = path.metadata().map_err(|e| io_err(e, &path))?.len();
        let name = path
            .strip_prefix(instance_dir)
            .map_err(|e| IOError::with_path(std::io::Error::other(e.to_string()), &path))?
            .to_string_lossy()
            .to_string();

        if size >= MAX_BLOCK_SIZE {
            let file = File::open(&path).map_err(|e| io_err(e, &path))?;
            let progress = ProgressReader { inner: file, bar };
            writer
                .push_archive_entry(ArchiveEntry::from_path(&path, name), Some(progress))
                .map_err(|e| {
                    IOError::with_path(std::io::Error::other(e.to_string()), &path)
                })?;
            continue;
        }

        if batch_size + size >= MAX_BLOCK_SIZE {
            writer
                .push_archive_entries(take(&mut entries), take(&mut readers))
                .map_err(|e| {
                    IOError::with_path(std::io::Error::other(e.to_string()), instance_dir)
                })?;
            batch_size = 0;
        }
        batch_size += size;
        let file = File::open(&path).map_err(|e| io_err(e, &path))?;
        entries.push(ArchiveEntry::from_path(&path, name));
        readers.push(SourceReader::new(ProgressReader { inner: file, bar }));
    }

    if !entries.is_empty() {
        writer
            .push_archive_entries(entries, readers)
            .map_err(|e| {
                IOError::with_path(std::io::Error::other(e.to_string()), instance_dir)
            })?;
    }

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

fn extract_archive(
    archive_path: &Path,
    instance_dir: &Path,
    bar: &LoadingBarId,
) -> crate::Result<()> {
    let file = File::open(archive_path).map_err(|e| io_err(e, archive_path))?;
    let source = ProgressingSourceReader { inner: file, bar };

    let extract_entry = |entry: &ArchiveEntry,
                         reader: &mut dyn Read,
                         dest_path: &PathBuf|
     -> Result<bool, sevenz_rust2::Error> {
        if entry.is_anti_item {
            return Ok(true);
        }
        if entry.is_directory {
            std::fs::create_dir_all(dest_path)
                .map_err(|e| sevenz_rust2::Error::Io(e, format!("create dir {}", entry.name()).into()))?;
            return Ok(true);
        }
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| sevenz_rust2::Error::Io(e, format!("create dir {}", entry.name()).into()))?;
        }
        let mut out = File::create(dest_path)
            .map_err(|e| sevenz_rust2::Error::Io(e, format!("create {}", entry.name()).into()))?;
        std::io::copy(reader, &mut out)
            .map_err(|e| sevenz_rust2::Error::Io(e, format!("extract {}", entry.name()).into()))?;
        Ok(true)
    };

    sevenz_rust2::decompress_with_extract_fn(source, instance_dir, extract_entry).map_err(
        |e| {
            IOError::with_path(
                std::io::Error::other(e.to_string()),
                archive_path,
            )
        },
    )?;

    std::fs::remove_file(archive_path)
        .map_err(|e| IOError::with_path(e, archive_path))?;
    Ok(())
}

/// Creates the loading bar shown in the app's download/notification area.
/// `total` is in bytes; readers emit their consumption of those bytes.
async fn init_compression_bar(
    instance_id: &str,
    instance_name: &str,
    total: u64,
    message: &str,
) -> crate::Result<LoadingBarId> {
    init_loading(
        LoadingBarType::ZipExtract {
            instance_id: instance_id.to_string(),
            instance_name: instance_name.to_string(),
        },
        total as f64,
        message,
    )
    .await
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

    let total = {
        let dir = instance_dir.clone();
        tokio::task::spawn_blocking(move || {
            let paths = collect_archive_files(&dir)?;
            Ok::<_, crate::Error>(total_size(&paths))
        })
        .await
        .map_err(|e| crate::ErrorKind::OtherError(e.to_string()))??
    };

    let bar = init_compression_bar(
        instance_id,
        &metadata.instance.name,
        total,
        "Compressing profile...",
    )
    .await?;

    let permit = state.io_semaphore.0.acquire().await?;
    let dir = instance_dir.clone();
    let archive = archive_path.clone();
    tokio::task::spawn_blocking(move || {
        let result = create_archive(&dir, &archive, &bar).and_then(|_| {
            prune_extracted_files(&dir, &archive)
        });
        drop(bar);
        result
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
        let total = archive_path.metadata().map_err(IOError::from)?.len();
        let bar = init_compression_bar(
            instance_id,
            &metadata.instance.name,
            total,
            "Extracting profile...",
        )
        .await?;

        let permit = state.io_semaphore.0.acquire().await?;
        let dir = instance_dir.clone();
        let archive = archive_path.clone();
        tokio::task::spawn_blocking(move || {
            let result = extract_archive(&archive, &dir, &bar);
            drop(bar);
            result
        })
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

    let total = archive_path.metadata().map_err(IOError::from)?.len();
    let bar = init_compression_bar(
        instance_id,
        &metadata.instance.name,
        total,
        "Preparing profile...",
    )
    .await?;

    let permit = state.io_semaphore.0.acquire().await?;
    let dir = instance_dir.clone();
    let archive = archive_path.clone();
    tokio::task::spawn_blocking(move || {
        let result = extract_archive(&archive, &dir, &bar);
        drop(bar);
        result
    })
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

    let (total, has_files) = {
        let dir = instance_dir.clone();
        tokio::task::spawn_blocking(move || {
            let paths = collect_archive_files(&dir)?;
            Ok::<_, crate::Error>((total_size(&paths), !paths.is_empty()))
        })
        .await
        .map_err(|e| crate::ErrorKind::OtherError(e.to_string()))??
    };
    if !has_files {
        return Ok(());
    }

    let bar = init_compression_bar(
        instance_id,
        &metadata.instance.name,
        total,
        "Compressing profile...",
    )
    .await?;

    let permit = state.io_semaphore.0.acquire().await?;
    let dir = instance_dir.clone();
    let archive = archive_path.clone();
    tokio::task::spawn_blocking(move || {
        let result = create_archive(&dir, &archive, &bar).and_then(|_| {
            prune_extracted_files(&dir, &archive)
        });
        drop(bar);
        result
    })
    .await
    .map_err(|e| crate::ErrorKind::OtherError(e.to_string()))??;
    drop(permit);

    Ok(())
}
