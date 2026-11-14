//! Sticker picker — sends WebP files as real sticker messages.

use egui::{Color32, FontId, RichText};

use crate::backend::BackendHandle;
use crate::commands::Command;
use crate::state::SharedState;

pub fn show_sticker_dialog(
    ctx: &egui::Context,
    state: &SharedState,
    backend: &BackendHandle,
) {
    let mut s = state.write();
    if !s.show_sticker_picker {
        return;
    }

    let chat_jid = match s.selected_chat.clone() {
        Some(jid) => jid,
        None => {
            s.show_sticker_picker = false;
            return;
        }
    };

    let mut open = true;
    let mut send: Option<std::path::PathBuf> = None;
    let mut pick_new = false;

    egui::Window::new(RichText::new("🎨  Samolepky").font(FontId::proportional(15.0)))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([360.0, 320.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("＋ Přidat ze souboru…").clicked() {
                    pick_new = true;
                }
                ui.label(
                    RichText::new("WebP, max. 1 MB")
                        .font(FontId::proportional(11.0))
                        .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                );
            });
            ui.separator();

            let files = crate::media::sticker_files();
            if files.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Zatím žádné samolepky. Přijaté a odeslané WebP soubory se tu objeví.",
                    )
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                );
                return;
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for path in &files {
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or_default()
                                .to_string();
                            let response = match crate::media::path_texture(ctx, path) {
                                Some(texture) => ui.add(
                                    egui::Image::new(egui::load::SizedTexture::new(
                                        texture.id(),
                                        egui::vec2(64.0, 64.0),
                                    ))
                                    .corner_radius(6.0)
                                    .sense(egui::Sense::click()),
                                ),
                                None => ui.add(egui::Button::new("🎨").min_size(egui::vec2(64.0, 64.0))),
                            };
                            if response.on_hover_text(name).clicked() {
                                send = Some(path.clone());
                            }
                        }
                    });
                });
        });

    if pick_new {
        if let Some(path) = crate::media::pick_file() {
            send = Some(path);
        }
    }
    if let Some(path) = send {
        let _ = backend.send_command(Command::SendMedia {
            chat_jid,
            file_path: path,
            caption: None,
            reply_to: None,
            view_once: false,
        });
        s.show_sticker_picker = false;
    }
    if !open {
        s.show_sticker_picker = false;
    }
}
