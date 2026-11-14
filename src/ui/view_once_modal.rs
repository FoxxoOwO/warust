//! Modal viewer for view-once media.
//!
//! Opening this window is the only moment the payload is shown; closing it
//! deletes the local copy and marks the message as spent, mirroring how the
//! phone client behaves.

use eframe::egui;

use crate::backend::BackendHandle;
use crate::commands::Command;
use crate::state::{MediaType, Message, SharedState};

pub fn show_view_once_modal(ctx: &egui::Context, state: &SharedState, backend: &BackendHandle) {
    let open = state.read().view_once_open.clone();
    let Some((chat_jid, message_id)) = open else {
        return;
    };

    let found = {
        let s = state.read();
        s.get_chat_messages(&chat_jid)
            .and_then(|msgs| msgs.iter().find(|m| m.id == message_id))
            .cloned()
    };
    let Some(message) = found else {
        // The message is gone (deleted remotely) — nothing to show.
        state.write().view_once_open = None;
        return;
    };

    let mut close = false;
    egui::Window::new("Zobrazit jednou")
        .id(egui::Id::new("view_once_modal"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Zobrazit jednou")
                        .color(crate::theme::ACCENT)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✕").clicked() {
                        close = true;
                    }
                });
            });
            ui.separator();
            render_body(ui, &message);
            ui.separator();
            ui.label(
                egui::RichText::new("Zavřením se médium smaže a už ho nezobrazíte.")
                    .small()
                    .color(egui::Color32::GRAY),
            );
            if ui.button("Zavřít").clicked() {
                close = true;
            }
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }

    if close {
        let _ = backend.send_command(Command::ConsumeViewOnce {
            chat_jid,
            message_id,
        });
        state.write().view_once_open = None;
    }
}

fn render_body(ui: &mut egui::Ui, message: &Message) {
    let path = message
        .media_path
        .as_ref()
        .map(std::path::Path::new)
        .filter(|p| p.exists());

    let Some(path) = path else {
        // Still downloading: the backend fills `media_path` when it is done.
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Stahuji médium…");
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        });
        return;
    };

    match message.media_type {
        Some(MediaType::Image) | Some(MediaType::Sticker) => {
            if let Some((texture, size)) = crate::media::image_texture(ui.ctx(), message) {
                ui.image((texture.id(), egui::vec2(size[0], size[1])));
            } else {
                ui.label("Náhled se nepodařilo načíst.");
            }
        }
        Some(MediaType::Video) => {
            if ui
                .button(egui::RichText::new("▶ Přehrát video").size(16.0))
                .clicked()
            {
                let _ = crate::video::toggle(&message.id, path);
            }
        }
        Some(MediaType::Audio) | Some(MediaType::Voice) => {
            if ui
                .button(egui::RichText::new("▶ Přehrát").size(16.0))
                .clicked()
            {
                let _ = crate::audio::toggle(&message.id, path);
            }
        }
        _ => {
            if ui.button("Otevřít soubor").clicked() {
                crate::media::open_file(path);
            }
        }
    }
}
