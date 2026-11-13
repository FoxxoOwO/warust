//! In-app video playback.
//!
//! whatsapp-rust only downloads and decrypts the MP4 - it does no decoding for
//! stored media. Decoding is delegated to an external `ffmpeg`, for the same
//! reason as audio: every native Rust video decoder either needs cmake/GStreamer
//! at build time or is incomplete, while ffmpeg handles anything WhatsApp sends.
//!
//! Frames are streamed from ffmpeg as raw RGBA over a pipe, scaled down to a
//! fixed height, and turned into an egui texture on demand.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::TextureHandle;
use parking_lot::Mutex;
use tracing::{debug, warn};

/// Decode height. Wide enough to look sharp in a chat bubble, small enough to
/// keep per-frame copies cheap on CPU.
const TARGET_HEIGHT: u32 = 480;
const FALLBACK_FPS: f32 = 25.0;

struct Frame {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    serial: u64,
}

struct Inner {
    message_id: Option<String>,
    path: Option<PathBuf>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    frame: Arc<Mutex<Option<Frame>>>,
    fps: f32,
    frames: Arc<AtomicU64>,
    /// Monotonic across decoder restarts so textures are never reused by accident.
    serial: Arc<AtomicU64>,
    /// Seconds of source already skipped by the running decoder (`-ss`).
    start: f32,
    /// Total length, probed once when the clip is opened.
    duration: Option<Duration>,
    /// Frame geometry of the decoded stream, needed to restart the decoder.
    width: u32,
    height: u32,
    texture: Option<TextureHandle>,
    texture_serial: u64,
    audio: Option<rodio::Player>,
    stream: Option<rodio::MixerDeviceSink>,
    temp: Option<PathBuf>,
    decoder_process: Option<std::process::Child>,
}

static STATE: Mutex<Option<Inner>> = Mutex::new(None);

fn lock() -> parking_lot::MutexGuard<'static, Option<Inner>> {
    STATE.lock()
}

/// Probe width/height/fps. Falls back to sane defaults when ffprobe is missing.
fn probe(path: &Path) -> ((u32, u32), f32) {
    let output = Command::new(crate::audio::ffprobe_bin())
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let trimmed = text.trim();
            let mut parts = trimmed.split(',');
            let w: u32 = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            let h: u32 = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            let rate = parts.next().unwrap_or("");
            let fps = rate
                .split('/')
                .collect::<Vec<_>>()
                .as_slice()
                .pipe_num()
                .unwrap_or(FALLBACK_FPS);
            if w > 0 && h > 0 {
                return ((w, h), fps);
            }
        }
    }
    ((640, 480), FALLBACK_FPS)
}

/// Total media length via ffprobe. One subprocess, cached for the clip's lifetime.
fn probe_duration(path: &Path) -> Option<Duration> {
    let ffprobe = crate::audio::ffprobe_bin();
    let out = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let secs: f32 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    if secs.is_finite() && secs > 0.0 {
        Some(Duration::from_secs_f32(secs))
    } else {
        None
    }
}

/// Small helper so `a/b` frame rates parse without a regex dependency.
trait RatioParts {
    fn pipe_num(&self) -> Option<f32>;
}

impl RatioParts for &[&str] {
    fn pipe_num(&self) -> Option<f32> {
        match self {
            [num, den] => {
                let n: f32 = num.trim().parse().ok()?;
                let d: f32 = den.trim().parse().ok()?;
                if d > 0.0 && n > 0.0 {
                    Some((n / d).clamp(1.0, 120.0))
                } else {
                    None
                }
            }
            [num] => num.trim().parse().ok().filter(|v: &f32| *v > 0.0),
            _ => None,
        }
    }
}

fn scaled_size((w, h): (u32, u32)) -> (u32, u32) {
    let height = TARGET_HEIGHT.min(h).max(2);
    let mut width = ((w as f32) * (height as f32) / (h as f32)).round() as u32;
    if width % 2 == 1 {
        width += 1;
    }
    (width.max(2), height)
}

