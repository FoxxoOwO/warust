//! Audio playback for voice notes and audio messages.
//!
//! whatsapp-rust only downloads and decrypts media bytes - it does no container
//! demuxing or decoding for stored media, so playback lives here.
//!
//! Decoding strategy:
//!   1. rodio (symphonia) handles mp3/aac(m4a)/wav/flac/ogg-vorbis directly.
//!   2. WhatsApp voice notes are OGG/Opus, which symphonia 0.5 does not decode.
//!      Those (and anything else rodio rejects) are transcoded to WAV with an
//!      external `ffmpeg` binary, which is the only codec-complete option that
//!      does not need cmake at build time.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use parking_lot::Mutex;
use rodio::Source;
use tracing::{debug, info, warn};

struct Inner {
    /// Keeps the cpal output stream alive; dropping it silences playback.
    stream: Option<rodio::MixerDeviceSink>,
    player: Option<rodio::Player>,
    playing_id: Option<String>,
    duration: Option<Duration>,
    /// Temporary WAV produced by ffmpeg, removed when playback is replaced.
    temp: Option<PathBuf>,
    volume: f32,
    error: Option<String>,
}

static STATE: Mutex<Inner> = Mutex::new(Inner {
    stream: None,
    player: None,
    playing_id: None,
    duration: None,
    temp: None,
    volume: 1.0,
    error: None,
});

fn lock() -> parking_lot::MutexGuard<'static, Inner> {
    STATE.lock()
}

/// A binary shipped next to the executable (the release bundles do this).
fn sibling_binary(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    candidate.is_file().then_some(candidate)
}

/// Path to the ffmpeg binary. `WARUST_FFMPEG` wins, then a bundled copy next to
/// the executable, then whatever is on `PATH`.
pub fn ffmpeg_bin() -> PathBuf {
    if let Some(p) = std::env::var_os("WARUST_FFMPEG") {
        return PathBuf::from(p);
    }
    sibling_binary("ffmpeg").unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

/// Path to ffprobe, resolved the same way as [`ffmpeg_bin`].
pub fn ffprobe_bin() -> PathBuf {
    if let Some(p) = std::env::var_os("WARUST_FFPROBE") {
        return PathBuf::from(p);
    }
    sibling_binary("ffprobe").unwrap_or_else(|| PathBuf::from("ffprobe"))
}

/// Whether a usable ffmpeg binary was found (cached after first probe).
pub fn ffmpeg_available() -> bool {
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let path = ffmpeg_bin();
        let ok = Command::new(&path)
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            info!("audio/video backend: ffmpeg at {}", path.display());
        } else {
            warn!(
                "ffmpeg not found at {} - voice notes and video fall back to the system player",
                path.display()
            );
        }
        ok
    })
}

fn temp_wav_path(src: &Path) -> PathBuf {
    // Stable per-source name so a re-download reuses the same transcode.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in src.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let dir = std::env::temp_dir().join("warust-audio");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{hash:016x}.wav"))
}

/// Transcode to PCM WAV so rodio can play codecs it cannot decode itself
/// (notably OGG/Opus voice notes).
fn transcode_to_wav(src: &Path) -> Result<PathBuf, String> {
    if !ffmpeg_available() {
        return Err("ffmpeg není k dispozici (nastav WARUST_FFMPEG nebo nainstaluj ffmpeg)".into());
    }
    let out = temp_wav_path(src);
    let status = Command::new(ffmpeg_bin())
        .args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(src)
        .args([
            "-vn",
            "-ac",
            "2",
            "-ar",
            "48000",
            "-c:a",
            "pcm_s16le",
            "-f",
            "wav",
        ])
        .arg(&out)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("ffmpeg se nepodařilo spustit: {e}"))?;

    if !status.status.success() {
        let msg = String::from_utf8_lossy(&status.stderr);
        let msg = msg.trim();
        let msg = if msg.is_empty() {
            "ffmpeg selhal".to_string()
        } else {
            msg.lines().next().unwrap_or(msg).to_string()
        };
        return Err(format!("dekódování selhalo: {msg}"));
    }
    Ok(out)
}

