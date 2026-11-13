//! Media handling: local storage, downloading, uploading, file picking, and texture caching.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

use crate::state::{MediaType, Message};

pub static MEDIA_REV: AtomicU64 = AtomicU64::new(1);

/// Directory for local media storage: `%APPDATA%/warust/media`
pub fn media_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("warust")
        .join("media")
}

/// Remove characters that are illegal in file names
pub fn clean_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect()
}

/// Compute safe file name for a media message
pub fn compute_file_name(
    msg_id: &str,
    caption_or_name: Option<&str>,
    mimetype: Option<&str>,
    media_type: MediaType,
) -> String {
    let clean_id = clean_filename(msg_id);
    
    if media_type == MediaType::Document {
        if let Some(name) = caption_or_name {
            let clean = clean_filename(name);
            if clean.contains('.') {
                let prefix: String = clean_id.chars().take(8).collect();
                return format!("{}_{}", prefix, clean);
            }
        }
    }

    let ext = match mimetype {
        Some("image/png") => "png",
        Some("image/webp") => "webp",
        Some("image/gif") => "gif",
        Some("video/mp4") => "mp4",
        Some("audio/ogg; codecs=opus") | Some("audio/ogg") => "ogg",
        Some("audio/mpeg") | Some("audio/mp3") => "mp3",
        Some("application/pdf") => "pdf",
        Some("application/zip") => "zip",
        _ => match media_type {
            MediaType::Image => "jpg",
            MediaType::Video => "mp4",
            MediaType::Audio | MediaType::Voice => "ogg",
            MediaType::Document => "bin",
            MediaType::Sticker => "webp",
        },
    };

    format!("{}.{}", clean_id, ext)
}

/// Returns the full local path for storing a message's media file
pub fn get_local_path(
    msg_id: &str,
    caption_or_name: Option<&str>,
    mimetype: Option<&str>,
    media_type: MediaType,
) -> PathBuf {
    media_dir().join(compute_file_name(msg_id, caption_or_name, mimetype, media_type))
}

/// Detect media type from file extension
pub fn detect_media_type(path: &Path) -> MediaType {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" => MediaType::Image,
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" => MediaType::Video,
        "ogg" | "opus" => MediaType::Voice,
        "mp3" | "wav" | "m4a" | "aac" | "flac" => MediaType::Audio,
        _ => MediaType::Document,
    }
}

/// Format file size for human display
pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Surface an audio playback failure (missing codec, no output device, ...).
pub fn report_playback_error(ctx: &egui::Context, message: String) {
    crate::audio::set_error(message);
    ctx.request_repaint();
}

/// Open a media file in the default system viewer
pub fn open_file(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg(path)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(path)
            .spawn();
    }
}

/// Reveal the media file in the file explorer (highlighted)
pub fn show_in_folder(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(not(windows))]
    {
        if let Some(parent) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
        }
    }
}

/// Open native file picker to choose a media file or document
pub fn pick_file() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        #[repr(C)]
        struct OpenFileNameW {
            l_struct_size: u32,
            hwnd_owner: *mut std::ffi::c_void,
            h_instance: *mut std::ffi::c_void,
            lpstr_filter: *const u16,
            lpstr_custom_filter: *mut u16,
            n_max_cust_filter: u32,
            n_filter_index: u32,
            lpstr_file: *mut u16,
            n_max_file: u32,
            lpstr_file_title: *mut u16,
            n_max_file_title: u32,
            lpstr_initial_dir: *const u16,
            lpstr_title: *const u16,
            flags: u32,
            n_file_offset: u16,
            n_file_extension: u16,
            lpstr_def_ext: *const u16,
            l_cust_data: isize,
            lpfn_hook: *mut std::ffi::c_void,
            lp_template_name: *const u16,
            pv_reserved: *mut std::ffi::c_void,
            dw_reserved: u32,
            flags_ex: u32,
        }

        #[link(name = "comdlg32")]
        extern "system" {
            fn GetOpenFileNameW(lpofn: *mut OpenFileNameW) -> i32;
        }

        let mut buffer = [0u16; 1024];
        let filter: Vec<u16> = "All Files (*.*)\0*.*\0Images (*.jpg;*.jpeg;*.png;*.webp;*.gif)\0*.jpg;*.jpeg;*.png;*.webp;*.gif\0Videos (*.mp4;*.mkv;*.mov)\0*.mp4;*.mkv;*.mov\0Audio (*.mp3;*.ogg;*.wav;*.m4a)\0*.mp3;*.ogg;*.wav;*.m4a\0Documents (*.pdf;*.docx;*.xlsx;*.txt;*.zip)\0*.pdf;*.docx;*.xlsx;*.txt;*.zip\0\0"
            .encode_utf16()
            .collect();
        let title: Vec<u16> = "Vyberte soubor k odeslání\0".encode_utf16().collect();

        let mut ofn: OpenFileNameW = unsafe { std::mem::zeroed() };
        ofn.l_struct_size = std::mem::size_of::<OpenFileNameW>() as u32;
        ofn.lpstr_file = buffer.as_mut_ptr();
        ofn.n_max_file = buffer.len() as u32;
        ofn.lpstr_filter = filter.as_ptr();
        ofn.n_filter_index = 1;
        ofn.lpstr_title = title.as_ptr();
        ofn.flags = 0x00080000 | 0x00001000 | 0x00000800 | 0x00000004;

        let success = unsafe { GetOpenFileNameW(&mut ofn) };
        if success != 0 {
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            let os_str = OsString::from_wide(&buffer[..len]);
            Some(PathBuf::from(os_str))
        } else {
            None
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

type MediaCache = Arc<Mutex<HashMap<String, (u64, Option<(egui::TextureHandle, [f32; 2])>)>>>;

/// Retrieves or loads an image/thumbnail texture for a message.
/// Returns the texture handle along with original dimensions [width, height].
pub fn image_texture(
    ctx: &egui::Context,
    msg: &Message,
) -> Option<(egui::TextureHandle, [f32; 2])> {
    let id = egui::Id::new("warust_media_texture_cache");
    let cache: MediaCache = ctx.data_mut(|d| d.get_temp::<MediaCache>(id)).unwrap_or_else(|| {
        let c: MediaCache = Arc::new(Mutex::new(HashMap::new()));
        ctx.data_mut(|d| d.insert_temp(id, c.clone()));
        c
    });

    let rev = MEDIA_REV.load(Ordering::Relaxed);
    let mut map = cache.lock();
    if let Some((r, cached)) = map.get(&msg.id) {
        if *r == rev {
            return cached.clone();
        }
    }

    // Try full downloaded file first
    let loaded = if let Some(path_str) = &msg.media_path {
        let p = PathBuf::from(path_str);
        if p.exists() {
            std::fs::read(&p)
                .ok()
                .and_then(|b| image::load_from_memory(&b).ok())
        } else {
            None
        }
    } else {
        None
    };

    // Fall back to JPEG thumbnail
    let loaded = loaded.or_else(|| {
        msg.media_jpeg_thumbnail
            .as_ref()
            .and_then(|thumb| image::load_from_memory(thumb).ok())
    });

    let result = loaded.map(|img| {
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let original_dim = [rgba.width() as f32, rgba.height() as f32];
        let color_img = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let tex = ctx.load_texture(
            format!("media-{}", clean_filename(&msg.id)),
            color_img,
            egui::TextureOptions::LINEAR,
        );
        (tex, original_dim)
    });

    map.insert(msg.id.clone(), (rev, result.clone()));
    result
}
