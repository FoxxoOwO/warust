//! Microphone capture for voice notes.
//!
//! cpal delivers raw device samples, but WhatsApp expects OGG/Opus, so each take
//! is written out as 16-bit mono WAV and encoded by the ffmpeg we already ship.
//! The capture stream lives on its own thread: `cpal::Stream` is not `Sync`, so
//! it must never be stored inside the shared application state.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;
use parking_lot::Mutex;

/// A finished take, already in the format WhatsApp accepts.
pub struct Recording {
    pub path: PathBuf,
    pub duration_ms: u64,
    /// 64 amplitude buckets (0..=100) for the waveform preview.
    pub waveform: Vec<u8>,
}

struct Session {
    stop: Sender<()>,
    samples: Arc<Mutex<Vec<f32>>>,
    rate: u32,
    started: Instant,
    worker: Option<std::thread::JoinHandle<()>>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Begin capturing. Returns an error string the UI can show verbatim.
pub fn start() -> Result<(), String> {
    let mut guard = SESSION.lock();
    if guard.is_some() {
        return Err("nahrávání už běží".into());
    }

    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();

    let sink = samples.clone();
    let worker = std::thread::spawn(move || run_capture(sink, stop_rx, ready_tx));

    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(rate)) => {
            *guard = Some(Session {
                stop: stop_tx,
                samples,
                rate,
                started: Instant::now(),
                worker: Some(worker),
            });
            Ok(())
        }
        Ok(Err(message)) => {
            drop(guard);
            let _ = worker.join();
            Err(message)
        }
        Err(_) => {
            let _ = stop_tx.send(());
            drop(guard);
            let _ = worker.join();
            Err("mikrofon neodpovídá".into())
        }
    }
}

/// How long the current take has been running, or 0 when idle.
pub fn elapsed_ms() -> u64 {
    SESSION
        .lock()
        .as_ref()
        .map(|s| s.started.elapsed().as_millis() as u64)
        .unwrap_or(0)
}

pub fn is_recording() -> bool {
    SESSION.lock().is_some()
}

/// Stop capturing, encode the take and hand back the OGG file.
pub fn finish() -> Result<Recording, String> {
    let session = SESSION.lock().take().ok_or("není co odeslat")?;
    let _ = session.stop.send(());
    if let Some(worker) = session.worker {
        let _ = worker.join();
    }

    let samples = std::mem::take(&mut *session.samples.lock());
    let rate = session.rate.max(1);
    if samples.is_empty() {
        return Err("nahrávka je prázdná".into());
    }
    let duration_ms = samples.len() as u64 * 1000 / rate as u64;
    if duration_ms < 300 {
        return Err("nahrávka je příliš krátká".into());
    }

    let wav = temp_path("wav");
    write_wav(&wav, &samples, rate)?;
    let encoded = encode_opus(&wav);
    let _ = std::fs::remove_file(&wav);
    let path = encoded?;

    Ok(Recording {
        path,
        duration_ms,
        waveform: waveform(&samples),
    })
}

/// Throw the current take away.
pub fn cancel() {
    let Some(session) = SESSION.lock().take() else {
        return;
    };
    let _ = session.stop.send(());
    if let Some(worker) = session.worker {
        let _ = worker.join();
    }
}

fn run_capture(sink: Arc<Mutex<Vec<f32>>>, stop: Receiver<()>, ready: Sender<Result<u32, String>>) {
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        let _ = ready.send(Err("není dostupný žádný mikrofon".into()));
        return;
    };
    let supported = match device.default_input_config() {
        Ok(config) => config,
        Err(err) => {
            let _ = ready.send(Err(format!("mikrofon: {err}")));
            return;
        }
    };
    let rate = supported.sample_rate();
    let config: cpal::StreamConfig = supported.into();

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, sink),
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, sink),
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, sink),
        other => Err(format!("nepodporovaný formát mikrofonu: {other:?}")),
    };
    let stream = match stream {
        Ok(stream) => stream,
        Err(message) => {
            let _ = ready.send(Err(message));
            return;
        }
    };
    if let Err(err) = stream.play() {
        let _ = ready.send(Err(format!("mikrofon nelze spustit: {err}")));
        return;
    }

    let _ = ready.send(Ok(rate));
    // Park here until the UI stops the take; the stream must stay alive.
    let _ = stop.recv_timeout(Duration::from_secs(600));
    drop(stream);
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sink: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + Sample,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    device
        .build_input_stream(
            config.clone(),
            move |data: &[T], _| {
                let mut out = sink.lock();
                for frame in data.chunks(channels) {
                    let sum: f32 = frame.iter().map(|s| f32::from_sample(*s)).sum();
                    out.push(sum / channels as f32);
                }
            },
            |err| tracing::warn!("capture stream error: {err}"),
            None,
        )
        .map_err(|err| format!("mikrofon: {err}"))
}

fn temp_path(ext: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("warust-voice");
    let _ = std::fs::create_dir_all(&dir);
    let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    dir.join(format!("{stamp}-{serial}.{ext}"))
}

fn write_wav(path: &Path, samples: &[f32], rate: u32) -> Result<(), String> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(path, out).map_err(|err| format!("nahrávku nelze uložit: {err}"))
}

fn encode_opus(wav: &Path) -> Result<PathBuf, String> {
    let out = temp_path("ogg");
    let result = std::process::Command::new(crate::audio::ffmpeg_bin())
        .arg("-y")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(wav)
        .args(["-c:a", "libopus", "-b:a", "32k", "-ar", "48000", "-ac", "1"])
        .arg(&out)
        .output()
        .map_err(|err| format!("ffmpeg se nepodařilo spustit: {err}"))?;
    if !result.status.success() {
        return Err(format!(
            "ffmpeg: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    if !out.exists() {
        return Err("ffmpeg nevyrobil soubor".into());
    }
    Ok(out)
}

/// 64 peak buckets, each 0..=100 — that is what WhatsApp expects in the preview.
pub fn waveform(samples: &[f32]) -> Vec<u8> {
    const BUCKETS: usize = 64;
    if samples.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(BUCKETS);
    for index in 0..BUCKETS {
        let start = index * samples.len() / BUCKETS;
        let end = ((index + 1) * samples.len() / BUCKETS).max(start + 1).min(samples.len());
        let peak = samples[start..end]
            .iter()
            .fold(0.0f32, |acc, s| acc.max(s.abs()));
        out.push((peak * 100.0).round().clamp(0.0, 100.0) as u8);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveform_has_one_bucket_per_slot_and_follows_peaks() {
        // Quiet first half, loud second half: the buckets must mirror that.
        let mut samples = vec![0.0f32; 1280];
        samples.extend(std::iter::repeat(0.5f32).take(1280));

        let buckets = waveform(&samples);
        assert_eq!(buckets.len(), 64);
        assert_eq!(buckets[0], 0);
        assert_eq!(buckets[63], 50);
    }

    #[test]
    fn waveform_of_silence_is_empty_safe() {
        assert!(waveform(&[]).is_empty());
        assert!(waveform(&[0.0; 64]).iter().all(|b| *b == 0));
    }

    #[test]
    fn wav_is_pcm16_mono_with_the_expected_header() {
        let path = temp_path("wav");
        write_wav(&path, &[0.0, 1.0, -1.0], 16_000).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 1); // PCM
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1); // mono
        assert_eq!(u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]), 16_000);
        assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 16); // bits per sample
        assert_eq!(bytes.len(), 44 + 6);
        // Samples are clamped, so full scale does not wrap around.
        assert_eq!(i16::from_le_bytes([bytes[46], bytes[47]]), i16::MAX);
    }
}
