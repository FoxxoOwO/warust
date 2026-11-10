//! Theme and visual styling

use std::sync::Arc;
use egui::{Color32, FontData, FontDefinitions, FontFamily, Theme};

/// Setup the Warust theme (WhatsApp-like dark mode with green accent)
pub fn setup_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(Theme::Dark)).clone();

    // WhatsApp dark mode colors
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = Color32::from_rgb(0x11, 0x1B, 0x21);      // Main background
    style.visuals.window_fill = Color32::from_rgb(0x1F, 0x2C, 0x34);     // Window/panel background
    style.visuals.extreme_bg_color = Color32::from_rgb(0x0B, 0x14, 0x1A); // Popovers, tooltips

    // WhatsApp green accent
    let wa_green = Color32::from_rgb(0x00, 0x80, 0x69);
    let wa_green_light = Color32::from_rgb(0x00, 0xA8, 0x84);
    let _wa_green_hover = Color32::from_rgb(0x00, 0x6E, 0x5C);

    style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(0x2A, 0x39, 0x42);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(0x2A, 0x39, 0x42);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x20, 0x2C, 0x33);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x33, 0x47, 0x50);
    style.visuals.widgets.active.bg_fill = wa_green;
    style.visuals.widgets.open.bg_fill = Color32::from_rgb(0x33, 0x47, 0x50);

    style.visuals.selection.bg_fill = wa_green;
    style.visuals.selection.stroke = egui::Stroke::NONE;

    style.visuals.hyperlink_color = wa_green_light;
    style.visuals.warn_fg_color = Color32::from_rgb(0xFF, 0xB3, 0x00);
    style.visuals.error_fg_color = Color32::from_rgb(0xFF, 0x6B, 0x6B);

    // Rounded corners
    style.visuals.window_corner_radius = egui::CornerRadius::same(12);
    style.visuals.menu_corner_radius = egui::CornerRadius::same(8);

    // Spacing
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.menu_margin = egui::Margin::same(8);

    ctx.set_style_of(Theme::Dark, style);

    // Fonts - use system fonts with Noto Sans fallback
    let mut fonts = FontDefinitions::default();

    // Add Noto Sans as primary font (embedded)
    fonts.font_data.insert(
        "noto_sans".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/NotoSans-Regular.ttf")))
    );
    fonts.font_data.insert(
        "noto_sans_bold".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/NotoSans-Bold.ttf")))
    );

    // Configure font families
    fonts.families.get_mut(&FontFamily::Proportional).unwrap()
        .insert(0, "noto_sans".into());
    fonts.families.get_mut(&FontFamily::Monospace).unwrap()
        .insert(0, "noto_sans".into());

    // Also add bold variant
    fonts.families.entry(FontFamily::Name("Bold".into())).or_default()
        .push("noto_sans_bold".into());

    #[cfg(windows)]
    {
        if let Ok(data) = std::fs::read(r"C:\Windows\Fonts\seguisym.ttf") {
            fonts.font_data.insert(
                "segoe_symbol".into(),
                Arc::new(FontData::from_owned(data)),
            );
            fonts.families.get_mut(&FontFamily::Proportional).unwrap().push("segoe_symbol".into());
            fonts.families.get_mut(&FontFamily::Monospace).unwrap().push("segoe_symbol".into());
        }
        if let Ok(data) = std::fs::read(r"C:\Windows\Fonts\seguiemj.ttf") {
            fonts.font_data.insert(
                "segoe_emoji".into(),
                Arc::new(FontData::from_owned(data)),
            );
            fonts.families.get_mut(&FontFamily::Proportional).unwrap().push("segoe_emoji".into());
            fonts.families.get_mut(&FontFamily::Monospace).unwrap().push("segoe_emoji".into());
        }
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_theme_fonts() {
        let ctx = egui::Context::default();
        setup_theme(&ctx);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.label("⚙ ➕ 📱 🔒 📹 📞 ⋮ 📌 🔕 ● ✓ ⟳ ⏳");
        });
        output.textures_delta.clear();
    }
}

/// Message bubble colors
pub mod bubble {
    use egui::Color32;

    pub const OUTGOING_BG: Color32 = Color32::from_rgb(0x00, 0x5C, 0x4B);    // WhatsApp green dark
    pub const OUTGOING_TEXT: Color32 = Color32::WHITE;
    pub const INCOMING_BG: Color32 = Color32::from_rgb(0x20, 0x2C, 0x33);    // Dark gray
    pub const INCOMING_TEXT: Color32 = Color32::WHITE;
    pub const SYSTEM_BG: Color32 = Color32::TRANSPARENT;
    pub const SYSTEM_TEXT: Color32 = Color32::from_rgb(0x86, 0x96, 0xA0);    // Gray
    pub const REPLY_BG: Color32 = Color32::from_rgb(0x1A, 0x26, 0x2C);
    pub const REPLY_TEXT: Color32 = Color32::from_rgb(0x86, 0x96, 0xA0);
    pub const REPLY_NAME: Color32 = Color32::from_rgb(0x00, 0xA8, 0x84);
}

/// Status icon colors
pub mod status {
    use egui::Color32;

    pub const SENDING: Color32 = Color32::from_rgb(0x86, 0x96, 0xA0);
    pub const SENT: Color32 = Color32::from_rgb(0x86, 0x96, 0xA0);           // Single check
    pub const DELIVERED: Color32 = Color32::from_rgb(0x86, 0x96, 0xA0);      // Double check gray
    pub const READ: Color32 = Color32::from_rgb(0x53, 0xB6, 0x9A);           // Double check blue
    pub const FAILED: Color32 = Color32::from_rgb(0xF1, 0x5C, 0x5C);         // Red
}

/// Chat list colors
pub mod chat_list {
    use egui::Color32;

    pub const BACKGROUND: Color32 = Color32::from_rgb(0x11, 0x1B, 0x21);
    pub const HOVER: Color32 = Color32::from_rgb(0x1F, 0x2C, 0x34);
    pub const SELECTED: Color32 = Color32::from_rgb(0x2A, 0x39, 0x42);
    pub const UNREAD_BG: Color32 = Color32::from_rgb(0x00, 0x80, 0x69);
    pub const UNREAD_TEXT: Color32 = Color32::WHITE;
    pub const MUTED_ICON: Color32 = Color32::from_rgb(0x86, 0x96, 0xA0);
    pub const PINNED_ICON: Color32 = Color32::from_rgb(0xFF, 0xD7, 0x00);
}