fn temp_audio_path(src: &Path) -> PathBuf {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in src.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let dir = std::env::temp_dir().join("warust-video");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{hash:016x}.wav"))
}

/// Extract the audio track to WAV. Runs off the UI thread; silence is fine.
fn extract_audio(src: &Path, stop: Arc<AtomicBool>) -> Option<PathBuf> {
    let out = temp_audio_path(src);
    let status = Command::new(crate::audio::ffmpeg_bin())
        .args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(src)
        .args(["-vn", "-ac", "2", "-ar", "48000", "-c:a", "pcm_s16le"])
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    if stop.load(Ordering::Relaxed) || !status.success() || !out.exists() {
        let _ = std::fs::remove_file(&out);
        return None;
    }
    Some(out)
}

/// Spawn the decoder at `start` seconds plus the thread that drains its frames.
fn spawn_decoder(inner: &mut Inner, start: f32) -> Result<(), String> {
    if let Some(mut child) = inner.decoder_process.take() {
        let _ = child.kill();
        let _ = child.wait();
    }

    let path = inner.path.clone().ok_or_else(|| "no media path".to_string())?;
    let mut cmd = Command::new(crate::audio::ffmpeg_bin());
    cmd.args(["-v", "error", "-nostdin"]);
    if start > 0.0 {
        // `-ss` before `-i` seeks on the demuxer, which is fast and keyframe-accurate enough here.
        cmd.args(["-ss", &format!("{start:.3}")]);
    }
    cmd.arg("-i")
        .arg(path)
        .args([
            "-an",
            "-vf",
            &format!("scale={}:{}", inner.width, inner.height),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("ffmpeg se nepodařilo spustit: {e}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ffmpeg nevrací data".to_string())?;

    let stop = inner.stop.clone();
    let paused = inner.paused.clone();
    let slot = inner.frame.clone();
    let counter = inner.frames.clone();
    let serial = inner.serial.clone();
    let fps = inner.fps.max(1.0);
    let width = inner.width;
    let height = inner.height;
    let frame_bytes = (width as usize) * (height as usize) * 4;

    std::thread::spawn(move || {
        use std::io::Read;
        let interval = Duration::from_secs_f32(1.0 / fps);
        let mut next = Instant::now();
        let mut buffer = vec![0u8; frame_bytes];

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            while paused.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(15));
            }

            if stdout.read_exact(&mut buffer).is_err() {
                break; // EOF, or the process was killed by a seek/stop
            }

            *slot.lock() = Some(Frame {
                rgba: buffer.clone(),
                width,
                height,
                serial: serial.fetch_add(1, Ordering::Relaxed),
            });
            counter.fetch_add(1, Ordering::Relaxed);

            // Pace to the source fps; when behind (slow device) never sleep.
            next += interval;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else {
                next = now;
            }
        }

        debug!("video worker finished");
    });

    inner.decoder_process = Some(child);
    inner.start = start;
    inner.frames.store(0, Ordering::Relaxed);
    *inner.frame.lock() = None;
    Ok(())
}

