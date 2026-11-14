//! Group creation and the "add member" picker.
//!
//! Both dialogs are the same widget with a different footer, so they share the
//! candidate list and the row rendering.

use std::collections::{HashMap, HashSet};

use egui::{Color32, FontId, RichText};
use whatsapp_rust::wacore_binary::Jid;

use crate::backend::BackendHandle;
use crate::commands::Command;
use crate::state::{Contact, SharedState};
use crate::ui::components::Avatar;

/// Contacts whose name or number matches `query`, minus anyone in `exclude`.
fn candidates(
    contacts: &HashMap<Jid, Contact>,
    query: &str,
    exclude: Option<&HashSet<Jid>>,
) -> Vec<(Jid, String)> {
    let query = query.trim().to_lowercase();
    let mut rows: Vec<(Jid, String)> = contacts
        .iter()
        .filter(|(jid, _)| exclude.map_or(true, |skip| !skip.contains(jid)))
        .map(|(jid, contact)| {
            let name = contact
                .name
                .as_deref()
                .or(contact.push_name.as_deref())
                .map(|n| n.to_string())
                .unwrap_or_else(|| crate::util::format_jid(jid));
            (jid.clone(), name)
        })
        .filter(|(jid, name)| {
            query.is_empty()
                || name.to_lowercase().contains(&query)
                || crate::util::format_jid(jid).contains(&query)
        })
        .collect();
    rows.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    rows
}

fn participant_rows(ui: &mut egui::Ui, rows: &[(Jid, String)], selected: &mut HashSet<Jid>) {
    for (jid, name) in rows {
        let mut ticked = selected.contains(jid);
        let response = ui
            .horizontal(|ui| {
                if ui.checkbox(&mut ticked, "").changed() {
                    if ticked {
                        selected.insert(jid.clone());
                    } else {
                        selected.remove(jid);
                    }
                }
                ui.add(Avatar::new(name).jid(jid.to_non_ad()).size(26.0));
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(name).font(FontId::proportional(13.0)));
                    ui.label(
                        RichText::new(crate::util::format_jid(jid))
                            .font(FontId::proportional(10.0))
                            .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                    );
                });
            })
            .response;
        // Clicking anywhere on the row toggles, like WhatsApp's picker.
        if response.interact(egui::Sense::click()).clicked() {
            if selected.contains(jid) {
                selected.remove(jid);
            } else {
                selected.insert(jid.clone());
            }
        }
    }
}

fn empty_hint(ui: &mut egui::Ui, contacts_empty: bool) {
    ui.add_space(8.0);
    ui.label(
        RichText::new(if contacts_empty {
            "Zatím žádné kontakty — synchronizujte je v seznamu chatů (🔄)."
        } else {
            "Nic nenalezeno."
        })
        .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
    );
}

pub fn show_new_group_dialog(ctx: &egui::Context, state: &SharedState, backend: &BackendHandle) {
    let mut s = state.write();
    let Some(mut dialog) = s.new_group.clone() else {
        return;
    };

    let rows = candidates(&s.contacts, &dialog.search, None);
    let contacts_empty = s.contacts.is_empty();
    let mut close = false;
    let mut create = false;

    egui::Window::new(RichText::new("👥  Nová skupina").font(FontId::proportional(15.0)))
        .collapsible(false)
        .resizable(true)
        .default_size([380.0, 480.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Název skupiny").font(FontId::proportional(12.0)));
            ui.add(
                egui::TextEdit::singleline(&mut dialog.subject)
                    .hint_text("Např. Rodina")
                    .desired_width(ui.available_width()),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut dialog.search)
                        .hint_text("Hledat účastníky…")
                        .desired_width(ui.available_width() - 110.0),
                );
                ui.label(
                    RichText::new(format!("Vybráno: {}", dialog.selected.len()))
                        .font(FontId::proportional(12.0))
                        .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                );
            });
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if rows.is_empty() {
                        empty_hint(ui, contacts_empty);
                    } else {
                        participant_rows(ui, &rows, &mut dialog.selected);
                    }
                });

            if let Some(err) = &dialog.error {
                ui.colored_label(Color32::from_rgb(0xFF, 0x6B, 0x6B), err);
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Zrušit").clicked() {
                    close = true;
                }
                let ready = !dialog.subject.trim().is_empty() && !dialog.selected.is_empty();
                if ui
                    .add_enabled(
                        ready,
                        egui::Button::new(
                            RichText::new("Vytvořit skupinu").color(Color32::WHITE),
                        )
                        .fill(crate::theme::ACCENT),
                    )
                    .clicked()
                {
                    create = true;
                }
            });
        });

    if create {
        let participants: Vec<Jid> = dialog.selected.iter().cloned().collect();
        let _ = backend.send_command(Command::CreateGroup {
            subject: dialog.subject.trim().to_string(),
            participants,
        });
        s.new_group = None;
    } else if close {
        s.new_group = None;
    } else {
        s.new_group = Some(dialog);
    }
}

