//! Animation utilities and interactive visual effects for Warust
//!
//! egui is immediate mode, so every motion here is derived from state that is
//! recomputed on each frame: a boolean handed to
//! [`egui::Context::animate_bool_with_time_and_easing`], or a timestamp kept in
//! the context's temp data. Nothing stores a curve of its own, and nothing keeps
//! the app repainting once it has settled — a visible animation asks for the
//! next frame, a finished one goes quiet.

use std::collections::HashMap;
use std::time::Duration;

use eframe::egui::{self, Color32, Context, FontId, Id, Pos2, Response, RichText, Sense, vec2};

/// Hover, press and tick feedback. Short enough to feel like a direct response.
pub const FAST: f32 = 0.10;
/// Entrances: a chat pane, a bubble that just arrived, the jump flash.
pub const MEDIUM: f32 = 0.18;
/// Linear interpolation between two Color32 values
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
        (a.a() as f32 + (b.a() as f32 - a.a() as f32) * t) as u8,
    )
}

/// Fade a colour by scaling its opacity only.
///
/// The rings and dots used to multiply rgb *and* alpha before storing the result
/// premultiplied, so a fading accent went dark as well as transparent. This
/// keeps the hue and takes the alpha down, which is what a fade has to look like
/// on a dark background.
pub fn fade_color(color: Color32, t: f32) -> Color32 {
    let a = (color.a() as f32 * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
}

/// Cubic ease-out: leaves immediately, arrives softly. Used for every entrance.
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}

/// Smoothed 0..1 for a hover or selection state. egui schedules the repaints.
pub fn hover_t(ctx: &Context, id: Id, on: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, FAST, ease_out_cubic)
}

/// Eased 0..1 progress for something that appeared on its first frame — a pane
/// swapped in, a popup opened. Re-arms itself when `id` is forgotten.
pub fn entrance(ctx: &Context, id: Id, duration: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let first = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with::<f64>(id, || now));
    let t = ((now - first) as f32 / duration.max(0.001)).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    ease_out_cubic(t)
}

/// Ids that have already been on screen, so entrances play exactly once.
#[derive(Default, Clone)]
struct ArrivalState {
    /// id -> (first frame it was drawn, stagger ordinal)
    seen: HashMap<String, (f64, u32)>,
}

/// Stamp the ids of one list so [`arrival_alpha`] knows what is new.
///
/// Only the last `window` ids are tracked: incoming messages land at the end, so
/// that is where an entrance matters. Scrolling up does not replay anything,
/// while a chat whose history arrives all at once fades that history in.
pub fn track_arrivals<'a>(
    ctx: &Context,
    id: Id,
    ids: impl Iterator<Item = &'a str>,
    window: usize,
) {
    let now = ctx.input(|i| i.time);
    let all: Vec<&str> = ids.collect();
    let start = all.len().saturating_sub(window);
    let tail = &all[start..];

    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<ArrivalState>(id);
        // Forget what scrolled out of the window, so coming back animates again
        // instead of staying invisible.
        state.seen.retain(|known, _| tail.contains(&known.as_str()));
        let last = tail.len().saturating_sub(1);
        for (rank, msg_id) in tail.iter().enumerate() {
            // The bottom-most message is the newest one: it goes last.
            let ordinal = (last - rank) as u32;
            state
                .seen
                .entry((*msg_id).to_owned())
                .or_insert((now, ordinal));
        }
    });
}

/// Eased fade-in for one tracked id. 1.0 means "fully there, nothing to do";
/// the newest messages start a few milliseconds after the one above them.
pub fn arrival_alpha(ctx: &Context, id: Id, msg_id: &str, duration: f32, stagger: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let stamped = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<ArrivalState>(id)
            .seen
            .get(msg_id)
            .copied()
    });
    let Some((first, ordinal)) = stamped else {
        return 1.0;
    };
    let delay = ordinal as f64 * stagger as f64;
    let t = ((now - first - delay) as f32 / duration.max(0.001)).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    ease_out_cubic(t)
}

/// Paint 3 bouncing dots directly with a painter
pub fn bouncing_dots_painter(painter: &egui::Painter, origin: Pos2, time: f64, color: Color32, dot_radius: f32, spacing: f32) {
    for i in 0..3 {
        let phase = time * 6.0 - (i as f64) * 0.9;
        let bounce = phase.sin().max(0.0) as f32 * (dot_radius * 1.5);
        let center_x = origin.x + dot_radius + (i as f32) * (dot_radius * 2.0 + spacing);
        let center_y = origin.y + dot_radius * 2.0 - bounce;

        // Dim only while the dot sits at the bottom of its hop.
        let lift = bounce / (dot_radius * 1.5);
        let color = fade_color(color, 0.62 + lift * 0.38);
        painter.circle_filled(Pos2::new(center_x, center_y), dot_radius, color);
    }
}

