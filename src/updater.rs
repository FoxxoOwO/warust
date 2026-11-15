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
            UpdateStatus::Checking => crate::tr!("Zjišťuji novou verzi…").to_string(),
            UpdateStatus::UpToDate => crate::tr!("Máš nejnovější verzi.").to_string(),
            UpdateStatus::Available(version) => crate::trf!("Dostupná verze {}.", version),
            UpdateStatus::Downloading => crate::tr!("Stahuji aktualizaci…").to_string(),
            UpdateStatus::Failed(error) => format!("{}: {}", crate::tr!("Chyba"), error),
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

/// Zeptá se GitHubu na nejnovější vydání bez ohledu na nainstalovanou verzi.
///
/// `fetch_latest` navíc filtruje podle `is_newer`; diagnostika (`--test-update`)
/// potřebuje i to, co je stejné, aby šlo ověřit balíček, který zrovna vyšel.
pub fn fetch_latest_release() -> Result<Option<UpdateInfo>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut response = ureq::get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()?;
    let body = response.body_mut().read_to_string()?;
    let json: serde_json::Value = serde_json::from_str(&body)?;
    Ok(parse_release(&json, asset_suffix()))
}

/// Zeptá se GitHubu na nejnovější vydání.
///
/// Vrací `Ok(None)`, když je nainstalovaná verze stejná nebo novější.
pub fn fetch_latest() -> Result<Option<UpdateInfo>> {
    Ok(fetch_latest_release()?.filter(|info| is_newer(CURRENT_VERSION, &info.version)))
}

/// Stáhne balíček do dočasné složky a vrátí cestu k němu.
pub fn download(url: &str) -> Result<PathBuf> {
    let mut response = ureq::get(url).header("User-Agent", USER_AGENT).call()?;
    let name = url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("warust-update");
    let path = std::env::temp_dir().join(name);
    // Tělo se teče rovnou do souboru, ne přes `read_to_vec()`: ten má v ureq 3
    // strop 10 MiB a balíček má kolem 40 MB, takže stahování skončilo chybou
    // "the response body is larger than request limit" a nic se neuložilo.
    // `as_reader()` limit nemá a do paměti se nic nekopíruje.
    let written = save_body_to_file(response.body_mut().as_reader(), &path)?;
    if written == 0 {
        let _ = std::fs::remove_file(&path);
        return Err(anyhow!("server vrátil prázdný balíček ({url})"));
    }
    Ok(path)
}

/// Překopíruje tělo odpovědi do souboru a vrátí počet zapsaných bajtů.
///
/// Nedopsaný soubor se po chybě smaže: při dalším pokusu by se s ním jinak
/// pracovalo jako s platným balíčkem.
fn save_body_to_file<R: std::io::Read>(mut reader: R, path: &Path) -> Result<u64> {
    let mut file = std::fs::File::create(path)?;
    match std::io::copy(&mut reader, &mut file) {
        Ok(written) => Ok(written),
        Err(error) => {
            drop(file);
            let _ = std::fs::remove_file(path);
            Err(error.into())
        }
    }
}

