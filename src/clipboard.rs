//! Clipboard image support: a copied screenshot becomes a chat attachment.

use std::path::PathBuf;

/// Writes the clipboard picture into the media folder as PNG.
///
/// `Ok(None)` means the clipboard simply held no image (text, files, …) — that
/// is not an error, it is what happens when the user pastes text.
pub fn save_clipboard_image() -> Result<Option<PathBuf>, String> {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(err) => return Err(format!("Schránku nelze otevřít: {err}")),
    };

    let image = match clipboard.get_image() {
        Ok(image) => image,
        Err(arboard::Error::ContentNotAvailable) => return Ok(None),
        Err(err) => return Err(format!("Čtení obrázku ze schránky selhalo: {err}")),
    };

    let width = image.width as u32;
    let height = image.height as u32;
    let buffer = image::RgbaImage::from_raw(width, height, image.bytes.into_owned())
        .ok_or_else(|| "Obrázek ze schránky má neplatná data".to_string())?;

    let dir = crate::media::media_dir();
    std::fs::create_dir_all(&dir).map_err(|err| format!("Složku médií nelze vytvořit: {err}"))?;

    let path = dir.join(format!(
        "clipboard-{}.png",
        chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f")
    ));
    buffer
        .save(&path)
        .map_err(|err| format!("Obrázek ze schránky nelze uložit: {err}"))?;
    Ok(Some(path))
}
