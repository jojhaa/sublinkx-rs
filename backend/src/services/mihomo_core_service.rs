use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::Duration,
};

use flate2::read::GzDecoder;
use reqwest::Client;
use serde::Deserialize;
use tokio::{process::Command, time};
use zip::ZipArchive;

use crate::{
    dto::settings::{
        MihomoCoreDownloadResponse, MihomoCoreDownloadResult, MihomoCoreStatus,
        MihomoCoreStatusResponse,
    },
    errors::AppError,
    state::AppState,
};

use super::settings_service;

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/MetaCubeX/mihomo/releases/latest";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize, Clone)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

pub async fn get_status(state: &AppState) -> Result<MihomoCoreStatusResponse, AppError> {
    let settings = settings_service::load_settings(state).await?;
    let path = resolve_existing_binary(&settings.latency_core_path);
    let version = match path.as_ref() {
        Some(path) => read_mihomo_version(path).await,
        None => None,
    };
    let target = current_target();

    Ok(MihomoCoreStatusResponse {
        code: "00000",
        data: MihomoCoreStatus {
            os: target.os.to_string(),
            arch: target.arch.to_string(),
            supported: target.supported,
            installed: path.is_some(),
            path: path.map(|value| value.display().to_string()),
            version,
            message: if target.supported {
                if target.needs_go123 {
                    "Mihomo core target is supported. Linux kernel is older than 3.2, so downloads will prefer go123 builds.".to_string()
                } else {
                    "Mihomo core target is supported.".to_string()
                }
            } else {
                "Current server OS/architecture is not supported by automatic download.".to_string()
            },
        },
    })
}

pub async fn download_latest(state: &AppState) -> Result<MihomoCoreDownloadResponse, AppError> {
    let target = current_target();
    if !target.supported {
        return Err(AppError::BadRequest(format!(
            "unsupported server target: {} {}",
            target.os, target.arch
        )));
    }

    let client = Client::builder()
        .user_agent("sublinkx-rs")
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| AppError::Internal)?;
    let release = client
        .get(LATEST_RELEASE_API)
        .send()
        .await
        .map_err(|error| AppError::BadRequest(format!("failed to query Mihomo release: {error}")))?
        .error_for_status()
        .map_err(|error| AppError::BadRequest(format!("failed to query Mihomo release: {error}")))?
        .json::<GitHubRelease>()
        .await
        .map_err(|error| {
            AppError::BadRequest(format!("failed to parse Mihomo release: {error}"))
        })?;

    let asset = select_asset(&release, &target).ok_or_else(|| {
        AppError::BadRequest(format!(
            "no Mihomo asset found for {} {} in {}",
            target.os, target.arch, release.tag_name
        ))
    })?;
    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|error| AppError::BadRequest(format!("failed to download Mihomo core: {error}")))?
        .error_for_status()
        .map_err(|error| AppError::BadRequest(format!("failed to download Mihomo core: {error}")))?
        .bytes()
        .await
        .map_err(|error| AppError::BadRequest(format!("failed to read Mihomo core: {error}")))?;

    let install_dir = preferred_install_dir()?;
    fs::create_dir_all(&install_dir).map_err(|_| AppError::Internal)?;
    let binary_path = extract_binary(&asset.name, &bytes, &install_dir)?;
    make_executable(&binary_path)?;
    settings_service::update_latency_core_path(state, &binary_path.display().to_string()).await?;

    Ok(MihomoCoreDownloadResponse {
        code: "00000",
        data: MihomoCoreDownloadResult {
            os: target.os.to_string(),
            arch: target.arch.to_string(),
            version: release.tag_name,
            asset_name: asset.name,
            path: binary_path.display().to_string(),
            size: asset.size,
        },
    })
}

