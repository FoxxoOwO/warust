//! Profile picture cache: downloaded JPEGs live in `%APPDATA%/warust/avatars`,
//! decoded textures are cached per egui context.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use parking_lot::Mutex;
use whatsapp_rust::wacore_binary::Jid;
use whatsapp_rust::Client;

use crate::state::SharedState;

/// Bumped whenever a new picture is written so UI textures get reloaded.
static AVATAR_REV: AtomicU64 = AtomicU64::new(1);
const REFRESH_AFTER: Duration = Duration::from_secs(24 * 3600);

fn dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("warust")
        .join("avatars")
}

fn key(jid: &Jid) -> String {
    jid.to_string()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn image_path(jid: &Jid) -> PathBuf {
    dir().join(format!("{}.jpg", key(jid)))
}

fn none_path(jid: &Jid) -> PathBuf {
    dir().join(format!("{}.none", key(jid)))
}

fn is_fresh(path: &PathBuf) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map_or(false, |age| age < REFRESH_AFTER)
}

/// Download (if missing/stale or `force`) the profile picture of one chat.
pub async fn fetch_avatar(client: &Arc<Client>, jid: &Jid, force: bool) {
    let img = image_path(jid);
    let none = none_path(jid);
    if !force && (is_fresh(&img) || is_fresh(&none)) {
        return;
    }
    let _ = std::fs::create_dir_all(dir());
    match client.contacts().get_profile_picture(jid, true).await {
        Ok(Some(pic)) => {
            let url = pic.url.clone();
            let bytes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
                let mut resp = ureq::get(&url).call()?;
                Ok(resp.body_mut().read_to_vec()?)
            })
            .await;
            match bytes {
                Ok(Ok(b)) if !b.is_empty() => {
                    if std::fs::write(&img, b).is_ok() {
                        let _ = std::fs::remove_file(&none);
                        AVATAR_REV.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Ok(Err(e)) => tracing::debug!("Avatar download failed for {}: {}", jid, e),
                _ => {}
            }
        }
        Ok(None) => {
            let _ = std::fs::write(&none, b"");
        }
        Err(e) => tracing::debug!("Profile picture query failed for {}: {}", jid, e),
    }
}

/// Sync pictures of all chats, one at a time to stay polite to the server.
pub async fn sync_all_avatars(client: &Arc<Client>, state: &SharedState, force: bool) {
    let jids: Vec<Jid> = {
        let s = state.read();
        let mut chats: Vec<_> = s.chats.values().collect();
        chats.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        chats.into_iter().map(|c| c.jid.clone()).collect()
    };
    tracing::info!("Syncing profile pictures for {} chats", jids.len());
    for jid in jids {
        fetch_avatar(client, &jid, force).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    tracing::info!("Profile picture sync finished");
}

type Cache = Arc<Mutex<HashMap<Jid, (u64, Option<egui::TextureHandle>)>>>;

/// Returns the decoded texture for a chat's picture, if one is cached on disk.
pub fn texture(ctx: &egui::Context, jid: &Jid) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("warust_avatar_cache");
    let cache: Cache = ctx.data_mut(|d| d.get_temp::<Cache>(id)).unwrap_or_else(|| {
        let c: Cache = Arc::new(Mutex::new(HashMap::new()));
        ctx.data_mut(|d| d.insert_temp(id, c.clone()));
        c
    });
    let rev = AVATAR_REV.load(Ordering::Relaxed);
    let mut map = cache.lock();
    if let Some((r, tex)) = map.get(jid) {
        if *r == rev {
            return tex.clone();
        }
    }
    let tex = std::fs::read(image_path(jid))
        .ok()
        .and_then(|b| image::load_from_memory(&b).ok())
        .map(|img| {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let ci = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            ctx.load_texture(format!("avatar-{}", key(jid)), ci, egui::TextureOptions::LINEAR)
        });
    map.insert(jid.clone(), (rev, tex.clone()));
    tex
}

/// Paint a circular picture into `rect`.
pub fn paint_circle(ui: &egui::Ui, tex: &egui::TextureHandle, rect: egui::Rect) {
    egui::Image::new(egui::load::SizedTexture::new(tex.id(), rect.size()))
        .corner_radius(rect.width() / 2.0)
        .paint_at(ui, rect);
}
