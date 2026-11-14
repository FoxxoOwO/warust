//! Contact browser.
//!
//! Contacts reach `AppState.contacts` through sync and message traffic, but
//! until now the only way to start a conversation was to type a phone number.
//! This window lists everyone we know about, filterable, and opens a chat.

use egui::{Color32, FontId, RichText};
use whatsapp_rust::wacore_binary::Jid;

use crate::backend::BackendHandle;
use crate::state::SharedState;
use crate::ui::components::Avatar;

pub fn show_contacts_dialog(ctx: &egui::Context, state: &SharedState, backend: &BackendHandle) {
    let mut s = state.write();
    if !s.show_contacts_dialog {
        return;
    }

    let mut open = true;
    let mut open_jid: Option<Jid> = None;

    egui::Window::new(RichText::new("👥  Kontakty").font(FontId::proportional(15.0)))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([380.0, 480.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut s.contacts_search)
                        .hint_text("Hledat kontakt…")
                        .desired_width(ui.available_width()),
                );
            });
            ui.add_space(6.0);

            let query = s.contacts_search.trim().to_lowercase();
            let mut rows: Vec<(Jid, String, bool)> = s
                .contacts
                .iter()
                .map(|(jid, contact)| {
                    let name = contact
                        .name
                        .as_deref()
                        .or(contact.push_name.as_deref())
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| crate::util::format_jid(jid));
                    (jid.clone(), name, contact.is_blocked)
                })
                .filter(|(_, name, _)| {
                    query.is_empty() || name.to_lowercase().contains(&query)
                })
                .collect();
            rows.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

            if rows.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(if s.contacts.is_empty() {
                        "Zatím žádné kontakty — synchronizujte je v seznamu chatů (🔄)."
                    } else {
                        "Nic nenalezeno."
                    })
                    .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                );
                return;
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (jid, name, blocked) in rows {
                        let response = ui
                            .horizontal(|ui| {
                                ui.add(Avatar::new(&name).jid(jid.to_non_ad()).size(30.0));
                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&name).font(FontId::proportional(14.0)));
                                    ui.label(
                                        RichText::new(crate::util::format_jid(&jid))
                                            .font(FontId::proportional(11.0))
                                            .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                                    );
                                });
                                if blocked {
                                    ui.label(
                                        RichText::new("🚫")
                                            .font(FontId::proportional(12.0))
                                            .color(Color32::from_rgb(0xF2, 0x8B, 0x82)),
                                    );
                                }
                            })
                            .response;
                        let response = response.interact(egui::Sense::click());
                        if response.clicked() {
                            open_jid = Some(jid.clone());
                        }
                        ui.separator();
                    }
                });
        });

    if let Some(jid) = open_jid {
        s.show_contacts_dialog = false;
        crate::ui::open_chat(&mut s, &jid, backend);
    }
    if !open {
        s.show_contacts_dialog = false;
    }
}
