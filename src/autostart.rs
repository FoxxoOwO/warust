const REG_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const REG_APP_NAME: &str = "Warust";

/// Check if Warust is configured to start with Windows.
pub fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        // Query HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v Warust
        let output = Command::new("reg")
            .args(["query", &format!(r"HKCU\{}", REG_KEY_PATH), "/v", REG_APP_NAME])
            .output();

        if let Ok(out) = output {
            out.status.success()
        } else {
            false
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Enable or disable starting Warust with Windows.
pub fn set_autostart_enabled(enabled: bool) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        if enabled {
            let current_exe = std::env::current_exe()?;
            let exe_str = current_exe.to_string_lossy().to_string();
            // Run with --tray flag so it starts in background/tray
            let reg_value = format!("\"{}\" --tray", exe_str);

            let status = Command::new("reg")
                .args([
                    "add",
                    &format!(r"HKCU\{}", REG_KEY_PATH),
                    "/v",
                    REG_APP_NAME,
                    "/t",
                    "REG_SZ",
                    "/d",
                    &reg_value,
                    "/f",
                ])
                .status()?;

            if !status.success() {
                anyhow::bail!("Failed to add registry entry for autostart");
            }
        } else {
            // Delete the registry value if it exists
            let status = Command::new("reg")
                .args([
                    "delete",
                    &format!(r"HKCU\{}", REG_KEY_PATH),
                    "/v",
                    REG_APP_NAME,
                    "/f",
                ])
                .status()?;

            // Error 1 from reg delete simply means it didn't exist, which is fine
            let _ = status;
        }

        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Ok(())
    }
}
