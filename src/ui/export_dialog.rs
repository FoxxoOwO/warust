//! Conversation Export Modal Dialog

use egui::{Align, Color32, FontId, Frame, Layout, Margin, RichText};
use crate::export::ExportFormat;
use crate::state::SharedState;

pub fn show_export_dialog(ctx: &egui::Context, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let mut s = state.write();
    let (chat_jid, chat_name, mut selected_format, status_msg, exported_path) = match s.export_dialog.as_ref() {
        Some(d) => (
            d.chat_jid.clone(),
            d.chat_name.clone(),
            d.format,
            d.status_message.clone(),
            d.exported_path.clone(),
        ),
        None => return,
    };

    let msg_count = s.messages.get(&chat_jid).map(|m| m.len()).unwrap_or(0);
    let mut request_export = false;
    let mut open_folder_path = None;
    let mut close = false;

    egui::Window::new("📤  Export Conversation")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.add_space(6.0);

            // Chat info header
            Frame::new()
                .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
                .inner_margin(Margin::same(12))
                .corner_radius(egui::CornerRadius::same(8))
                .show(ui, |ui| {
                    ui.label(RichText::new(&chat_name).font(FontId::proportional(15.0)).strong().color(Color32::WHITE));
                    ui.add_space(2.0);
                    ui.label(RichText::new(format!("Total messages: {}", msg_count)).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                });

            ui.add_space(12.0);

            // Format selection
            ui.label(RichText::new("Choose export format:").font(FontId::proportional(13.0)).strong());
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.radio_value(&mut selected_format, ExportFormat::Txt, "📄 Text (.txt)");
                ui.add_space(8.0);
                ui.radio_value(&mut selected_format, ExportFormat::Json, "📦 JSON (.json)");
                ui.add_space(8.0);
                ui.radio_value(&mut selected_format, ExportFormat::Html, "🌐 HTML (.html)");
            });

            ui.add_space(8.0);

            let desc = match selected_format {
                ExportFormat::Txt => "Standard readable text file with timestamps and sender names.",
                ExportFormat::Json => "Full structured data export including message IDs, statuses and media metadata.",
                ExportFormat::Html => "Interactive offline webpage styled in dark WhatsApp bubbles.",
            };
            ui.label(RichText::new(desc).font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));

            ui.add_space(14.0);

            // Status / result message
            if let Some((success, text)) = &status_msg {
                let bg = if *success {
                    Color32::from_rgba_premultiplied(0, 168, 132, 35)
                } else {
                    Color32::from_rgba_premultiplied(241, 92, 92, 35)
                };
                let text_color = if *success {
                    Color32::from_rgb(0x25, 0xD3, 0x66)
                } else {
                    Color32::from_rgb(0xF1, 0x5C, 0x5C)
                };

                Frame::new()
                    .fill(bg)
                    .inner_margin(Margin::same(10))
                    .corner_radius(egui::CornerRadius::same(6))
                    .show(ui, |ui| {
                        ui.colored_label(text_color, text);
                    });
                ui.add_space(8.0);
            }

            // Action buttons
            ui.horizontal(|ui| {
                if status_msg.as_ref().map_or(false, |(s, _)| *s) {
                    if let Some(ref path) = exported_path {
                        if ui.button("📂  Open in Explorer").clicked() {
                            open_folder_path = Some(path.clone());
                        }
                    }
                } else {
                    if ui.button(RichText::new("📤  Export Now").strong()).clicked() {
                        request_export = true;
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(6.0);
        });

    if let Some(path) = open_folder_path {
        crate::export::open_in_file_manager(&path);
    }

    if request_export {
        if let Some(dlg) = s.export_dialog.as_mut() {
            dlg.format = selected_format;
        }
        let _ = backend.send_command(crate::commands::Command::ExportChat {
            chat_jid,
            format: selected_format,
            custom_dir: None,
        });
    }

    if close {
        s.export_dialog = None;
    }
}