pub fn show_add_member_dialog(ctx: &egui::Context, state: &SharedState, backend: &BackendHandle) {
    let mut s = state.write();
    let Some(mut dialog) = s.add_member.clone() else {
        return;
    };

    let members: HashSet<Jid> = s
        .chats
        .get(&dialog.group_jid)
        .map(|chat| chat.participants.clone())
        .unwrap_or_default();
    let rows = candidates(&s.contacts, &dialog.search, Some(&members));
    let group_name = s
        .chats
        .get(&dialog.group_jid)
        .map(|chat| chat.name.clone())
        .unwrap_or_else(|| crate::util::format_jid(&dialog.group_jid));
    let contacts_empty = s.contacts.is_empty();
    let mut close = false;
    let mut add = false;

    egui::Window::new(RichText::new("➕  Přidat člena").font(FontId::proportional(15.0)))
        .collapsible(false)
        .resizable(true)
        .default_size([380.0, 460.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("Do skupiny {group_name}"))
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
            );
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::singleline(&mut dialog.search)
                    .hint_text("Hledat kontakty…")
                    .desired_width(ui.available_width()),
            );
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if rows.is_empty() {
                        empty_hint(ui, contacts_empty || members.len() > 1);
                    } else {
                        participant_rows(ui, &rows, &mut dialog.selected);
                    }
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Zrušit").clicked() {
                    close = true;
                }
                if ui
                    .add_enabled(
                        !dialog.selected.is_empty(),
                        egui::Button::new(RichText::new("Přidat").color(Color32::WHITE))
                            .fill(crate::theme::ACCENT),
                    )
                    .clicked()
                {
                    add = true;
                }
            });
        });

    if add {
        let participants: Vec<Jid> = dialog.selected.iter().cloned().collect();
        let _ = backend.send_command(Command::AddParticipants {
            chat_jid: dialog.group_jid.clone(),
            participants,
        });
        s.add_member = None;
    } else if close {
        s.add_member = None;
    } else {
        s.add_member = Some(dialog);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jid(number: &str) -> Jid {
        Jid::pn(number)
    }

    fn contacts() -> HashMap<Jid, Contact> {
        let mut map = HashMap::new();
        for (number, name) in [("420111222333", "Alice"), ("420444555666", "Bob")] {
            let j = jid(number);
            map.insert(j.clone(), Contact::new(j, Some(name.to_string())));
        }
        map
    }

    #[test]
    fn candidates_sort_filter_and_exclude_members() {
        let contacts = contacts();

        let all = candidates(&contacts, "", None);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].1, "Alice", "rows come back alphabetically");

        let filtered = candidates(&contacts, "bo", None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1, "Bob");

        let mut exclude = HashSet::new();
        exclude.insert(jid("420111222333"));
        let remaining = candidates(&contacts, "", Some(&exclude));
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].1, "Bob", "existing members are not offered again");
    }

    #[test]
    fn unnamed_contacts_fall_back_to_the_number() {
        let jid = jid("420777888999");
        let mut map = HashMap::new();
        map.insert(jid.clone(), Contact::new(jid, None));

        let rows = candidates(&map, "", None);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].1.contains("777"), "falls back to the formatted number: {}", rows[0].1);
    }
}
