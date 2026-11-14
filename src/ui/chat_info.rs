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
use crate::state::{AppState, Chat, GalleryTab, MediaType, Message};
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

            media_gallery(ui, state, chat);

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


/// Pull every `http(s)://` link out of `text`, without duplicates.
///
/// WhatsApp links plain URLs in message bodies; this is the same idea, kept
/// deliberately dumb so it never swallows the punctuation around a link.
pub fn extract_links(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in text.split_whitespace() {
        let trimmed = token.trim_matches(|c: char| {
            matches!(
                c,
                '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '"' | '\'' | ',' | ';'
            )
        });
        let candidate = trimmed.trim_end_matches(['.', ':', '!', '?']);
        if (candidate.starts_with("http://") || candidate.starts_with("https://"))
            && !out.iter().any(|existing| existing == candidate)
        {
            out.push(candidate.to_string());
        }
    }
    out
}

/// What to call a document in the gallery list.
fn document_label(message: &Message) -> String {
    if let Some(path) = message.media_path.as_deref() {
        if let Some(name) = std::path::Path::new(path).file_name() {
            return name.to_string_lossy().to_string();
        }
    }
    let caption = message.text.trim();
    if !caption.is_empty() {
        return caption.to_string();
    }
    match message.media_mimetype.as_deref() {
        Some(mime) if !mime.is_empty() => "Dokument".to_string(),
        _ => "Soubor".to_string(),
    }
}

fn is_gallery_media(kind: Option<MediaType>) -> bool {
    matches!(
        kind,
        Some(MediaType::Image) | Some(MediaType::Video) | Some(MediaType::Sticker)
    )
}

/// Messages that belong to `tab`, newest first.
fn gallery_messages<'a>(messages: &'a [Message], tab: GalleryTab) -> Vec<&'a Message> {
    let mut out: Vec<&Message> = messages
        .iter()
        .filter(|message| match tab {
            GalleryTab::Media => is_gallery_media(message.media_type),
            GalleryTab::Documents => message.media_type == Some(MediaType::Document),
            GalleryTab::Links => {
                !message.text.is_empty() && !extract_links(&message.text).is_empty()
            }
        })
        .collect();
    out.reverse();
    out
}