/// Open a rodio decoder, falling back to an ffmpeg transcode.
fn open_source(path: &Path) -> Result<(rodio::Decoder<std::io::BufReader<std::fs::File>>, Option<PathBuf>), String> {
    match std::fs::File::open(path).map_err(|e| format!("{e}")) {
        Ok(file) => match rodio::Decoder::try_from(file) {
            Ok(dec) => Ok((dec, None)),
            Err(e) => {
                debug!("rodio cannot decode {:?} ({e}), trying ffmpeg", path);
                let wav = transcode_to_wav(path)?;
                let file = std::fs::File::open(&wav).map_err(|e| format!("{e}"))?;
                let dec = rodio::Decoder::try_from(file).map_err(|e| format!("{e}"))?;
                Ok((dec, Some(wav)))
            }
        },
        Err(e) => Err(e),
    }
}

fn ensure_stream(inner: &mut Inner) -> Result<(), String> {
    if inner.stream.is_none() {
        let stream = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("zvukové zařízení není k dispozici: {e}"))?;
        inner.stream = Some(stream);
    }
    Ok(())
}

/// Start playback of `path`, or stop it if this message is already playing.
pub fn toggle(id: &str, path: &Path) -> Result<(), String> {
    if playing_id().as_deref() == Some(id) {
        stop();
        return Ok(());
    }
    play(id, path)
}

pub fn play(id: &str, path: &Path) -> Result<(), String> {
    let result = play_inner(id, path);
    if let Err(e) = &result {
        warn!("playback failed for {:?}: {e}", path);
        lock().error = Some(e.clone());
    }
    result
}

/// Take the last playback error for display (e.g. missing ffmpeg).
pub fn take_error() -> Option<String> {
    lock().error.take()
}

/// Record an error raised outside the play path (e.g. from the UI).
pub fn set_error(message: String) {
    lock().error = Some(message);
}

fn play_inner(id: &str, path: &Path) -> Result<(), String> {
    let mut inner = lock();

    // Always stop whatever is running before switching tracks.
    if let Some(p) = inner.player.take() {
        p.stop();
    }
    inner.playing_id = None;
    inner.duration = None;

    // Drop the previous transcode, if any.
    if let Some(old) = inner.temp.take() {
        let _ = std::fs::remove_file(old);
    }

    ensure_stream(&mut inner)?;

    let (decoder, temp) = open_source(path)?;
    let duration = Source::total_duration(&decoder);

    let player = {
        let mixer = inner
            .stream
            .as_ref()
            .map(|s| s.mixer())
            .ok_or_else(|| "zvukové zařízení není k dispozici".to_string())?;
        rodio::Player::connect_new(mixer)
    };
    player.set_volume(inner.volume);
    player.append(decoder);
    player.play();

    inner.player = Some(player);
    inner.playing_id = Some(id.to_string());
    inner.duration = duration;
    inner.temp = temp;
    Ok(())
}

pub fn stop() {
    let mut inner = lock();
    if let Some(p) = inner.player.take() {
        p.stop();
    }
    inner.playing_id = None;
    inner.duration = None;
    if let Some(old) = inner.temp.take() {
        let _ = std::fs::remove_file(old);
    }
}

pub fn pause() {
    let inner = lock();
    if let Some(p) = inner.player.as_ref() {
        p.pause();
    }
}

pub fn resume() {
    let inner = lock();
    if let Some(p) = inner.player.as_ref() {
        p.play();
    }
}

pub fn is_paused() -> bool {
    lock().player.as_ref().is_some_and(|p| p.is_paused())
}

