//! Forward target picker — shown after ticking messages in the bulk-action bar.

use egui::{Align2, Color32, FontId, RichText};
use whatsapp_rust::wacore_binary::Jid;

use crate::commands::Command;
use crate::state::SharedState;

/// Draw the forward dialog, if one is open.
pub fn show_forward_dialog(
    ctx: &egui::Context,
    state: &SharedState,
    backend: &crate::backend::BackendHandle,
) {
    let mut s = state.write();

    let Some(mut dialog) = s.forward_dialog.clone() else {
        return;
    };

    let mut close = false;
    let mut target: Option<Jid> = None;

    egui::Window::new("↪  Přeposlat")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(360.0);

            ui.label(
                RichText::new(format!("Vybrané zprávy: {}", dialog.message_ids.len()))
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
            );
            ui.add_space(6.0);

            ui.add(
                egui::TextEdit::singleline(&mut dialog.search)
                    .hint_text("Hledat chat…")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);

            let query = dialog.search.trim().to_lowercase();
            let mut chats: Vec<&crate::state::Chat> = s
                .chats
                .values()
                .filter(|c| !c.is_archived)
                .filter(|c| query.is_empty() || c.name.to_lowercase().contains(&query))
                .collect();
            chats.sort_by_key(|c| std::cmp::Reverse(c.last_activity));

            egui::ScrollArea::vertical()
                .max_height(320.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if chats.is_empty() {
                        ui.label(
                            RichText::new("Žádný chat nenalezen")
                                .font(FontId::proportional(12.0))
                                .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                        );
                        return;
                    }

                    for chat in chats {
                        let label = if chat.is_group {
                            format!("👥  {}", chat.name)
                        } else {
                            format!("👤  {}", chat.name)
                        };
                        let row = ui.add_sized(
                            [ui.available_width(), 32.0],
                            egui::Button::new(
                                RichText::new(label)
                                    .font(FontId::proportional(13.0))
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
                            .corner_radius(egui::CornerRadius::same(6)),
                        );
                        if row.clicked() {
                            target = Some(chat.jid.clone());
                        }
                        ui.add_space(2.0);
                    }
                });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Zrušit").clicked() {
                    close = true;
                }
            });
        });

    if let Some(to) = target {
        let _ = backend.send_command(Command::ForwardMessages {
            to,
            from_chat: dialog.source_chat.clone(),
            message_ids: dialog.message_ids.clone(),
        });
        s.selected_messages.clear();
        s.forward_dialog = None;
    } else if close {
        s.forward_dialog = None;
    } else {
        // Keep the search box in sync so it survives a repaint.
        s.forward_dialog = Some(dialog);
    }
}
