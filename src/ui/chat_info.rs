//! Right-hand info panel for the current chat.
//!
//! Mirrors what WhatsApp Web shows when you click a chat header: a large
//! avatar, the contact's number/presence, and the per-chat settings (mute
//! duration, disappearing messages) plus the danger actions (clear, block,
//! leave group).

use egui::{Align, Color32, FontId, Layout, RichText};
use whatsapp_rust::wacore_binary::Jid;

use crate::backend::BackendHandle;
use crate::commands::Command;
use crate::state::{AppState, Chat};
use crate::ui::components::Avatar;

const MUTED_TEXT: Color32 = Color32::from_rgb(0x86, 0x96, 0xA0);
const DANGER: Color32 = Color32::from_rgb(0xF1, 0x5C, 0x5C);

/// Disappearing-message presets, in seconds. `0` disables the timer.
const EPHEMERAL_PRESETS: [(u32, &str); 4] = [
    (0, "Vypnuto"),
    (86_400, "24 hodin"),
    (604_800, "7 dní"),
    (7_776_000, "90 dní"),
];

pub fn show(ui: &mut egui::Ui, state: &mut AppState, chat: &Chat, backend: &BackendHandle) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(
            RichText::new(if chat.is_group {
                "Info o skupině"
            } else {
                "Info o kontaktu"
            })
            .font(FontId::proportional(16.0))
            .strong()
            .color(Color32::WHITE),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("✕").font(FontId::proportional(15.0)))
                        .frame(false),
                )
                .on_hover_text("Zavřít")
                .clicked()
            {
                state.show_chat_info = false;
            }
        });
    });
    ui.add_space(4.0);
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(14.0);
            ui.vertical_centered(|ui| {
                ui.add(Avatar::new(&chat.name).jid(chat.jid.clone()).size(96.0));
                ui.add_space(10.0);
                ui.label(
                    RichText::new(&chat.name)
                        .font(FontId::proportional(19.0))
                        .strong()
                        .color(Color32::WHITE),
                );
                let subtitle = if chat.is_group {
                    format!("{} členů", chat.participants.len())
                } else {
                    crate::util::format_jid(&chat.jid)
                };
                ui.label(
                    RichText::new(subtitle)
                        .font(FontId::proportional(13.0))
                        .color(MUTED_TEXT),
                );
            });
            ui.add_space(14.0);
            ui.separator();

            if chat.is_group {
                group_details(ui, state, chat, backend);
            } else {
                contact_details(ui, state, chat, backend);
            }

            chat_settings(ui, state, chat, backend);
        });
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(
        RichText::new(title)
            .font(FontId::proportional(12.0))
            .strong()
            .color(crate::theme::ACCENT),
    );
    ui.add_space(2.0);
}

fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(13.0))
                .color(MUTED_TEXT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .font(FontId::proportional(13.0))
                    .color(Color32::WHITE),
            );
        });
    });
}

fn contact_details(
    ui: &mut egui::Ui,
    state: &AppState,
    chat: &Chat,
    backend: &BackendHandle,
) {
    let contact = state.contacts.get(&chat.jid).cloned();

    section_title(ui, "O KONTAKTU");
    info_row(ui, "Telefon", &crate::util::format_jid(&chat.jid));

    if let Some(contact) = &contact {
        if let Some(verified) = &contact.verified_name {
            info_row(ui, "Ověřený název", verified);
        }
        if contact.is_business {
            info_row(ui, "Účet", "Firma");
        }
        let presence = if contact.is_online {
            "online".to_string()
        } else if let Some(last_seen) = contact.last_seen {
            format!("naposledy {}", crate::util::format_timestamp(last_seen))
        } else {
            "neznámé".to_string()
        };
        info_row(ui, "Stav", &presence);
    }

    if contact.as_ref().map(|c| c.is_blocked).unwrap_or(false) {
        info_row(ui, "Blokace", "kontakt je blokován");
    }

    ui.add_space(8.0);
    ui.separator();

    section_title(ui, "AKCE");
    let blocked = contact.as_ref().map(|c| c.is_blocked).unwrap_or(false);
    let label = if blocked {
        "Odblokovat kontakt"
    } else {
        "Blokovat kontakt"
    };
    if ui
        .add(egui::Button::new(
            RichText::new(label).font(FontId::proportional(13.0)).color(DANGER),
        ))
        .clicked()
    {
        let cmd = if blocked {
            Command::UnblockContact {
                jid: chat.jid.clone(),
            }
        } else {
            Command::BlockContact {
                jid: chat.jid.clone(),
            }
        };
        let _ = backend.send_command(cmd);
    }
}

