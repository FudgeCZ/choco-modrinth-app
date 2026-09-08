//! Importer for profiles created by the official Modrinth App.
//!
//! The official app stores profiles in the same layout as this fork
//! (`<config>/profiles/<path>` with metadata in `<config>/app.db`), so an
//! import reads the metadata from the official database and re-registers the
//! profile folder here without re-downloading any mod or world content.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{
    install::{InstallPhaseDetails, InstallProgressReporter},
    pack::{
        import::finish_import,
        install_from::{self, CreatePackDescription, PackDependency},
    },
    state::{ModLoader, State},
};

const OFFICIAL_IDENTIFIER: &str = "ModrinthApp";
const OFFICIAL_DB_FILE: &str = "app.db";

pub fn get_default_modrinth_app_path() -> Option<PathBuf> {
    let path = dirs::data_dir()?.join(OFFICIAL_IDENTIFIER);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

async fn connect_official_db(base_path: &Path) -> crate::Result<sqlx::SqlitePool> {
    let db = base_path.join(OFFICIAL_DB_FILE);
    if !db.exists() {
        return Err(crate::ErrorKind::InputError(format!(
            "Invalid ModrinthApp launcher path, could not find '{}'",
            db.display()
        ))
        .into());
    }

    let url = format!("sqlite://{}?mode=ro", db.to_string_lossy());
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .map_err(|e| {
            crate::ErrorKind::InputError(format!(
                "Could not open Modrinth App database {}: {e}",
                db.display()
            ))
        })?;
    Ok(pool)
}

/// Lists profile folder names registered in the official Modrinth App database
pub async fn get_official_instances(base_path: &Path) -> crate::Result<Vec<String>> {
    let pool = connect_official_db(base_path).await?;

    let rows: Vec<(String,)> = sqlx::query_as(
        "
		SELECT path
		FROM instances
		WHERE COALESCE(install_stage, 'installed') NOT IN ('not_installed', 'archived')
		ORDER BY name
		",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        crate::ErrorKind::InputError(format!(
            "Could not read instances from the Modrinth App database: {e}"
        ))
    })?;

    pool.close().await;
    Ok(rows.into_iter().map(|(path,)| path).collect())
}

pub async fn is_valid_modrinth_app_instance(base_path: &Path, instance_folder: &str) -> bool {
    let result = connect_official_db(base_path).await;
    let pool = match result {
        Ok(pool) => pool,
        Err(_) => return false,
    };
    let exists: Result<Option<(i64,)>, _> = sqlx::query_as("SELECT 1 FROM instances WHERE path = ? LIMIT 1")
        .bind(instance_folder)
        .fetch_optional(&pool)
        .await;
    pool.close().await;
    matches!(exists, Ok(Some(_)))
}

pub async fn import_modrinth_app(
    base_path: PathBuf,      // path to the official Modrinth App config folder
    instance_folder: String, // profile folder name ('path' in the official database)
    instance_id: &str,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    let pool = connect_official_db(&base_path).await?;

    #[derive(Debug, sqlx::FromRow)]
    struct Meta {
        name: Option<String>,
        game_version: Option<String>,
        loader: Option<String>,
        loader_version: Option<String>,
    }

    let metas: Vec<Meta> = sqlx::query_as(
        "
		SELECT i.name, cs.game_version, cs.loader, cs.loader_version
		FROM instances i
		LEFT JOIN instance_content_sets cs ON cs.id = i.applied_content_set_id
		WHERE i.path = ?
		LIMIT 1
		",
    )
    .bind(&instance_folder)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        crate::ErrorKind::InputError(format!(
            "Could not read instance metadata from the Modrinth App database: {e}"
        ))
    })?;
    pool.close().await;

    let meta = metas
        .into_iter()
        .next()
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Profile '{instance_folder}' was not found in the Modrinth App database"
            ))
        })?;

    let game_version = meta.game_version.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Profile '{instance_folder}' has no Minecraft version recorded"
        ))
    })?;

    let name = meta.name.unwrap_or_else(|| instance_folder.clone());
    let loader = meta
        .loader
        .as_deref()
        .map(ModLoader::from_string)
        .unwrap_or(ModLoader::Vanilla);

    let mut dependencies: HashMap<PackDependency, String> = HashMap::new();
    dependencies.insert(PackDependency::Minecraft, game_version.clone());
    match loader {
        ModLoader::Fabric => {
            if let Some(version) = meta.loader_version.clone() {
                dependencies.insert(PackDependency::FabricLoader, version);
            }
        }
        ModLoader::Forge => {
            if let Some(version) = meta.loader_version.clone() {
                dependencies.insert(PackDependency::Forge, version);
            }
        }
        ModLoader::Quilt => {
            if let Some(version) = meta.loader_version.clone() {
                dependencies.insert(PackDependency::QuiltLoader, version);
            }
        }
        ModLoader::NeoForge => {
            if let Some(version) = meta.loader_version.clone() {
                dependencies.insert(PackDependency::NeoForge, version);
            }
        }
        ModLoader::Vanilla => {}
    }

    let description = CreatePackDescription {
        icon: None,
        override_title: Some(name),
        project_id: None,
        version_id: None,
        instance_id: instance_id.to_string(),
        source_filename: None,
    };

    // Registers the instance under the recorded game version and loader
    install_from::set_instance_information(
        instance_id.to_string(),
        &description,
        "Imported from Modrinth App",
        None,
        &dependencies,
        false,
    )
    .await?;

    // The official profile folder is already laid out like a .minecraft folder;
    // copy its contents into the new instance, then install the Minecraft
    // libraries and assets for this launcher
    let source_folder = base_path.join("profiles").join(&instance_folder);
    if source_folder.is_dir() {
        let state = State::get().await?;
        finish_import(
            instance_id,
            source_folder,
            &state.io_semaphore,
            reporter,
            details,
        )
        .await
    } else {
        Ok(())
    }
}