pub(crate) fn resolve_existing_binary(configured_path: &str) -> Option<PathBuf> {
    let configured = configured_path.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        if path.exists() {
            return Some(path);
        }
    }

    for base in search_bases() {
        for candidate in candidate_paths(&base) {
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

pub(crate) fn fallback_binary_path() -> PathBuf {
    PathBuf::from("mihomo")
}

fn select_asset(release: &GitHubRelease, target: &Target) -> Option<GitHubAsset> {
    let mut candidates = release
        .assets
        .iter()
        .filter(|asset| asset.name.contains(target.os) && asset.name.contains(target.arch))
        .filter(|asset| {
            if target.os == "windows" {
                asset.name.ends_with(".zip")
            } else {
                asset.name.ends_with(".gz")
            }
        })
        .cloned()
        .collect::<Vec<_>>();

    candidates.sort_by_key(|asset| asset_rank(&asset.name, target));
    candidates.into_iter().next()
}

fn asset_rank(name: &str, target: &Target) -> u8 {
    if target.needs_go123 && name.contains("go123") {
        return 0;
    }
    if target.needs_go123 && name.contains("-go") {
        return 1;
    }
    if target.arch == "amd64" && name.contains("-amd64-v1-v") && !name.contains("-go") {
        return 2;
    }
    if target.arch == "amd64" && name.contains("-amd64-v1-") {
        return 3;
    }
    if target.arch != "amd64" && !name.contains("-go") {
        return 4;
    }
    if target.arch == "amd64" && name.contains("compatible") {
        return 5;
    }
    if target.arch == "amd64" && !name.contains("-go") {
        return 6;
    }
    7
}

fn extract_binary(asset_name: &str, bytes: &[u8], install_dir: &Path) -> Result<PathBuf, AppError> {
    let output_name = if cfg!(windows) {
        "mihomo.exe"
    } else {
        "mihomo"
    };
    let output_path = install_dir.join(output_name);

    if asset_name.ends_with(".gz") {
        let mut decoder = GzDecoder::new(Cursor::new(bytes));
        let mut output = Vec::new();
        decoder.read_to_end(&mut output).map_err(|error| {
            AppError::BadRequest(format!("failed to decompress Mihomo core: {error}"))
        })?;
        fs::write(&output_path, output).map_err(|_| AppError::Internal)?;
        return Ok(output_path);
    }

    if asset_name.ends_with(".zip") {
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|error| AppError::BadRequest(format!("failed to open Mihomo zip: {error}")))?;
        for index in 0..archive.len() {
            let mut file = archive.by_index(index).map_err(|error| {
                AppError::BadRequest(format!("failed to read Mihomo zip: {error}"))
            })?;
            let Some(name) = Path::new(file.name())
                .file_name()
                .and_then(|value| value.to_str())
            else {
                continue;
            };
            let is_binary = if cfg!(windows) {
                name.eq_ignore_ascii_case("mihomo.exe") || name.ends_with(".exe")
            } else {
                name == "mihomo" || name.starts_with("mihomo-")
            };
            if is_binary {
                let mut output = Vec::new();
                file.read_to_end(&mut output).map_err(|error| {
                    AppError::BadRequest(format!("failed to extract Mihomo zip: {error}"))
                })?;
                fs::write(&output_path, output).map_err(|_| AppError::Internal)?;
                return Ok(output_path);
            }
        }
        return Err(AppError::BadRequest(
            "Mihomo zip does not contain a supported binary".to_string(),
        ));
    }

    Err(AppError::BadRequest(format!(
        "unsupported Mihomo asset format: {asset_name}"
    )))
}

async fn read_mihomo_version(path: &Path) -> Option<String> {
    let output = time::timeout(
        Duration::from_secs(3),
        Command::new(path).arg("-v").kill_on_drop(true).output(),
    )
    .await
    .ok()?
    .ok()?;

    let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&output.stderr).trim().to_string();
    }
    if text.is_empty() {
        None
    } else {
        Some(text.lines().next().unwrap_or(&text).to_string())
    }
}

fn preferred_install_dir() -> Result<PathBuf, AppError> {
    Ok(std::env::current_dir()
        .map_err(|_| AppError::Internal)?
        .join("mihomo"))
}

fn search_bases() -> Vec<PathBuf> {
    let mut bases = Vec::new();
    if let Ok(current) = std::env::current_dir() {
        bases.push(current.clone());
        if let Some(parent) = current.parent() {
            bases.push(parent.to_path_buf());
        }
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        bases.push(parent.to_path_buf());
        if let Some(grand_parent) = parent.parent() {
            bases.push(grand_parent.to_path_buf());
        }
    }
    bases
}

fn candidate_paths(base: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for name in candidate_names() {
        paths.push(base.join("mihomo").join(name));
        paths.push(base.join(name));
    }
    paths
}

fn candidate_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &[
            "mihomo.exe",
            "mihomo-windows-amd64.exe",
            "mihomo-windows-amd64-compatible.exe",
            "clash-meta.exe",
        ]
    } else {
        &["mihomo", "mihomo-linux-amd64", "clash-meta"]
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|_| AppError::Internal)?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|_| AppError::Internal)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

#[derive(Debug)]
struct Target {
    os: &'static str,
    arch: &'static str,
    supported: bool,
    needs_go123: bool,
}

fn current_target() -> Target {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "darwin",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "arm" => "armv7",
        "x86" => "386",
        other => other,
    };
    let supported = matches!(os, "windows" | "linux" | "darwin")
        && matches!(arch, "amd64" | "arm64" | "armv7" | "386");
    let needs_go123 = os == "linux" && linux_kernel_needs_go123();

    Target {
        os,
        arch,
        supported,
        needs_go123,
    }
}

fn linux_kernel_needs_go123() -> bool {
    let Ok(output) = std::process::Command::new("uname").arg("-r").output() else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.trim().split('-').next().unwrap_or_default();
    let mut parts = version
        .split('.')
        .filter_map(|part| part.parse::<u64>().ok());
    let Some(major) = parts.next() else {
        return false;
    };
    let minor = parts.next().unwrap_or(0);
    major < 3 || (major == 3 && minor < 2)
}