/// Skript pro Windows, který počká na ukončení procesu, rozbalí archiv a
/// aplikaci znovu spustí.
///
/// `tar` se volá přes `%SystemRoot%\System32\tar.exe`, protože `cmd` najde na
/// PATH dřív `tar` z Gitu (GNU tar), a ten umí jen tar archivy — náš balíček je
/// zip, takže s ním tiše selhal a aplikace se znovu otevřela ve staré verzi.
/// Když bsdtar chybí (Windows starší než 1803), rozbaluje PowerShell.
#[cfg(windows)]
fn windows_update_script(pid: u32, dir: &Path, exe: &Path, archive: &Path) -> String {
    let exe_name = exe
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "warust.exe".to_string());
    format!(
        "@echo off\r\n\
         :wait\r\n\
         tasklist /FI \"PID eq {pid}\" | find \"{pid}\" >nul\r\n\
         if not errorlevel 1 (\r\n\
         \x20 timeout /t 1 /nobreak >nul\r\n\
         \x20 goto wait\r\n\
         )\r\n\
         set \"LOG=%TEMP%\\warust-update.log\"\r\n\
         set \"ZIP={zip}\"\r\n\
         set \"DIR={dir}\"\r\n\
         set \"STAGE=%TEMP%\\warust-unpack\"\r\n\
         echo --- aktualizace %DATE% %TIME% >>\"%LOG%\"\r\n\
         if exist \"%STAGE%\" rmdir /s /q \"%STAGE%\"\r\n\
         mkdir \"%STAGE%\"\r\n\
         set \"TAR=%SystemRoot%\\System32\\tar.exe\"\r\n\
         if exist \"%TAR%\" \"%TAR%\" -xf \"%ZIP%\" -C \"%STAGE%\" >>\"%LOG%\" 2>&1\r\n\
         if not exist \"%STAGE%\\{exe_name}\" powershell -NoProfile -ExecutionPolicy Bypass -Command \"Expand-Archive -LiteralPath '%ZIP%' -DestinationPath '%STAGE%' -Force\" >>\"%LOG%\" 2>&1\r\n\
         if exist \"%STAGE%\\{exe_name}\" (\r\n\
         \x20 copy /y \"%STAGE%\\{exe_name}\" \"%DIR%\\{exe_name}\" >>\"%LOG%\" 2>&1\r\n\
         \x20 for %%F in (ffmpeg.exe ffprobe.exe FFMPEG-NOTICE.txt) do if exist \"%STAGE%\\%%F\" copy /y \"%STAGE%\\%%F\" \"%DIR%\\%%F\" >>\"%LOG%\" 2>&1\r\n\
         \x20 rmdir /s /q \"%STAGE%\"\r\n\
         ) else (\r\n\
         \x20 echo rozbaleni selhalo, zustava stara verze >>\"%LOG%\"\r\n\
         )\r\n\
         del \"%ZIP%\"\r\n\
         start \"\" \"%DIR%\\{exe_name}\"\r\n\
         del \"%~f0\"\r\n",
        pid = pid,
        dir = dir.display(),
        zip = archive.display(),
        exe_name = exe_name,
    )
}

/// Skript pro Unix, který počká na ukončení procesu, rozbalí archiv a aplikaci
/// znovu spustí.
#[cfg(not(windows))]
fn unix_update_script(pid: u32, dir: &Path, exe: &Path, archive: &Path) -> String {
    format!(
        "#!/bin/sh\n\
         log=\"${{TMPDIR:-/tmp}}/warust-update.log\"\n\
         stage=\"${{TMPDIR:-/tmp}}/warust-unpack\"\n\
         while kill -0 {pid} 2>/dev/null; do sleep 1; done\n\
         rm -rf \"$stage\"\n\
         mkdir -p \"$stage\"\n\
         if tar -xzf \"{zip}\" -C \"$stage\" >>\"$log\" 2>&1; then\n\
         \x20 cp -f \"$stage\"/* \"{dir}/\" >>\"$log\" 2>&1\n\
         \x20 echo aktualizace rozbalena >>\"$log\"\n\
         else\n\
         \x20 echo rozbaleni selhalo, zustava stara verze >>\"$log\"\n\
         fi\n\
         rm -rf \"$stage\"\n\
         rm -f \"{zip}\"\n\
         \"{exe}\" >/dev/null 2>&1 &\n",
        pid = pid,
        dir = dir.display(),
        exe = exe.display(),
        zip = archive.display(),
    )
}

/// Rozbalí archiv, přepíše spustitelný soubor a aplikaci znovu spustí.
///
/// Na Windows je běžící `.exe` zamčené, proto výměnu provádí odpojený batch
/// skript, který nejdřív počká na ukončení tohoto procesu.
/// Seznam souborů ve stagingu a jestli mezi nimi je spustitelný `warust.exe`.
///
/// Oddělené od stahování, aby šlo ověřit bez sítě: instalace je hotová jen když
/// v rozbalené složce leží spustitelný soubor, ne jen „tar nespadl“.
fn inspect_staging(dir: &Path) -> Result<(Vec<(String, u64)>, bool)> {
    let mut files = Vec::new();
    let mut has_exe = false;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if name == "warust.exe" || name == "warust" {
            has_exe = std::fs::read(&path)
                .map(|head| head.len() >= 2 && (&head[..2] == b"MZ" || &head[..2] == b"E"))
                .unwrap_or(false);
        }
        files.push((name, size));
    }
    files.sort();
    Ok((files, has_exe))
}

