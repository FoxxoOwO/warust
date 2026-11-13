use std::sync::mpsc::{channel, Receiver};
#[cfg(target_os = "windows")]
use tray_item::{IconSource, TrayItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMessage {
    ShowWindow,
    QuitApp,
}

pub struct SystemTray {
    #[cfg(target_os = "windows")]
    _tray: TrayItem,
    pub rx: Receiver<TrayMessage>,
}

impl SystemTray {
    #[cfg(target_os = "windows")]
    pub fn new() -> Result<Self, tray_item::TIError> {
        let (tx, rx) = channel::<TrayMessage>();

        // `build.rs` embeds assets/app-icon.ico through `winres::set_icon`, which
        // writes it into the .rc as `1 ICON "..."` - a numeric resource ID, not a
        // name. LoadImageW only resolves numeric IDs when they are written as
        // "#<id>", otherwise it reports ERROR_RESOURCE_NAME_NOT_FOUND (1812).
        let icon = IconSource::Resource("#1");
        let mut tray = TrayItem::new("Warust", icon)?;

        let tx_show = tx.clone();
        let _ = tray.add_menu_item("Otevřít Warust", move || {
            let _ = tx_show.send(TrayMessage::ShowWindow);
        });

        // Add separator
        let _ = tray.inner_mut().add_separator();

        let tx_quit = tx;
        let _ = tray.add_menu_item("Ukončit Warust", move || {
            let _ = tx_quit.send(TrayMessage::QuitApp);
        });

        Ok(Self { _tray: tray, rx })
    }

    #[cfg(not(target_os = "windows"))]
    pub fn new() -> Result<Self, anyhow::Error> {
        let (_tx, rx) = channel::<TrayMessage>();
        Ok(Self { rx })
    }
}