/// Start (or restart) playback of `path` for `message_id`.
pub fn open(message_id: &str, path: &Path) -> Result<(), String> {
    if !crate::audio::ffmpeg_available() {
        return Err("ffmpeg není k dispozici (nastav WARUST_FFMPEG nebo nainstaluj ffmpeg)".into());
    }

    stop();

    let (native, fps) = probe(path);
    let (width, height) = scaled_size(native);

    let mut inner = Inner {
        message_id: Some(message_id.to_string()),
        path: Some(path.to_path_buf()),
        stop: Arc::new(AtomicBool::new(false)),
        paused: Arc::new(AtomicBool::new(false)),
        frame: Arc::new(Mutex::new(None)),
        fps,
        frames: Arc::new(AtomicU64::new(0)),
        serial: Arc::new(AtomicU64::new(1)),
        start: 0.0,
        duration: probe_duration(path),
        width,
        height,
        texture: None,
        texture_serial: 0,
        audio: None,
        stream: None,
        temp: None,
        decoder_process: None,
    };

    spawn_decoder(&mut inner, 0.0)?;

    // Audio track in the background so the UI is never blocked by ffmpeg.
    let audio_path = path.to_path_buf();
    let audio_stop = inner.stop.clone();
    let audio_slot: PendingAudio = Arc::new(Mutex::new(None));
    let audio_slot_thread = audio_slot.clone();
    let volume = crate::audio::volume();
    std::thread::spawn(move || {
        let Some(wav) = extract_audio(&audio_path, audio_stop.clone()) else {
            return;
        };
        if audio_stop.load(Ordering::Relaxed) {
            let _ = std::fs::remove_file(&wav);
            return;
        }
        let Ok(stream) = rodio::DeviceSinkBuilder::open_default_sink() else {
            let _ = std::fs::remove_file(&wav);
            return;
        };
        let player = rodio::Player::connect_new(stream.mixer());
        player.set_volume(volume);
        match std::fs::File::open(&wav) {
            Ok(file) => match rodio::Decoder::try_from(file) {
                Ok(decoder) => {
                    player.append(decoder);
                    player.play();
                    *audio_slot_thread.lock() = Some((player, stream, wav));
                }
                Err(e) => {
                    warn!("video audio decode failed: {e}");
                    let _ = std::fs::remove_file(&wav);
                }
            },
            Err(_) => {
                let _ = std::fs::remove_file(&wav);
            }
        }
    });

    *lock() = Some(inner);

    // Merge late-arriving audio into the state on the next UI frame.
    *PENDING_AUDIO.lock() = Some(audio_slot);

    Ok(())
}

/// A finished background audio track waiting to be adopted by the player state.
type PendingAudio = Arc<Mutex<Option<(rodio::Player, rodio::MixerDeviceSink, PathBuf)>>>;

/// Audio extraction finishes after `open` returns, so the result is parked here
/// until the next UI frame picks it up.
static PENDING_AUDIO: Mutex<Option<PendingAudio>> = Mutex::new(None);

/// Move a finished background audio track into the live state.
fn adopt_audio(state: &mut Option<Inner>) {
    let taken = PENDING_AUDIO.lock().take();
    if let Some(slot) = taken {
        let ready = slot.lock().take();
        if let (Some(inner), Some((player, stream, wav))) = (state.as_mut(), ready) {
            // A seek may have happened while the track was still being extracted.
            if inner.start > 0.0 {
                let _ = player.try_seek(Duration::from_secs_f32(inner.start));
            }
            if inner.paused.load(Ordering::Relaxed) {
                player.pause();
            }
            inner.audio = Some(player);
            inner.stream = Some(stream);
            inner.temp = Some(wav);
        }
    }
}

pub fn stop() {
    let mut guard = lock();
    if let Some(inner) = guard.as_mut() {
        inner.stop.store(true, Ordering::Relaxed);
        if let Some(player) = inner.audio.take() {
            player.stop();
        }
        inner.stream = None;
        if let Some(mut child) = inner.decoder_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(temp) = inner.temp.take() {
            let _ = std::fs::remove_file(temp);
        }
    }
    *guard = None;
    PENDING_AUDIO.lock().take();
}

/// Message id currently open in the player, if any.
pub fn current_message_id() -> Option<String> {
    lock().as_ref().and_then(|i| i.message_id.clone())
}

pub fn is_open_for(message_id: &str) -> bool {
    current_message_id().as_deref() == Some(message_id)
}

pub fn toggle(message_id: &str, path: &Path) -> Result<(), String> {
    if is_open_for(message_id) {
        if is_paused() {
            // Pressing play on a finished clip rewinds instead of doing nothing.
            if let Some(total) = duration() {
                if position() + Duration::from_millis(250) >= total {
                    seek(Duration::ZERO);
                }
            }
            resume();
            return Ok(());
        }
        pause();
        return Ok(());
    }
    open(message_id, path)
}