/// Stáhne nejnovější balíček a zkusí ho rozbalit — bez výměny souborů.
///
/// Dělá přesně to, co instalátor (systémový `tar`, staging složka), ale nic
/// nepřepisuje a aplikaci neukončuje. Tím se dá ověřit, že balíček jde rozbalit,
/// aniž by člověk přišel o instalaci, a je to jediný způsob, jak tuhle cestu
/// otestovat před vydáním.
pub fn self_test_update() -> Result<()> {
    let info = fetch_latest_release()?.ok_or_else(|| anyhow!("GitHub nevrátil žádné vydání"))?;
    info!(
        "test aktualizace: nejnovější vydání {} (nainstalováno {})",
        info.version, CURRENT_VERSION
    );
    let url = info
        .asset_url
        .clone()
        .ok_or_else(|| anyhow!("vydání {} nemá balíček pro tento systém", info.version))?;
    info!("test aktualizace: stahuji {url}");
    let archive = download(&url)?;
    let size = std::fs::metadata(&archive)?.len();
    info!("test aktualizace: {} bajtů v {}", size, archive.display());

    let staging = std::env::temp_dir().join("warust-selftest");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;

    #[cfg(windows)]
    let tar = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_default())
        .join("System32")
        .join("tar.exe");
    #[cfg(not(windows))]
    let tar = std::path::PathBuf::from("tar");

    let output = std::process::Command::new(&tar)
        .arg("-x")
        .arg("-f")
        .arg(&archive)
        .arg("-C")
        .arg(&staging)
        .output()?;
    info!(
        "test aktualizace: {} skončil {:?}",
        tar.display(),
        output.status.code()
    );
    if !output.stderr.is_empty() {
        warn!(
            "test aktualizace: stderr: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let (files, has_exe) = inspect_staging(&staging)?;
    for (name, bytes) in &files {
        info!("test aktualizace: rozbaleno {name} ({bytes} B)");
    }
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_file(&archive);

    if !has_exe {
        return Err(anyhow!(
            "v rozbaleném balíčku není spustitelný warust.exe (soubory: {:?})",
            files.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
        ));
    }
    info!(
        "test aktualizace: OK — {} souborů, warust.exe je spustitelný",
        files.len()
    );
    Ok(())
}

pub fn install_and_restart(archive: &Path) -> Result<()> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("spustitelný soubor nemá nadřazenou složku"))?
        .to_path_buf();

    #[cfg(windows)]
    {
        let script = std::env::temp_dir().join("warust-update.cmd");
        std::fs::write(
            &script,
            windows_update_script(std::process::id(), &dir, &exe, archive),
        )?;
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
        std::fs::write(
            &script,
            unix_update_script(std::process::id(), &dir, &exe, archive),
        )?;
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
        state.write().update_status = UpdateStatus::Failed(crate::tr!(
            "vydání nemá balíček pro tuto platformu"
        )
        .to_string());
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

    /// `cmd` má na PATH dřív GNU tar z Gitu, který zip nerozbalí, takže skript
    /// musí sáhnout po systémovém bsdtaru a mít záložní cestu přes PowerShell.
    #[cfg(windows)]
    #[test]
    fn windows_script_extracts_with_the_system_tar() {
        let script = windows_update_script(
            4242,
            Path::new(r"C:\Apps\Warust"),
            Path::new(r"C:\Apps\Warust\warust.exe"),
            Path::new(r"C:\Temp\warust-v0.3.1-windows-x86_64.zip"),
        );
        assert!(
            script.contains(r"%SystemRoot%\System32\tar.exe"),
            "{script}"
        );
        assert!(script.contains("Expand-Archive"), "{script}");
        assert!(
            script.contains(r#"copy /y "%STAGE%\warust.exe" "%DIR%\warust.exe""#),
            "{script}"
        );
        assert!(script.contains("PID eq 4242"), "{script}");
        assert!(
            script.contains(r#"start "" "%DIR%\warust.exe""#),
            "{script}"
        );
        // Stará verze zůstává, když rozbalení selže.
        assert!(script.contains("zustava stara verze"), "{script}");
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_script_extracts_then_restarts() {
        let script = unix_update_script(
            4242,
            Path::new("/opt/warust"),
            Path::new("/opt/warust/warust"),
            Path::new("/tmp/warust-v0.3.1-linux-x86_64.tar.gz"),
        );
        assert!(script.contains("tar -xzf"), "{script}");
        assert!(script.contains("zustava stara verze"), "{script}");
        assert!(script.contains(r#""/opt/warust/warust""#), "{script}");
    }

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

    /// Balíček je rozbalený správně jen když v něm leží spustitelný soubor.
    /// Samotný název nestačí: prázdný nebo zkomolený soubor se stejným jménem
    /// vypadá jako úspěch a aplikace se pak neaktualizuje.
    #[test]
    fn staging_needs_an_executable_binary() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ffmpeg.exe"), b"MZ ffmpeg").unwrap();
        let (files, ok) = inspect_staging(dir.path()).unwrap();
        assert!(!ok, "soubory: {files:?}");

        std::fs::write(dir.path().join("warust.exe"), b"MZ warust").unwrap();
        let (files, ok) = inspect_staging(dir.path()).unwrap();
        assert!(ok);
        assert!(files.iter().any(|(name, _)| name == "warust.exe"));

        std::fs::write(dir.path().join("warust.exe"), b"").unwrap();
        assert!(!inspect_staging(dir.path()).unwrap().1);
    }

    #[test]
    fn status_labels_are_readable() {
        let _language = crate::i18n::lock_language();
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

    /// Balíček má kolem 40 MB, ale `read_to_vec()` má v ureq 3 strop 10 MiB a
    /// stahování na něm skončilo chybou "the response body is larger than request
    /// limit". Tělo se proto teče do souboru; test hlídá, že se limit neuplatní
    /// a že se zapíše všechno.
    #[test]
    fn stazeny_balicek_se_tece_do_souboru_cely() {
        const DATA: u64 = 12 * 1024 * 1024;

        /// Nekonečný zdroj, který se po `DATA` bajtech zastaví.
        struct NekonecnyZroj {
            zbyva: u64,
        }

        impl std::io::Read for NekonecnyZroj {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let kus = (buf.len() as u64).min(self.zbyva) as usize;
                buf[..kus].fill(b'x');
                self.zbyva -= kus as u64;
                Ok(kus)
            }
        }

        let path = std::env::temp_dir().join(format!(
            "warust-test-tece-{}.zip", std::process::id()
        ));
        let written = save_body_to_file(NekonecnyZroj { zbyva: DATA }, &path).unwrap();
        assert_eq!(written, DATA);
        let saved = std::fs::metadata(&path).unwrap().len();
        let _ = std::fs::remove_file(&path);
        assert_eq!(saved, DATA, "na disk se dostalo jen {saved} z {DATA} bajtů");
    }

    /// Nedopsaný balíček by při dalším pokusu vypadal jako platný, takže po
    /// chybě musí po souboru nezůstat ani stopa.
    #[test]
    fn nedopsany_balicek_po_chybe_zmizi() {
        struct Rozbity {
            zbyva: u8,
        }

        impl std::io::Read for Rozbity {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.zbyva == 0 {
                    return Err(std::io::Error::other("spojení spadlo"));
                }
                self.zbyva -= 1;
                buf[0] = b'x';
                Ok(1)
            }
        }

        let path = std::env::temp_dir().join(format!(
            "warust-test-nedopsany-{}.zip", std::process::id()
        ));
        assert!(save_body_to_file(Rozbity { zbyva: 4 }, &path).is_err());
        assert!(!path.exists(), "zůstal nedopsaný soubor {}", path.display());
    }
}
