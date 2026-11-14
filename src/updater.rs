//! Kontrola aktualizací a samoobnovení aplikace.
//!
//! Data pocházejí z GitHub API (`/releases/latest`). Po stažení archivu se
//! spustí odpojený skript, který počká na ukončení procesu, rozbalí archiv na
//! místo instalace a aplikaci znovu spustí.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Result};
use semver::Version;
use tracing::{info, warn};

use crate::state::SharedState;

/// Repozitář, ze kterého se aktualizace stahují.
pub const REPO: &str = "FoxxoOwO/warust";
/// Verze, se kterou je aplikace zkompilovaná.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Jak často se aktualizace kontrolují na pozadí.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

const USER_AGENT: &str = concat!("Warust/", env!("CARGO_PKG_VERSION"));

/// Nalezená novější verze.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub tag: String,
    pub page_url: String,
    pub notes: String,
    pub asset_url: Option<String>,
    pub asset_name: Option<String>,
}

impl UpdateInfo {
    /// Lze aktualizaci nainstalovat přímo z aplikace?
    pub fn has_asset(&self) -> bool {
        self.asset_url.is_some()
    }
}

/// Stav kontroly aktualizací.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(String),
    Downloading,
    Failed(String),
}

impl UpdateStatus {
    pub fn label(&self) -> String {
        match self {
            UpdateStatus::Idle => String::new(),
            UpdateStatus::Checking => "Zjišťuji novou verzi…".to_string(),
            UpdateStatus::UpToDate => "Máš nejnovější verzi.".to_string(),
            UpdateStatus::Available(version) => format!("Dostupná verze {}.", version),
            UpdateStatus::Downloading => "Stahuji aktualizaci…".to_string(),
            UpdateStatus::Failed(error) => format!("Kontrola selhala: {}", error),
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, UpdateStatus::Checking | UpdateStatus::Downloading)
    }
}

fn clean_version(value: &str) -> &str {
    value.trim().trim_start_matches(['v', 'V'])
}

/// Je `candidate` novější než `current`?
pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (
        Version::parse(clean_version(current)),
        Version::parse(clean_version(candidate)),
    ) {
        (Ok(current), Ok(candidate)) => candidate > current,
        _ => false,
    }
}

/// Přípona balíčku, který patří této platformě.
pub fn asset_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-x86_64.zip"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos-arm64.tar.gz"
    } else if cfg!(target_os = "macos") {
        "macos-x86_64.tar.gz"
    } else {
        "linux-x86_64.tar.gz"
    }
}

/// Vytáhne informace o vydání z JSON odpovědi GitHubu.
pub fn parse_release(json: &serde_json::Value, suffix: &str) -> Option<UpdateInfo> {
    let tag = json.get("tag_name")?.as_str()?.to_string();
    let version = clean_version(&tag).to_string();
    let page_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let notes = json
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let asset = json.get("assets").and_then(|v| v.as_array()).and_then(|assets| {
        assets.iter().find_map(|asset| {
            let name = asset.get("name")?.as_str()?;
            if !name.ends_with(suffix) {
                return None;
            }
            let url = asset.get("browser_download_url")?.as_str()?.to_string();
            Some((name.to_string(), url))
        })
    });

    let (asset_name, asset_url) = match asset {
        Some((name, url)) => (Some(name), Some(url)),
        None => (None, None),
    };

    Some(UpdateInfo {
        version,
        tag,
        page_url,
        notes,
        asset_url,
        asset_name,
    })
}

/// Zeptá se GitHubu na nejnovější vydání.
///
/// Vrací `Ok(None)`, když je nainstalovaná verze stejná nebo novější.
pub fn fetch_latest() -> Result<Option<UpdateInfo>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut response = ureq::get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()?;
    let body = response.body_mut().read_to_string()?;
    let json: serde_json::Value = serde_json::from_str(&body)?;
    let info = parse_release(&json, asset_suffix());
    Ok(info.filter(|info| is_newer(CURRENT_VERSION, &info.version)))
}

/// Stáhne balíček do dočasné složky a vrátí cestu k němu.
pub fn download(url: &str) -> Result<PathBuf> {
    let mut response = ureq::get(url).header("User-Agent", USER_AGENT).call()?;
    let bytes = response.body_mut().read_to_vec()?;
    let name = url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("warust-update");
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, &bytes)?;
    Ok(path)
}

/// Rozbalí archiv, přepíše spustitelný soubor a aplikaci znovu spustí.
///
/// Na Windows je běžící `.exe` zamčené, proto výměnu provádí odpojený batch
/// skript, který nejdřív počká na ukončení tohoto procesu.
pub fn install_and_restart(archive: &Path) -> Result<()> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("spustitelný soubor nemá nadřazenou složku"))?
        .to_path_buf();

    #[cfg(windows)]
    {
        let pid = std::process::id();
        let script = std::env::temp_dir().join("warust-update.cmd");
        let body = format!(
            "@echo off\r\n\
             :wait\r\n\
             tasklist /FI \"PID eq {pid}\" | find \"{pid}\" >nul\r\n\
             if not errorlevel 1 (\r\n\
             \x20 timeout /t 1 /nobreak >nul\r\n\
             \x20 goto wait\r\n\
             )\r\n\
             cd /d \"{dir}\"\r\n\
             tar -xf \"{zip}\" -C \"{dir}\"\r\n\
             start \"\" \"{exe}\"\r\n\
             del \"{zip}\"\r\n\
             del \"%~f0\"\r\n",
            pid = pid,
            dir = dir.display(),
            exe = exe.display(),
            zip = archive.display(),
        );
        std::fs::write(&script, body)?;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("cmd")
            .arg("/C")
            .arg(&script)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        let script = std::env::temp_dir().join("warust-update.sh");
        let body = format!(
            "#!/bin/sh\n\
             while kill -0 {pid} 2>/dev/null; do sleep 1; done\n\
             cd \"{dir}\" || exit 1\n\
             tar -xzf \"{zip}\" -C \"{dir}\"\n\
             \"{exe}\" >/dev/null 2>&1 &\n\
             rm -f \"{zip}\"\n",
            pid = std::process::id(),
            dir = dir.display(),
            exe = exe.display(),
            zip = archive.display(),
        );
        std::fs::write(&script, body)?;
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script, perms)?;
        }
        std::process::Command::new("sh").arg(&script).spawn()?;
    }

    info!("Aktualizace připravena, ukončuji aplikaci kvůli výměně souborů");
    std::process::exit(0);
}