pub fn pause() {
    let mut guard = lock();
    if let Some(inner) = guard.as_mut() {
        inner.paused.store(true, Ordering::Relaxed);
        if let Some(player) = inner.audio.as_ref() {
            player.pause();
        }
    }
}

pub fn resume() {
    let mut guard = lock();
    if let Some(inner) = guard.as_mut() {
        inner.paused.store(false, Ordering::Relaxed);
        if let Some(player) = inner.audio.as_ref() {
            player.play();
        }
    }
}

pub fn is_paused() -> bool {
    lock().as_ref().is_some_and(|i| i.paused.load(Ordering::Relaxed))
}

/// Current playback position: the seek offset plus frames delivered since then.
pub fn position() -> Duration {
    let guard = lock();
    match guard.as_ref() {
        Some(inner) => {
            let frames = inner.frames.load(Ordering::Relaxed) as f32;
            let pos = inner.start + frames / inner.fps.max(1.0);
            match inner.duration {
                Some(total) => Duration::from_secs_f32(pos.min(total.as_secs_f32())),
                None => Duration::from_secs_f32(pos),
            }
        }
        None => Duration::ZERO,
    }
}

/// Jump to `target`, clamped to the clip. Restarts the ffmpeg decoder at that
/// point and seeks the audio track in place, so scrubbing stays responsive.
pub fn seek(target: Duration) {
    let mut guard = lock();
    let Some(inner) = guard.as_mut() else { return };

    let mut secs = target.as_secs_f32().max(0.0);
    if let Some(total) = inner.duration {
        secs = secs.min(total.as_secs_f32());
    }
    inner.start = secs;
    inner.frames.store(0, Ordering::Relaxed);

    if let Some(player) = inner.audio.as_ref() {
        let _ = player.try_seek(Duration::from_secs_f32(secs));
    }
    if let Err(e) = spawn_decoder(inner, secs) {
        warn!("video seek failed: {e}");
    }
}

/// Draw-ready texture for the current frame of `message_id`.
pub fn texture(ctx: &egui::Context, message_id: &str) -> Option<TextureHandle> {
    let mut guard = lock();
    adopt_audio(&mut guard);
    let inner = guard.as_mut()?;
    if inner.message_id.as_deref() != Some(message_id) {
        return None;
    }

    let pending = inner.frame.lock().take();
    if let Some(frame) = pending {
        if frame.serial != inner.texture_serial || inner.texture.is_none() {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width as usize, frame.height as usize],
                &frame.rgba,
            );
            let name = format!("video-{}-{}", message_id, frame.serial);
            inner.texture = Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR));
            inner.texture_serial = frame.serial;
        }
    }
    inner.texture.clone()
}

pub fn has_frame(message_id: &str) -> bool {
    texture_serial(message_id).is_some()
}

fn texture_serial(message_id: &str) -> Option<u64> {
    let guard = lock();
    let inner = guard.as_ref()?;
    if inner.message_id.as_deref() != Some(message_id) {
        return None;
    }
    Some(inner.texture_serial)
}

pub fn set_volume(volume: f32) {
    if let Some(inner) = lock().as_ref() {
        if let Some(player) = inner.audio.as_ref() {
            player.set_volume(volume);
        }
    }
}

/// Duration of the currently open video, probed once when it was opened.
pub fn duration() -> Option<Duration> {
    lock().as_ref().and_then(|inner| inner.duration)
}

