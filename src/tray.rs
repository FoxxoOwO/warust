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

        let icon = IconSource::Resource("app-icon");
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