/// Zkontroluje aktualizace a výsledek zapíše do stavu (blokující).
///
/// Vrací `true`, když se má aplikace kvůli automatické instalaci ukončit.
pub fn check_now(state: &SharedState) -> bool {
    state.write().update_status = UpdateStatus::Checking;
    let result = fetch_latest();
    match result {
        Ok(Some(info)) => {
            let auto_install = {
                let mut s = state.write();
                s.update_status = UpdateStatus::Available(info.version.clone());
                s.update_available = Some(info.clone());
                s.preferences.auto_install_updates
            };
            info!("Nová verze {} je k dispozici", info.version);
            if auto_install {
                return install_now(state, &info);
            }
            false
        }
        Ok(None) => {
            let mut s = state.write();
            s.update_available = None;
            s.update_status = UpdateStatus::UpToDate;
            false
        }
        Err(error) => {
            warn!("Kontrola aktualizací selhala: {error}");
            state.write().update_status = UpdateStatus::Failed(error.to_string());
            false
        }
    }
}

/// Stáhne balíček a předá ho instalačnímu skriptu (blokující).
pub fn install_now(state: &SharedState, info: &UpdateInfo) -> bool {
    let Some(url) = info.asset_url.clone() else {
        state.write().update_status =
            UpdateStatus::Failed("vydání nemá balíček pro tuto platformu".to_string());
        return false;
    };
    state.write().update_status = UpdateStatus::Downloading;
    match download(&url) {
        Ok(path) => match install_and_restart(&path) {
            Ok(()) => true,
            Err(error) => {
                state.write().update_status = UpdateStatus::Failed(error.to_string());
                false
            }
        },
        Err(error) => {
            state.write().update_status = UpdateStatus::Failed(error.to_string());
            false
        }
    }
}

/// Spustí kontrolu na pozadí (jednorázově).
pub fn spawn_check(state: SharedState) {
    std::thread::spawn(move || {
        check_now(&state);
    });
}

/// Spustí stažení a instalaci na pozadí.
pub fn spawn_install(state: SharedState, info: UpdateInfo) {
    std::thread::spawn(move || {
        install_now(&state, &info);
    });
}

/// Pravidelná kontrola na pozadí podle uživatelského nastavení.
pub fn spawn_periodic_check(state: SharedState) {
    std::thread::spawn(move || loop {
        if !state.read().preferences.check_updates {
            std::thread::sleep(Duration::from_secs(60));
            continue;
        }
        check_now(&state);
        std::thread::sleep(CHECK_INTERVAL);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_are_detected() {
        assert!(is_newer("0.2.0", "0.3.0"));
        assert!(is_newer("0.2.0", "v0.2.1"));
        assert!(is_newer("1.0.0", "1.0.1"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.3.0", "0.2.9"));
        assert!(!is_newer("0.2.0", "nightly"));
        assert!(!is_newer("not-a-version", "0.3.0"));
    }

    #[test]
    fn release_json_picks_the_platform_asset() {
        let json = serde_json::json!({
            "tag_name": "v0.3.0",
            "html_url": "https://example.test/releases/v0.3.0",
            "body": "Notes",
            "assets": [
                { "name": "warust-v0.3.0-linux-x86_64.tar.gz", "browser_download_url": "https://example.test/linux" },
                { "name": "warust-v0.3.0-windows-x86_64.zip", "browser_download_url": "https://example.test/win" }
            ]
        });
        let info = parse_release(&json, "windows-x86_64.zip").expect("release parses");
        assert_eq!(info.version, "0.3.0");
        assert_eq!(info.tag, "v0.3.0");
        assert_eq!(info.asset_url.as_deref(), Some("https://example.test/win"));
        assert_eq!(
            info.asset_name.as_deref(),
            Some("warust-v0.3.0-windows-x86_64.zip")
        );
        assert!(info.has_asset());
    }

    #[test]
    fn release_json_without_matching_asset_still_parses() {
        let json = serde_json::json!({
            "tag_name": "v0.3.0",
            "assets": [
                { "name": "warust-v0.3.0-linux-x86_64.tar.gz", "browser_download_url": "https://example.test/linux" }
            ]
        });
        let info = parse_release(&json, "windows-x86_64.zip").expect("release parses");
        assert!(info.asset_url.is_none());
        assert!(!info.has_asset());
    }

    #[test]
    fn status_labels_are_readable() {
        assert!(UpdateStatus::Checking.label().contains("Zjišťuji"));
        assert!(UpdateStatus::Available("0.3.0".to_string())
            .label()
            .contains("0.3.0"));
        assert!(UpdateStatus::Failed("offline".to_string())
            .label()
            .contains("offline"));
        assert!(UpdateStatus::UpToDate.label().contains("nejnovější"));
        assert!(UpdateStatus::Checking.is_busy());
        assert!(!UpdateStatus::Idle.is_busy());
    }
}