/// Zero-to-one progress, for drawing the timeline without knowing the length.
pub fn progress() -> f32 {
    match duration() {
        Some(total) if total.as_secs_f32() > 0.0 => {
            (position().as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both ffmpeg tests drive the one global player, so they must not overlap.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn scales_to_target_height_keeping_aspect() {
        assert_eq!(scaled_size((1920, 1080)), (854, 480));
        assert_eq!(scaled_size((1080, 1920)), (270, 480));
        // Never upscale, never produce an odd width.
        assert_eq!(scaled_size((320, 240)), (320, 240));
        let (w, _) = scaled_size((641, 481));
        assert_eq!(w % 2, 0);
    }

    /// End-to-end: synthesize a short clip, decode it through ffmpeg, and check
    /// that frames and a texture actually arrive. Skipped without ffmpeg.
    #[test]
    fn decodes_frames_from_generated_clip() {
        let _guard = TEST_LOCK.lock();
        if !crate::audio::ffmpeg_available() {
            eprintln!("ffmpeg missing - skipping");
            return;
        }

        let dir = std::env::temp_dir().join("warust-video-test");
        std::fs::create_dir_all(&dir).unwrap();
        let clip = dir.join("clip.mp4");
        let status = Command::new(crate::audio::ffmpeg_bin())
            .args(["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=size=320x240:rate=15:duration=1")
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=1"])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest"])
            .arg(&clip)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("spawn ffmpeg");
        assert!(status.success(), "ffmpeg could not build the test clip");
        assert!(clip.metadata().unwrap().len() > 1000);

        open("test-message", &clip).expect("open video");

        // Frames are produced by a background thread; give it a moment.
        let ctx = egui::Context::default();
        let mut frames_seen = 0u64;
        let mut got_texture = false;
        for _ in 0..80 {
            if texture(&ctx, "test-message").is_some() {
                got_texture = true;
            }
            frames_seen = frames_seen.max(inner_frames());
            if frames_seen >= 3 && got_texture {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        assert!(frames_seen >= 3, "only {frames_seen} frames decoded");
        assert!(got_texture, "no texture was produced");
        assert!(position() > Duration::ZERO);

        stop();
        assert!(current_message_id().is_none());
        let _ = std::fs::remove_file(&clip);
    }

    /// Seeking restarts the decoder at an offset and reports the new position.
    #[test]
    fn seeks_to_requested_offset() {
        let _guard = TEST_LOCK.lock();
        if !crate::audio::ffmpeg_available() {
            eprintln!("ffmpeg missing - skipping");
            return;
        }

        let dir = std::env::temp_dir().join("warust-video-test");
        std::fs::create_dir_all(&dir).unwrap();
        let clip = dir.join("seek.mp4");
        let status = Command::new(crate::audio::ffmpeg_bin())
            .args(["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=size=320x240:rate=15:duration=2")
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&clip)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("spawn ffmpeg");
        assert!(status.success(), "ffmpeg could not build the test clip");

        open("seek-message", &clip).expect("open video");
        let total = duration().expect("duration probed at open");
        assert!(total.as_secs_f32() >= 1.5, "duration was {total:?}");

        seek(Duration::from_secs_f32(1.4));
        assert!(position() >= Duration::from_millis(1300), "position after seek was {:?}", position());

        // The restarted decoder must keep delivering frames from the new offset.
        let started = std::time::Instant::now();
        let mut advanced = false;
        while started.elapsed() < Duration::from_secs(3) {
            if position() > Duration::from_secs_f32(1.45) {
                advanced = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(advanced, "decoder did not resume after seek");

        // Clamping: seeking past the end must not run away.
        seek(Duration::from_secs(60));
        assert!(position() + Duration::from_millis(50) >= total);

        stop();
        let _ = std::fs::remove_file(&clip);
    }

    fn inner_frames() -> u64 {
        lock()
            .as_ref()
            .map(|i| i.frames.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    #[test]
    fn parses_frame_rates() {
        assert_eq!((&["30000", "1001"][..]).pipe_num().unwrap().round(), 30.0);
        assert_eq!((&["25", "1"][..]).pipe_num(), Some(25.0));
        assert_eq!((&["0", "0"][..]).pipe_num(), None);
        assert_eq!((&["garbage"][..]).pipe_num(), None);
    }
}