fn group_details(ui: &mut egui::Ui, state: &AppState, chat: &Chat, backend: &BackendHandle) {
    section_title(ui, &format!("ČLENOVÉ ({})", chat.participants.len()));
    if chat.participants.is_empty() {
        ui.label(
            RichText::new("Seznam členů zatím není k dispozici.")
                .font(FontId::proportional(12.0))
                .color(MUTED_TEXT),
        );
    } else {
        let mut participants: Vec<&Jid> = chat
            .participants
            .iter()
            .filter(|p| Some(*p) != state.self_jid.as_ref())
            .collect();
        participants.sort_by_key(|p| display_name_for(state, p).to_lowercase());
        egui::ScrollArea::vertical()
            .id_salt("chat_info_participants")
            .max_height(220.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for participant in participants {
                    ui.horizontal(|ui| {
                        ui.add(
                            Avatar::new(display_name_for(state, participant))
                                .jid(participant.clone())
                                .size(26.0),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(display_name_for(state, participant))
                                .font(FontId::proportional(13.0))
                                .color(Color32::WHITE),
                        );
                    });
                }
            });
    }

    ui.add_space(8.0);
    ui.separator();

    section_title(ui, "AKCE");
    if ui
        .add(egui::Button::new(
            RichText::new("Opustit skupinu")
                .font(FontId::proportional(13.0))
                .color(DANGER),
        ))
        .clicked()
    {
        let _ = backend.send_command(Command::LeaveGroup {
            chat_jid: chat.jid.clone(),
        });
    }
}

fn display_name_for(state: &AppState, jid: &Jid) -> String {
    state
        .contacts
        .get(jid)
        .and_then(|c| c.name.clone().or_else(|| c.push_name.clone()))
        .unwrap_or_else(|| crate::util::format_jid(jid))
}

fn chat_settings(ui: &mut egui::Ui, state: &mut AppState, chat: &Chat, backend: &BackendHandle) {
    section_title(ui, "NASTAVENÍ CHATU");

    // Mute duration. `MuteChat { until: None }` means "always".
    let muted = chat.is_muted();
    let mute_label = match chat.mute_until {
        Some(until) => format!("do {}", crate::util::format_timestamp(until)),
        None if muted => "vždy".to_string(),
        None => "vypnuto".to_string(),
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Umlčení")
                .font(FontId::proportional(13.0))
                .color(MUTED_TEXT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            egui::ComboBox::from_id_salt("chat_info_mute")
                .selected_text(RichText::new(mute_label).font(FontId::proportional(13.0)))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(!muted, "Vypnuto").clicked() {
                        let _ = backend.send_command(Command::UnmuteChat {
                            chat_jid: chat.jid.clone(),
                        });
                    }
                    for (label, hours) in [("8 hodin", 8i64), ("1 týden", 24 * 7)] {
                        if ui.selectable_label(false, label).clicked() {
                            let until = chrono::Utc::now() + chrono::Duration::hours(hours);
                            let _ = backend.send_command(Command::MuteChat {
                                chat_jid: chat.jid.clone(),
                                until: Some(until),
                            });
                        }
                    }
                    if ui.selectable_label(false, "Vždy").clicked() {
                        let _ = backend.send_command(Command::MuteChat {
                            chat_jid: chat.jid.clone(),
                            until: None,
                        });
                    }
                });
        });
    });

    // Disappearing messages. Works for 1:1 chats as well as groups.
    let current_secs = state.ephemeral.get(&chat.jid).copied().unwrap_or(0);
    let current_label = EPHEMERAL_PRESETS
        .iter()
        .find(|(secs, _)| *secs == current_secs)
        .map(|(_, label)| *label)
        .unwrap_or("vlastní");
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Mizející zprávy")
                .font(FontId::proportional(13.0))
                .color(MUTED_TEXT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            egui::ComboBox::from_id_salt("chat_info_ephemeral")
                .selected_text(RichText::new(current_label).font(FontId::proportional(13.0)))
                .show_ui(ui, |ui| {
                    for (secs, label) in EPHEMERAL_PRESETS {
                        if ui
                            .selectable_label(secs == current_secs, label)
                            .clicked()
                        {
                            let _ = backend.send_command(Command::SetEphemeral {
                                chat_jid: chat.jid.clone(),
                                expiration: secs,
                            });
                        }
                    }
                });
        });
    });

    ui.add_space(10.0);
    ui.separator();
    section_title(ui, "SPRÁVA");
    if ui
        .add(egui::Button::new(
            RichText::new("Vymazat zprávy")
                .font(FontId::proportional(13.0))
                .color(Color32::WHITE),
        ))
        .clicked()
    {
        let _ = backend.send_command(Command::ClearChat {
            chat_jid: chat.jid.clone(),
        });
    }
}