/// Message id currently playing, if any. Clears itself when playback ends.
pub fn playing_id() -> Option<String> {
    let mut inner = lock();
    if let Some(p) = inner.player.as_ref() {
        if p.empty() && !p.is_paused() {
            // Track finished naturally.
            inner.player = None;
            inner.playing_id = None;
            inner.duration = None;
            if let Some(old) = inner.temp.take() {
                let _ = std::fs::remove_file(old);
            }
            return None;
        }
    }
    inner.playing_id.clone()
}

pub fn is_playing(id: &str) -> bool {
    playing_id().as_deref() == Some(id)
}

pub fn position() -> Duration {
    lock()
        .player
        .as_ref()
        .map(|p| p.get_pos())
        .unwrap_or_default()
}

pub fn duration() -> Option<Duration> {
    lock().duration
}

/// Seek within the current track (used by the progress bar).
pub fn seek(pos: Duration) {
    let inner = lock();
    if let Some(p) = inner.player.as_ref() {
        if let Err(e) = p.try_seek(pos) {
            warn!("seek failed: {e}");
        }
    }
}

pub fn set_volume(volume: f32) {
    let mut inner = lock();
    inner.volume = volume.clamp(0.0, 2.0);
    if let Some(p) = inner.player.as_ref() {
        p.set_volume(inner.volume);
    }
}

pub fn volume() -> f32 {
    lock().volume
}

/// Rendered duration in WhatsApp style, e.g. "0:07".
pub fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    format!("{}:{:02}", total / 60, total % 60)
}

/// Convenience for wave-bar counts.
pub fn is_supported_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    matches!(ext.as_str(), "mp3" | "m4a" | "aac" | "wav" | "flac" | "ogg" | "opus")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0:00");
        assert_eq!(format_duration(Duration::from_secs(7)), "0:07");
        assert_eq!(format_duration(Duration::from_secs(125)), "2:05");
    }

    #[test]
    fn detects_supported_extensions() {
        assert!(is_supported_file(Path::new("a.OGG")));
        assert!(is_supported_file(Path::new("voice.opus")));
        assert!(!is_supported_file(Path::new("photo.jpg")));
        assert!(!is_supported_file(Path::new("noext")));
    }

    /// The whole point of the ffmpeg fallback: WhatsApp voice notes are OGG/Opus,
    /// which symphonia cannot decode. Verify the real binary path end to end.
    #[test]
    fn transcodes_opus_voice_note_and_decodes_it() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not available");
            return;
        }
        let dir = std::env::temp_dir().join("warust-audio-test");
        std::fs::create_dir_all(&dir).unwrap();
        let ogg = dir.join("tone.ogg");
        let made = Command::new(ffmpeg_bin())
            .args(["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg("sine=frequency=440:duration=0.3")
            .args(["-c:a", "libopus", "-b:a", "24k"])
            .arg(&ogg)
            .status()
            .expect("spawn ffmpeg");
        if !made.success() {
            eprintln!("skipping: ffmpeg build lacks libopus");
            return;
        }

        // rodio alone must fail on this file, otherwise the fallback is moot.
        let direct = std::fs::File::open(&ogg).unwrap();
        assert!(
            rodio::Decoder::try_from(direct).is_err(),
            "expected rodio to reject opus"
        );

        let wav = transcode_to_wav(&ogg).expect("transcode");
        let len = std::fs::metadata(&wav).unwrap().len();
        assert!(len > 1_000, "wav looks empty: {len} bytes");

        let (decoder, extra) = open_source(&ogg).expect("open_source should fall back");
        assert_eq!(extra.as_deref(), Some(wav.as_path()));
        assert!(Source::total_duration(&decoder).is_some());
    }

    #[test]
    fn temp_paths_are_stable_per_source() {
        let a = temp_wav_path(Path::new("/tmp/one.ogg"));
        let b = temp_wav_path(Path::new("/tmp/one.ogg"));
        let c = temp_wav_path(Path::new("/tmp/two.ogg"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.extension().unwrap(), "wav");
    }
}