/// 3 Bouncing dots animation (WhatsApp typing indicator)
pub fn bouncing_dots(ui: &mut egui::Ui, color: Color32, dot_radius: f32, spacing: f32) -> Response {
    let total_width = dot_radius * 6.0 + spacing * 2.0;
    let total_height = dot_radius * 4.0;
    let (rect, response) = ui.allocate_exact_size(vec2(total_width, total_height), Sense::hover());

    if ui.is_rect_visible(rect) {
        let time = ui.ctx().input(|i| i.time);
        bouncing_dots_painter(ui.painter(), rect.min, time, color, dot_radius, spacing);
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }

    response
}

/// Typing indicator row with text and animated bouncing dots
pub fn typing_indicator(ui: &mut egui::Ui, name_opt: Option<&str>) {
    ui.horizontal(|ui| {
        let label_text = if let Some(name) = name_opt {
            format!("{} píše", name)
        } else {
            "píše".to_string()
        };
        ui.label(
            RichText::new(label_text)
                .font(FontId::proportional(12.0))
                .color(Color32::from_rgb(0x00, 0xA8, 0x84)),
        );
        ui.add_space(2.0);
        bouncing_dots(ui, Color32::from_rgb(0x00, 0xA8, 0x84), 2.5, 3.0);
    });
}

/// Expanding radar ripple circles for calls and ringing
pub fn pulse_radar(painter: &egui::Painter, center: Pos2, base_radius: f32, max_radius: f32, color: Color32, time: f64) {
    let waves = 3;
    for i in 0..waves {
        let offset = (i as f64) / (waves as f64);
        let progress = ((time * 0.8 + offset) % 1.0) as f32;
        // Eased outwards: launches off the avatar and slows down as it fades.
        let radius = base_radius + (max_radius - base_radius) * ease_out_cubic(progress);
        let alpha = (1.0 - progress) * 200.0;
        let ring_color = fade_color(color, alpha / 255.0);
        painter.circle_stroke(center, radius, egui::Stroke::new(1.8, ring_color));
    }
}

/// Sending message pulsing clock indicator
pub fn sending_indicator(_ui: &egui::Ui, time: f64) -> Color32 {
    let pulse = ((time * 4.0).sin() * 0.35 + 0.65) as f32;
    Color32::from_rgba_premultiplied(
        (255.0 * pulse) as u8,
        (255.0 * pulse) as u8,
        (255.0 * pulse) as u8,
        (220.0 * pulse) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_is_monotonic_and_clamped() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert_eq!(ease_out_cubic(-3.0), 0.0);
        assert_eq!(ease_out_cubic(9.0), 1.0);
        let mut last = 0.0;
        for step in 0..=20 {
            let v = ease_out_cubic(step as f32 / 20.0);
            assert!(v >= last, "ease-out went backwards at {step}");
            last = v;
        }
    }

    /// Fading drops opacity and keeps the hue the accent always had.
    #[test]
    fn fade_scales_alpha_and_keeps_the_hue() {
        let accent = Color32::from_rgb(0x00, 0xA8, 0x84);
        let half = fade_color(accent, 0.5);
        assert_eq!(half.a(), 128);
        // Color32 stores premultiplied, so half the opacity is half the channels.
        assert!((half.g() as i32 - 84).abs() <= 2, "g={}", half.g());
        assert!((half.b() as i32 - 66).abs() <= 2, "b={}", half.b());
        assert_eq!(fade_color(accent, 1.0), accent);
        assert_eq!(fade_color(accent, 0.0).a(), 0);
        assert_eq!(fade_color(accent, 2.0).a(), 255);
    }

    /// A tracked id fades in once and is never re-stamped while it stays put.
    #[test]
    fn arrivals_fade_in_once() {
        let ctx = egui::Context::default();
        let id = Id::unique("test_arrivals");
        let raw = ["a", "b"];
        track_arrivals(&ctx, id, raw.iter().copied(), 8);
        let first = arrival_alpha(&ctx, id, "a", 0.18, 0.0);
        // Unknown ids are treated as already settled, so old rows are untouched.
        assert_eq!(arrival_alpha(&ctx, id, "zzz", 0.18, 0.0), 1.0);

        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.label("advance a frame");
        });
        output.textures_delta.clear();

        track_arrivals(&ctx, id, raw.iter().copied(), 8);
        // Same ids again: the clock keeps running instead of restarting at zero.
        assert!(arrival_alpha(&ctx, id, "a", 0.18, 0.0) >= first);
    }
}
