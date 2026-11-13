//! Desktop notifications and the notification sound.
//!
//! Two independent channels, because they are configured separately and either
//! can be unavailable: an OS toast (via `notify-rust`) and a short two-tone
//! chime synthesized with rodio, so there is no asset to ship.
//!
//! Notifications are suppressed when the window is focused - WhatsApp behaves
//! the same way, and it is the difference between a usable client and one that
//! toasts about messages you are already looking at.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tracing::{debug, info, warn};

/// Set from the UI thread every frame.
static WINDOW_FOCUSED: AtomicBool = AtomicBool::new(true);

/// Minimum gap between two toasts, so a burst of messages does not stack up.
const MIN_GAP: Duration = Duration::from_secs(2);

/// Only the first toast failure is reported per process lifetime; a platform
/// without a notification daemon should not spam the log on every message.
static FAILURE_REPORTED: AtomicBool = AtomicBool::new(false);

/// Burst suppression. A struct rather than a bare `static Option` so the
/// decision can be tested without sharing process-global state.
struct RateLimiter {
    last: Option<Instant>,
}

impl RateLimiter {
    const fn new() -> Self {
        Self { last: None }
    }

    fn allow(&mut self, now: Instant) -> bool {
        match self.last {
            Some(previous) if now.duration_since(previous) < MIN_GAP => false,
            _ => {
                self.last = Some(now);
                true
            }
        }
    }
}

static LIMITER: Mutex<RateLimiter> = Mutex::new(RateLimiter::new());

pub fn set_window_focused(focused: bool) {
    WINDOW_FOCUSED.store(focused, Ordering::Relaxed);
}

pub fn window_focused() -> bool {
    WINDOW_FOCUSED.load(Ordering::Relaxed)
}

/// Rate limiter. `now` is passed in so the logic can be tested without sleeping.
fn allow_at(now: Instant) -> bool {
    LIMITER.lock().allow(now)
}

/// Show a toast, subject to rate limiting.
///
/// Returns `true` when a notification was actually handed to the OS.
pub fn notify(title: &str, body: &str, sound: bool) -> bool {
    if window_focused() {
        debug!("skipping notification, window is focused");
        return false;
    }
    if !allow_at(Instant::now()) {
        debug!("skipping notification, rate limited");
        return false;
    }

    if sound {
        play_sound();
    }

    show_toast(title, body)
}

/// Send a toast without any of the focus/rate-limit logic. Used by the
/// "test notification" button in settings.
pub fn show_toast(title: &str, body: &str) -> bool {
    let mut notification = notify_rust::Notification::new();
    notification
        .summary(title)
        .body(body)
        .appname("Warust")
        .timeout(notify_rust::Timeout::Milliseconds(6000));

    #[cfg(target_os = "windows")]
    notification.app_id("Warust");

    match notification.show() {
        Ok(_) => {
            info!("notification shown: {}", title);
            true
        }
        Err(e) => {
            if !FAILURE_REPORTED.swap(true, Ordering::Relaxed) {
                warn!(
                    "desktop notifications unavailable ({}); sound will still play",
                    e
                );
            }
            false
        }
    }
}

/// Short two-tone chime. Synthesized, so it works with no bundled audio files.
/// Does nothing when there is no output device - notifications must not panic
/// on a headless or locked-down machine.
pub fn play_sound() {
    std::thread::spawn(|| {
        if let Err(e) = play_sound_blocking() {
            debug!("notification sound skipped: {}", e);
        }
    });
}

fn play_sound_blocking() -> Result<(), String> {
    use rodio::Source;

    let stream = rodio::DeviceSinkBuilder::open_default_sink()
        .map_err(|e| format!("no audio device: {e}"))?;
    let player = rodio::Player::connect_new(stream.mixer());
    player.set_volume(0.6);

    let note = |freq: f32, millis: u64| {
        rodio::source::SineWave::new(freq)
            .take_duration(Duration::from_millis(millis))
            .amplify(0.25)
            .fade_in(Duration::from_millis(4))
    };
    player.append(note(880.0, 90));
    player.append(note(1174.7, 120));
    player.sleep_until_end();
    Ok(())
}

/// Message body used in toasts: keep it short and on one line.
pub fn preview(text: &str) -> String {
    let collapsed: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.chars().count() > 120 {
        let cut: String = collapsed.chars().take(117).collect();
        format!("{cut}...")
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_allows_first_then_blocks_burst() {
        let mut limiter = RateLimiter::new();
        let now = Instant::now();

        assert!(limiter.allow(now));
        assert!(!limiter.allow(now + Duration::from_millis(500)));
        assert!(!limiter.allow(now + Duration::from_millis(1999)));
        assert!(limiter.allow(now + Duration::from_secs(5)));
    }

    #[test]
    fn focused_window_suppresses_notifications() {
        set_window_focused(true);
        assert!(!notify("Chat", "hello", false));
        // A suppressed call must not consume the rate limiter, otherwise the
        // first real notification after losing focus would be dropped.
        assert!(LIMITER.lock().last.is_none());
    }

    #[test]
    fn preview_collapses_whitespace_and_truncates() {
        assert_eq!(preview("  hello \n world  "), "hello world");

        let long = "a".repeat(400);
        let short = preview(&long);
        assert_eq!(short.chars().count(), 120);
        assert!(short.ends_with("..."));
    }

    #[test]
    fn preview_never_splits_a_multibyte_char() {
        let czech = "ěščřžýáíé".repeat(30);
        let out = preview(&czech);
        assert_eq!(out.chars().count(), 120);
        assert!(out.ends_with("..."));
    }
}
