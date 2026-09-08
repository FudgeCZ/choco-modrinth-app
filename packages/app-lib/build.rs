use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};
use std::{env, fs};

fn main() {
    println!("cargo::rerun-if-changed=.env");
    println!("cargo::rerun-if-changed=java/gradle");
    println!("cargo::rerun-if-changed=java/src");
    println!("cargo::rerun-if-changed=java/build.gradle.kts");
    println!("cargo::rerun-if-changed=java/settings.gradle.kts");
    println!("cargo::rerun-if-changed=java/gradle.properties");

    set_env();
    build_java_jars();
}

fn set_env() {
    for (var_name, var_value) in
        dotenvy::dotenv_iter().into_iter().flatten().flatten()
    {
        if var_name == "DATABASE_URL" {
            // The sqlx database URL is a build-time detail that should not be exposed to the crate
            continue;
        }

        println!("cargo::rustc-env={var_name}={var_value}");
    }
}

fn build_java_jars() {
    let out_dir =
        dunce::canonicalize(PathBuf::from(env::var_os("OUT_DIR").unwrap()))
            .unwrap();

    let libs_dir = out_dir.join("java/libs");
    println!(
        "cargo::rustc-env=JAVA_JARS_DIR={}",
        libs_dir.display()
    );

    // ChocoModrinth: skip the (multi-minute) Gradle build when the jars are
    // already up to date. The rerun-if-changed directives above already force
    // a rebuild of the build script whenever the Java sources change; the
    // freshness check below covers the case where only the jars are missing.
    let java_src = dunce::canonicalize("java").unwrap();
    if libs_dir.exists() {
        let jar_time = fs::metadata(&libs_dir)
            .and_then(|m| m.modified())
            .ok();
        let src_time = latest_mtime(&java_src);
        if let (Some(jar_time), Some(src_time)) = (jar_time, src_time) {
            if jar_time >= src_time {
                return;
            }
        } else {
            return;
        }
    }

    let gradle_path = fs::canonicalize(
        #[cfg(target_os = "windows")]
        "java\\gradlew.bat",
        #[cfg(not(target_os = "windows"))]
        "java/gradlew",
    )
    .unwrap();

    let mut build_dir_str = OsString::from("-Dorg.gradle.project.buildDir=");
    build_dir_str.push(out_dir.join("java"));
    let exit_status = Command::new(gradle_path)
        .arg(build_dir_str)
        .arg("build")
        .arg("--no-daemon")
        .arg("--console=rich")
        .current_dir(dunce::canonicalize("java").unwrap())
        .status()
        .expect("Failed to wait on Gradle build");

    if !exit_status.success() {
        println!("cargo::error=Gradle build failed with {exit_status}");
        exit(exit_status.code().unwrap_or(1));
    }
}

/// Newest modification time within a directory tree (used to decide whether
/// the Gradle-built jars are stale)
fn latest_mtime(dir: &Path) -> Option<std::time::SystemTime> {
    let mut latest = fs::metadata(dir).and_then(|m| m.modified()).ok();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(t) = latest_mtime(&path)
                    && latest.is_none_or(|current| t > current)
                {
                    latest = Some(t);
                }
            } else if let Ok(t) = entry.metadata().and_then(|m| m.modified())
                && latest.is_none_or(|current| t > current)
            {
                latest = Some(t);
            }
        }
    }
    latest
}