/// "Média, odkazy a dokumenty" section: the gallery WhatsApp puts above the
/// per-chat settings.
fn media_gallery(ui: &mut egui::Ui, state: &mut AppState, chat: &Chat) {
    let jid = chat.jid.clone();
    let tab = state.gallery_tab;
    let counts = {
        let messages = state.messages.get(&jid).map(|m| m.as_slice()).unwrap_or(&[]);
        GalleryTab::ALL.map(|candidate| gallery_messages(messages, candidate).len())
    };

    section_title(ui, "Média, odkazy a dokumenty");
    ui.horizontal_wrapped(|ui| {
        for (index, candidate) in GalleryTab::ALL.into_iter().enumerate() {
            let label = format!("{} ({})", candidate.label(), counts[index]);
            if ui
                .selectable_label(
                    tab == candidate,
                    RichText::new(label).font(FontId::proportional(11.0)),
                )
                .clicked()
            {
                state.gallery_tab = candidate;
            }
        }
    });
    ui.add_space(4.0);

    // Collected first: rendering below needs an immutable borrow of `state` for
    // thumbnails, and the click result is applied afterwards.
    let mut jump_to: Option<String> = None;
    let mut open_url: Option<String> = None;

    match tab {
        GalleryTab::Media => {
            let ids: Vec<String> = {
                let messages = state.messages.get(&jid).map(|m| m.as_slice()).unwrap_or(&[]);
                gallery_messages(messages, GalleryTab::Media)
                    .into_iter()
                    .map(|message| message.id.clone())
                    .collect()
            };
            if ids.is_empty() {
                ui.label(
                    RichText::new("Zatím žádné fotky ani videa.")
                        .font(FontId::proportional(12.0))
                        .color(MUTED_TEXT),
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("chat_info_gallery_scroll")
                .max_height(180.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let cell = 54.0;
                    let per_row =
                        ((ui.available_width() + 4.0) / (cell + 4.0)).floor().max(1.0) as usize;
                    for chunk in ids.chunks(per_row) {
                        ui.horizontal(|ui| {
                            for id in chunk {
                                let message = state
                                    .messages
                                    .get(&jid)
                                    .and_then(|messages| messages.iter().find(|m| &m.id == id));
                                let Some(message) = message else {
                                    continue;
                                };
                                let response = match crate::media::image_texture(ui.ctx(), message)
                                {
                                    Some((texture, _)) => ui.add(
                                        egui::Image::new(&texture)
                                            .fit_to_exact_size(egui::vec2(cell, cell))
                                            .corner_radius(4.0)
                                            .sense(egui::Sense::click()),
                                    ),
                                    None => {
                                        let icon = if message.media_type == Some(MediaType::Video)
                                        {
                                            "🎬"
                                        } else {
                                            "🖼"
                                        };
                                        ui.add_sized(
                                            [cell, cell],
                                            egui::Button::new(
                                                RichText::new(icon)
                                                    .font(FontId::proportional(20.0)),
                                            )
                                            .fill(Color32::from_rgb(0x1D, 0x28, 0x2E))
                                            .corner_radius(4.0),
                                        )
                                    }
                                };
                                if response.clicked() {
                                    jump_to = Some(id.clone());
                                }
                            }
                        });
                    }
                });
        }
        GalleryTab::Documents | GalleryTab::Links => {
            let rows: Vec<(String, String)> = {
                let messages = state.messages.get(&jid).map(|m| m.as_slice()).unwrap_or(&[]);
                gallery_messages(messages, tab)
                    .into_iter()
                    .map(|message| {
                        if tab == GalleryTab::Links {
                            let first = extract_links(&message.text)
                                .first()
                                .cloned()
                                .unwrap_or_default();
                            (message.id.clone(), first)
                        } else {
                            (message.id.clone(), document_label(message))
                        }
                    })
                    .collect()
            };
            if rows.is_empty() {
                let empty = if tab == GalleryTab::Links {
                    "Zatím žádné odkazy."
                } else {
                    "Zatím žádné dokumenty."
                };
                ui.label(
                    RichText::new(empty)
                        .font(FontId::proportional(12.0))
                        .color(MUTED_TEXT),
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("chat_info_list_scroll")
                .max_height(160.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (message_id, label) in &rows {
                        ui.horizontal(|ui| {
                            let icon = if tab == GalleryTab::Links { "🔗" } else { "📄" };
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(format!("{icon} {label}"))
                                            .font(FontId::proportional(12.0)),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("Zobrazit zprávu")
                                .clicked()
                            {
                                jump_to = Some(message_id.clone());
                            }
                            if tab == GalleryTab::Links
                                && ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Otevřít")
                                                .font(FontId::proportional(11.0)),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                            {
                                open_url = Some(label.clone());
                            }
                        });
                    }
                });
        }
    }

    if let Some(message_id) = jump_to {
        state.jump_target = Some(crate::state::JumpTarget::new(message_id));
    }
    if let Some(url) = open_url {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
}

fn contact_details(
    ui: &mut egui::Ui,
    state: &mut AppState,
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
        if blocked {
            let _ = backend.send_command(Command::UnblockContact {
                jid: chat.jid.clone(),
            });
        } else {
            crate::ui::confirm_dialog::ask(
                state,
                format!("Blokovat {}?", chat.name),
                "Nebudou ti chodit zprávy ani hovory a nebude vidět, že jsi online.",
                "Blokovat",
                crate::state::ConfirmAction::BlockContact {
                    jid: chat.jid.clone(),
                },
            );
        }
    }
}

fn group_details(ui: &mut egui::Ui, state: &mut AppState, chat: &Chat, backend: &BackendHandle) {
    let key = chat.jid.to_string();
    let snapshot = state.group_info.get(&key).cloned().unwrap_or_default();
    let is_admin = state
        .self_jid
        .as_ref()
        .map(|me| snapshot.is_admin(&state.canonical_jid(me)))
        .unwrap_or(false);

    // Ask once per group; the backend fills the snapshot and the panel redraws.
    if !state.group_info.contains_key(&key) && !state.group_info_pending.contains(&key) {
        state.group_info_pending.insert(key.clone());
        let _ = backend.send_command(Command::RequestGroupInfo {
            chat_jid: chat.jid.clone(),
        });
    } else if state.group_info.contains_key(&key) {
        state.group_info_pending.remove(&key);
    }

    // --- Subject ---
    section_title(ui, "NÁZEV SKUPINY");
    let editing_subject = matches!(&state.group_subject_draft, Some((j, _)) if j == &chat.jid);
    if editing_subject {
        let mut save = None;
        let mut cancel = false;
        if let Some((_, draft)) = state.group_subject_draft.as_mut() {
            ui.add(
                egui::TextEdit::singleline(draft)
                    .hint_text("Název skupiny")
                    .desired_width(ui.available_width()),
            );
            ui.horizontal(|ui| {
                if ui.button("Uložit").clicked() {
                    save = Some(draft.trim().to_string());
                }
                if ui.button("Zrušit").clicked() {
                    cancel = true;
                }
            });
        }
        if let Some(subject) = save {
            if !subject.is_empty() {
                let _ = backend.send_command(Command::SetGroupSubject {
                    chat_jid: chat.jid.clone(),
                    subject,
                });
                state.group_subject_draft = None;
            }
        } else if cancel {
            state.group_subject_draft = None;
        }
    } else {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&chat.name)
                    .font(FontId::proportional(13.0))
                    .color(Color32::WHITE),
            );
            if is_admin && ui.small_button("✏").clicked() {
                state.group_subject_draft = Some((chat.jid.clone(), chat.name.clone()));
            }
        });
    }

    // --- Description ---
    section_title(ui, "POPIS SKUPINY");
    let editing_desc = matches!(&state.group_desc_draft, Some((j, _)) if j == &chat.jid);
    if editing_desc {
        let mut save = None;
        let mut cancel = false;
        if let Some((_, draft)) = state.group_desc_draft.as_mut() {
            ui.add(
                egui::TextEdit::multiline(draft)
                    .hint_text("Čím se skupina zabývá…")
                    .desired_rows(3)
                    .desired_width(ui.available_width()),
            );
            ui.horizontal(|ui| {
                if ui.button("Uložit").clicked() {
                    save = Some(draft.trim().to_string());
                }
                if ui.button("Zrušit").clicked() {
                    cancel = true;
                }
            });
        }
        if let Some(description) = save {
            let _ = backend.send_command(Command::SetGroupDescription {
                chat_jid: chat.jid.clone(),
                description,
            });
            state.group_desc_draft = None;
        } else if cancel {
            state.group_desc_draft = None;
        }
    } else {
        match snapshot.description.as_deref().filter(|d| !d.trim().is_empty()) {
            Some(description) => {
                ui.label(
                    RichText::new(description)
                        .font(FontId::proportional(13.0))
                        .color(MUTED_TEXT),
                );
            }
            None => {
                ui.label(
                    RichText::new("Skupina zatím nemá popis.")
                        .font(FontId::proportional(12.0))
                        .color(MUTED_TEXT),
                );
            }
        }
        if is_admin
            && ui
                .add(egui::Button::new(
                    RichText::new("Upravit popis").font(FontId::proportional(12.0)),
                ))
                .clicked()
        {
            state.group_desc_draft = Some((
                chat.jid.clone(),
                snapshot.description.clone().unwrap_or_default(),
            ));
        }
    }

    // Admin switches for the two group-wide restrictions.
    ui.add_space(4.0);
    if is_admin {
        let mut announce = snapshot.is_announcement;
        if ui
            .checkbox(&mut announce, "Zprávy mohou psát jen admini")
            .changed()
        {
            let _ = backend.send_command(Command::SetGroupAnnounce {
                chat_jid: chat.jid.clone(),
                enabled: announce,
            });
        }
        let mut locked = snapshot.is_locked;
        if ui
            .checkbox(&mut locked, "Info skupiny mohou měnit jen admini")
            .changed()
        {
            let _ = backend.send_command(Command::SetGroupLocked {
                chat_jid: chat.jid.clone(),
                enabled: locked,
            });
        }
    } else {
        if snapshot.is_announcement {
            info_row(ui, "Zprávy", "jen administrátoři");
        }
        if snapshot.is_locked {
            info_row(ui, "Úpravy", "jen administrátoři");
        }
    }

    // --- Members ---
    section_title(ui, &format!("ČLENOVÉ ({})", chat.participants.len()));
    let mut remove_member: Option<Jid> = None;
    let mut promote_member: Option<Jid> = None;
    let mut demote_member: Option<Jid> = None;

    if chat.participants.is_empty() {
        ui.label(
            RichText::new("Seznam členů zatím není k dispozici.")
                .font(FontId::proportional(12.0))
                .color(MUTED_TEXT),
        );
    } else {
        let me = state.self_jid.as_ref().map(|j| state.canonical_jid(j));
        let mut participants: Vec<Jid> = chat
            .participants
            .iter()
            .filter(|p| Some(*p) != me.as_ref())
            .cloned()
            .collect();
        participants.sort_by_key(|p| display_name_for(state, p).to_lowercase());

        egui::ScrollArea::vertical()
            .id_salt("chat_info_participants")
            .max_height(240.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for participant in &participants {
                    let name = display_name_for(state, participant);
                    let admin = snapshot.is_admin(participant);
                    ui.horizontal(|ui| {
                        ui.add(Avatar::new(&name).jid(participant.clone()).size(26.0));
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(&name)
                                .font(FontId::proportional(13.0))
                                .color(Color32::WHITE),
                        );
                        if admin {
                            ui.label(
                                RichText::new("admin")
                                    .font(FontId::proportional(10.0))
                                    .color(crate::theme::ACCENT),
                            );
                        }
                        if is_admin {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.menu_button("⋯", |ui| {
                                    if admin {
                                        if ui.button("Odebrat admina").clicked() {
                                            demote_member = Some(participant.clone());
                                            ui.close();
                                        }
                                    } else if ui.button("Povýšit na admina").clicked() {
                                        promote_member = Some(participant.clone());
                                        ui.close();
                                    }
                                    if ui
                                        .button(RichText::new("Odebrat ze skupiny").color(DANGER))
                                        .clicked()
                                    {
                                        remove_member = Some(participant.clone());
                                        ui.close();
                                    }
                                });
                            });
                        }
                    });
                }
            });
    }

    if is_admin
        && ui
            .add(egui::Button::new(
                RichText::new("➕  Přidat člena").font(FontId::proportional(12.0)),
            ))
            .clicked()
    {
        state.add_member = Some(crate::state::AddMemberState {
            group_jid: chat.jid.clone(),
            search: String::new(),
            selected: Default::default(),
        });
    }

    if let Some(member) = promote_member {
        let _ = backend.send_command(Command::PromoteParticipants {
            chat_jid: chat.jid.clone(),
            participants: vec![member],
        });
    }
    if let Some(member) = demote_member {
        let _ = backend.send_command(Command::DemoteParticipants {
            chat_jid: chat.jid.clone(),
            participants: vec![member],
        });
    }
    if let Some(member) = remove_member {
        let member_name = display_name_for(state, &member);
        crate::ui::confirm_dialog::ask(
            state,
            format!("Odebrat {member_name} ze skupiny?"),
            "Tato osoba přestane dostávat zprávy skupiny.",
            "Odebrat",
            crate::state::ConfirmAction::RemoveMember {
                chat_jid: chat.jid.clone(),
                member,
            },
        );
    }

    // --- Invite link ---
    section_title(ui, "POZVÁNKOVÝ ODKAZ");
    match snapshot.invite_link.as_deref() {
        Some(link) => {
            let short: String = link.chars().take(38).collect();
            ui.label(
                RichText::new(format!("{short}…"))
                    .font(FontId::proportional(11.0))
                    .color(MUTED_TEXT),
            );
            if ui.button("Kopírovat odkaz").clicked() {
                ui.ctx().copy_text(link.to_string());
            }
        }
        None => {
            if ui.button("Získat odkaz").clicked() {
                let _ = backend.send_command(Command::GetGroupInviteLink {
                    chat_jid: chat.jid.clone(),
                });
            }
        }
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
        crate::ui::confirm_dialog::ask(
            state,
            format!("Opustit skupinu {}?", chat.name),
            "Zprávy, které jsi poslal, zůstanou ostatním.",
            "Opustit",
            crate::state::ConfirmAction::LeaveGroup {
                chat_jid: chat.jid.clone(),
            },
        );
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
        crate::ui::confirm_dialog::ask(
            state,
            format!("Vymazat zprávy v chatu {}?", chat.name),
            "Historie zmizí jen u tebe, ostatní ji pořád uvidí.",
            "Vymazat",
            crate::state::ConfirmAction::ClearChat {
                chat_jid: chat.jid.clone(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MessageStatus;
    use chrono::Utc;
    use whatsapp_rust::wacore_binary::Jid;

    fn message(id: &str, text: &str, media: Option<MediaType>) -> Message {
        let jid: Jid = "123@s.whatsapp.net".parse().unwrap();
        let mut msg = Message::new_text(
            id.to_string(),
            jid.clone(),
            jid,
            text.to_string(),
            Utc::now(),
            false,
            MessageStatus::Delivered,
            None,
            None,
        );
        msg.media_type = media;
        msg
    }

    #[test]
    fn extract_links_strips_punctuation_and_dedupes() {
        let text = "mrkni (https://example.com/a?b=1), nebo http://foo.cz. \
                    https://example.com/a?b=1 a hotovo";
        assert_eq!(
            extract_links(text),
            vec!["https://example.com/a?b=1".to_string(), "http://foo.cz".to_string()]
        );
    }

    #[test]
    fn extract_links_ignores_plain_words() {
        assert!(extract_links("zadny odkaz tady neni").is_empty());
        assert!(extract_links("ftp://example.com").is_empty());
    }

    #[test]
    fn gallery_splits_messages_into_tabs_newest_first() {
        let messages = vec![
            message("a", "podivej https://example.com", None),
            message("b", "", Some(MediaType::Image)),
            message("c", "faktura", Some(MediaType::Document)),
        ];

        let media = gallery_messages(&messages, GalleryTab::Media);
        assert_eq!(media.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["b"]);

        let docs = gallery_messages(&messages, GalleryTab::Documents);
        assert_eq!(docs.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["c"]);

        let links = gallery_messages(&messages, GalleryTab::Links);
        assert_eq!(links.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["a"]);
    }

    #[test]
    fn document_label_prefers_file_name_then_caption() {
        let mut doc = message("d", "naskenovana faktura", Some(MediaType::Document));
        doc.media_path = Some("C:/media/faktura-2024.pdf".to_string());
        assert_eq!(document_label(&doc), "faktura-2024.pdf");

        doc.media_path = None;
        assert_eq!(document_label(&doc), "naskenovana faktura");
    }
}
