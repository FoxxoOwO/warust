//! Animation utilities and interactive visual effects for Warust

use std::time::Duration;
use eframe::egui::{self, Color32, FontId, Pos2, Response, RichText, Sense, vec2};

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

/// Paint 3 bouncing dots directly with a painter
pub fn bouncing_dots_painter(painter: &egui::Painter, origin: Pos2, time: f64, color: Color32, dot_radius: f32, spacing: f32) {
    for i in 0..3 {
        let phase = time * 6.0 - (i as f64) * 0.9;
        let bounce = phase.sin().max(0.0) as f32 * (dot_radius * 1.5);
        let center_x = origin.x + dot_radius + (i as f32) * (dot_radius * 2.0 + spacing);
        let center_y = origin.y + dot_radius * 2.0 - bounce;
        
        let alpha = (160.0 + (bounce / (dot_radius * 1.5)) * 95.0) as u8;
        let dot_color = Color32::from_rgba_premultiplied(
            (color.r() as f32 * (alpha as f32 / 255.0)) as u8,
            (color.g() as f32 * (alpha as f32 / 255.0)) as u8,
            (color.b() as f32 * (alpha as f32 / 255.0)) as u8,
            alpha,
        );
        painter.circle_filled(Pos2::new(center_x, center_y), dot_radius, dot_color);
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
        let radius = base_radius + (max_radius - base_radius) * progress;
        let alpha = ((1.0 - progress) * 200.0) as u8;
        let ring_color = Color32::from_rgba_premultiplied(
            (color.r() as f32 * (alpha as f32 / 255.0)) as u8,
            (color.g() as f32 * (alpha as f32 / 255.0)) as u8,
            (color.b() as f32 * (alpha as f32 / 255.0)) as u8,
            alpha,
        );
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
