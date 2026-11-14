//! Chat List UI - WhatsApp Web/Desktop style

use egui::{Align, Color32, FontId, Frame, Layout, Margin, RichText, ScrollArea, TextEdit, Ui, Widget};

use crate::state::{AppState, Chat, ConnectionStatus, SharedState};
use crate::theme::chat_list;
use crate::ui::components::{Avatar, ChatItem};

/// Chat list panel - left sidebar like WhatsApp Web
pub fn show_chat_list(ui: &mut Ui, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let mut s = state.write();

    // Panel frame with WhatsApp background
    Frame::new()
        .fill(chat_list::BACKGROUND)
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // Header bar with search
                let jump_requested = show_header(ui, &mut s, backend);

                ui.separator();

                // Connection status indicator
                show_connection_status(ui, &s, backend);

                let chats = s.filtered_chats();
                let archived = s.archived_chats();
                let show_archived = s.show_archived;
                let mut toggle_archived = false;
                let mut actions = ListActions::default();

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                    .show(ui, |ui| {
                        ui.add_space(4.0);

                        if chats.is_empty() {
                            show_empty_state(ui);
                        } else {
                            for chat in &chats {
                                let is_selected = s.selected_chat.as_ref() == Some(&chat.jid);
                                let draft = s.drafts.get(&chat.jid).map(|d| d.as_str());
                                show_chat_row(ui, chat, is_selected, draft, backend, &mut actions);
                                ui.add_space(1.0);
                            }
                        }

                        // Archived chats keep out of the way until asked for.
                        if !archived.is_empty() {
                            ui.add_space(6.0);
                            let arrow = if show_archived { "▾" } else { "▸" };
                            let label = format!("{arrow}  Archivované ({})", archived.len());
                            let toggle = ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(label).font(FontId::proportional(13.0)),
                                    )
                                    .frame(false)
                                    .fill(Color32::TRANSPARENT),
                                )
                                .on_hover_text("Zobrazit archivované chaty");
                            if toggle.clicked() {
                                toggle_archived = true;
                            }

                            if show_archived {
                                ui.add_space(2.0);
                                for chat in &archived {
                                    let is_selected = s.selected_chat.as_ref() == Some(&chat.jid);
                                    let draft = s.drafts.get(&chat.jid).map(|d| d.as_str());
                                    show_chat_row(ui, chat, is_selected, draft, backend, &mut actions);
                                    ui.add_space(1.0);
                                }
                            }
                        }

                        ui.add_space(8.0);
                    });

                if toggle_archived {
                    s.show_archived = !show_archived;
                }

                if let Some(req) = actions.confirm.take() {
                    s.confirm = Some(req);
                }
                if let Some((jid, name)) = actions.edit_contact.take() {
                    s.editing_contact = Some((jid, name));
                }
                if let Some((jid, name)) = actions.export_chat.take() {
                    s.export_dialog = Some(crate::state::ExportDialogState {
                        chat_jid: jid,
                        chat_name: name,
                        format: crate::export::ExportFormat::Txt,
                        status_message: None,
                        exported_path: None,
                    });
                }
                if let Some(jid) = actions.schedule_chat.take() {
                    s.schedule_target_chat = Some(jid);
                    s.show_schedule_dialog = true;
                }

                // "Contact info" selects the chat first, then opens the panel.
                let info_requested = actions.info_chat.is_some();
                if actions.clicked.is_none() {
                    actions.clicked = actions.info_chat.take();
                }

                if let Some(jid) = actions.clicked.take() {
                    crate::ui::open_chat(&mut s, &jid, backend);
                }

                if jump_requested {
                    if let Some(target) = s.next_unread_chat(s.selected_chat.clone().as_ref()) {
                        let message_id = s.first_unread_message_id(&target);
                        crate::ui::open_chat(&mut s, &target, backend);
                        if let Some(id) = message_id {
                            s.jump_target = Some(crate::state::JumpTarget::new(id));
                        }
                    }
                }

                if info_requested {
                    s.show_chat_info = true;
                }
            });
        });


    // Modal dialog for editing contact name
    if let Some((jid, current_name)) = &mut s.editing_contact {
        let mut close = false;
        let mut save = None;
        egui::Window::new("✏  Edit contact name")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("Enter custom name for this contact:").font(FontId::proportional(13.0)));
                ui.add_space(6.0);
                let response = ui.add(TextEdit::singleline(current_name).desired_width(260.0));
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    save = Some((jid.clone(), current_name.clone()));
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        save = Some((jid.clone(), current_name.clone()));
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });

        if let Some((jid, name)) = save {
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() {
                let _ = backend.send_command(crate::commands::Command::SaveContact {
                    jid,
                    name: trimmed,
                });
            }
            s.editing_contact = None;
        } else if close {
            s.editing_contact = None;
        }
    }
}

/// Header with search and quick actions. Returns true when the user asked to
/// jump to the next chat with unread messages.
fn show_header(ui: &mut Ui, state: &mut AppState, backend: &crate::backend::BackendHandle) -> bool {
    let mut jump_requested = false;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        
        // Profile avatar/button (top left)
        if let Some(self_jid) = &state.self_jid {
            let name = state.self_name.as_deref().unwrap_or("You");
            Avatar::new(name)
                .jid(self_jid.to_non_ad())
                .size(36.0)
                .ui(ui);
        } else {
            Avatar::new("WA")
                .size(36.0)
                .ui(ui);
        }
        
        ui.add_space(8.0);
        
        // Search bar
        let search_width = (ui.available_width() - 44.0).clamp(60.0, 360.0);
        let search_response = ui.add(
            TextEdit::singleline(&mut state.search_query)
                .hint_text("Search chats...")
                .desired_width(search_width)
                .font(FontId::proportional(14.0))
                .margin(egui::vec2(10.0, 8.0))
        );
        
        if search_response.changed() {
            // Search triggers automatically via filtered_chats()
        }
        
        // Refresh / Sync contacts button (🔄)
        let refresh_btn = ui.add(
            egui::Button::new(RichText::new("🔄").font(FontId::proportional(15.0)))
                .frame(true)
        ).on_hover_text("Synchronizovat kontakty a názvy z WhatsAppu");
        
        if refresh_btn.clicked() {
            let _ = backend.send_command(crate::commands::Command::RefreshChats);
        }

        // New Chat button (+)
        let new_chat_btn = ui.add(
            egui::Button::new(RichText::new("➕").font(FontId::proportional(16.0)))
                .frame(true)
        ).on_hover_text("Start new chat");
        
        if new_chat_btn.clicked() {
            state.show_new_chat_dialog = !state.show_new_chat_dialog;
            state.new_chat_error = None;
        }

        // Unread backlog: one click walks the chats that still owe an answer.
        let unread_total: u32 = state
            .chats
            .values()
            .filter(|c| !c.is_archived)
            .map(|c| c.unread_count)
            .sum();
        if unread_total > 0 {
            let next_btn = ui
                .add(
                    egui::Button::new(
                        RichText::new(format!("⬇ {}", unread_total))
                            .font(FontId::proportional(13.0)),
                    )
                    .frame(true),
                )
                .on_hover_text("Skočit na další nepřečtenou zprávu (Alt+↓)");
            if next_btn.clicked()
                || ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowDown))
            {
                jump_requested = true;
            }
        }

        ui.add_space(8.0);
    });
    
    // New chat drawer if opened
    if state.show_new_chat_dialog {
        ui.add_space(4.0);
        Frame::new()
            .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
            .inner_margin(Margin::same(10))
            .corner_radius(egui::CornerRadius::same(8))
            .show(ui, |ui| {
                ui.label(RichText::new("Nová konverzace").font(FontId::proportional(13.0)).color(Color32::WHITE));
                ui.add_space(6.0);
                // The two ways of starting a conversation that are not a phone
                // number live here so the ➕ button covers everything.
                ui.horizontal(|ui| {
                    if ui.button("👥  Nová skupina").clicked() {
                        state.new_group = Some(crate::state::NewGroupState::default());
                        state.show_new_chat_dialog = false;
                    }
                    if ui.button("📇  Kontakty").clicked() {
                        state.show_contacts_dialog = true;
                        state.show_new_chat_dialog = false;
                    }
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                // Joining by link lives here because it needs no contact.
                ui.label(RichText::new("Připojit se do skupiny").font(FontId::proportional(13.0)).color(Color32::WHITE));
                ui.add_space(4.0);
                let mut join_requested = false;
                ui.horizontal(|ui| {
                    let input = ui.add(
                        TextEdit::singleline(&mut state.join_link_input)
                            .hint_text("https://chat.whatsapp.com/…")
                            .desired_width(ui.available_width() - 80.0)
                            .font(FontId::proportional(13.0))
                    );
                    if input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        join_requested = true;
                    }
                    if ui.button("Připojit").clicked() {
                        join_requested = true;
                    }
                });
                if join_requested {
                    let code = state.join_link_input.trim().to_string();
                    if code.is_empty() {
                        state.new_chat_error = Some("Zadej odkaz nebo kód pozvánky".to_string());
                    } else {
                        let _ = backend.send_command(crate::commands::Command::JoinGroup { invite_code: code });
                        state.join_link_input.clear();
                        state.new_chat_error = None;
                        state.show_new_chat_dialog = false;
                    }
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(RichText::new("Start chat with phone number:").font(FontId::proportional(13.0)).color(Color32::WHITE));
                ui.add_space(4.0);
                
                let mut start_requested = false;
                ui.horizontal(|ui| {
                    let input_resp = ui.add(
                        TextEdit::singleline(&mut state.new_chat_input)
                            .hint_text("+420 123 456 789")
                            .desired_width(ui.available_width() - 70.0)
                            .font(FontId::proportional(13.0))
                    );
                    if input_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        start_requested = true;
                    }
                    if ui.button("Chat").clicked() {
                        start_requested = true;
                    }
                });
                
                if start_requested {
                    if let Some(target_jid) = crate::util::parse_jid(&state.new_chat_input) {
                        let name = crate::util::format_jid(&target_jid);
                        let _ = state.get_or_create_chat(target_jid.clone(), name, false);
                        state.selected_chat = Some(target_jid.clone());
                        state.selected_messages.clear();
                        state.show_new_chat_dialog = false;
                        state.new_chat_input.clear();
                        state.new_chat_error = None;
                        let _ = backend.send_command(crate::commands::Command::SelectChat { chat_jid: Some(target_jid) });
                    } else {
                        state.new_chat_error = Some("Invalid phone number or JID".to_string());
                    }
                }
                
                if let Some(err) = &state.new_chat_error {
                    ui.add_space(2.0);
                    ui.colored_label(Color32::from_rgb(0xFF, 0x6B, 0x6B), err);
                }
            });
    }

    ui.add_space(8.0);
    jump_requested
}

fn show_connection_status(ui: &mut Ui, state: &AppState, backend: &crate::backend::BackendHandle) {
    match &state.connection {
        ConnectionStatus::Connecting => {
            ui.horizontal_centered(|ui| {
                ui.spinner();
                ui.label(RichText::new("Connecting...").color(Color32::from_rgb(0x86, 0x96, 0xA0)));
            });
            ui.add_space(8.0);
            ui.separator();
        }
        ConnectionStatus::QrCode(_) | ConnectionStatus::PairCode(_) => {
            // Handled by QR pairing screen
        }
        ConnectionStatus::Syncing(_) => {
            // Handled as non-intrusive floating popup window in app.rs so it never displaces chats
        }
        ConnectionStatus::Error(err) => {
            Frame::new()
                .fill(Color32::from_rgba_premultiplied(0xF1, 0x5C, 0x5C, 30))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(0xF1, 0x5C, 0x5C), format!("Error: {}", err));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Retry").clicked() {
                                let _ = backend.send_command(crate::commands::Command::Connect);
                            }
                        });
                    });
                });
            ui.add_space(4.0);
            ui.separator();
        }
        _ => {}
    }
}

fn show_empty_state(ui: &mut Ui) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new("💬").font(FontId::proportional(36.0)));
            ui.add_space(12.0);
            ui.label(RichText::new("No chats yet").font(FontId::proportional(16.0)).strong().color(Color32::WHITE));
            ui.add_space(6.0);
            ui.label(RichText::new("Click ➕ above to start a conversation").font(FontId::proportional(13.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
        });
    });
}

fn show_chat_context_menu(
    ui: &mut Ui,
    chat: &Chat,
    backend: &crate::backend::BackendHandle,
    edit_contact_req: &mut Option<(whatsapp_rust::wacore_binary::Jid, String)>,
    export_chat_req: &mut Option<(whatsapp_rust::wacore_binary::Jid, String)>,
    schedule_chat_req: &mut Option<whatsapp_rust::wacore_binary::Jid>,
    info_chat_req: &mut Option<whatsapp_rust::wacore_binary::Jid>,
    confirm_req: &mut Option<crate::state::ConfirmRequest>,
) {
    let jid = chat.jid.clone();

    // Export conversation
    if ui.selectable_label(false, "📤  Export conversation").clicked() {
        *export_chat_req = Some((jid.clone(), chat.name.clone()));
        ui.close();
    }

    // Schedule message
    if ui.selectable_label(false, "🕒  Schedule message").clicked() {
        *schedule_chat_req = Some(jid.clone());
        ui.close();
    }

    ui.separator();

    // Edit contact name
    if !chat.is_group {
        if ui.selectable_label(false, "✏  Edit contact name").clicked() {
            *edit_contact_req = Some((jid.clone(), chat.name.clone()));
            ui.close();
        }
        ui.separator();
    }
    
    // Pin/Unpin
    if ui.selectable_label(chat.is_pinned, if chat.is_pinned { "📌  Unpin" } else { "📌  Pin" }).clicked() {
        let _ = backend.send_command(if chat.is_pinned {
            crate::commands::Command::UnpinChat { chat_jid: jid.clone() }
        } else {
            crate::commands::Command::PinChat { chat_jid: jid.clone() }
        });
        ui.close();
    }
    
    // Archive/Unarchive
    if ui.selectable_label(chat.is_archived, if chat.is_archived { "📦  Unarchive" } else { "📦  Archive" }).clicked() {
        let _ = backend.send_command(if chat.is_archived {
            crate::commands::Command::UnarchiveChat { chat_jid: jid.clone() }
        } else {
            crate::commands::Command::ArchiveChat { chat_jid: jid.clone() }
        });
        ui.close();
    }
    
    // Mute notifications, with the duration choices WhatsApp offers.
    ui.menu_button(
        if chat.is_muted() { "🔔  Umlčení" } else { "🔕  Umlčení" },
        |ui| {
            if ui
                .selectable_label(!chat.is_muted(), "Vypnout umlčení")
                .clicked()
            {
                let _ = backend.send_command(crate::commands::Command::UnmuteChat {
                    chat_jid: jid.clone(),
                });
                ui.close();
            }
            for (label, hours) in [("8 hodin", 8i64), ("1 týden", 24 * 7)] {
                if ui.selectable_label(false, label).clicked() {
                    let until = chrono::Utc::now() + chrono::Duration::hours(hours);
                    let _ = backend.send_command(crate::commands::Command::MuteChat {
                        chat_jid: jid.clone(),
                        until: Some(until),
                    });
                    ui.close();
                }
            }
            if ui.selectable_label(false, "Vždy").clicked() {
                let _ = backend.send_command(crate::commands::Command::MuteChat {
                    chat_jid: jid.clone(),
                    until: None,
                });
                ui.close();
            }
        },
    );
    
    ui.separator();
    
    // Mark as read/unread
    if chat.unread_count > 0 {
        if ui.selectable_label(false, "✓  Mark as read").clicked() {
            let _ = backend.send_command(crate::commands::Command::MarkChatAsRead { chat_jid: jid.clone() });
            ui.close();
        }
    }
    
    ui.separator();
    
    // Delete chat. Destructive, so it asks first.
    if ui.selectable_label(false, "🗑  Delete chat").clicked() {
        *confirm_req = Some(crate::ui::confirm_dialog::request(
            format!("Smazat chat {}?", chat.name),
            "Chat zmizí ze seznamu; zprávy zůstanou na serveru.",
            "Smazat",
            crate::state::ConfirmAction::DeleteChat {
                chat_jid: jid.clone(),
            },
        ));
        ui.close();
    }
    
    // Clear messages
    if ui.selectable_label(false, "🧹  Clear messages").clicked() {
        *confirm_req = Some(crate::ui::confirm_dialog::request(
            format!("Vymazat zprávy v chatu {}?", chat.name),
            "Historie zmizí jen u tebe, ostatní ji pořád uvidí.",
            "Vymazat",
            crate::state::ConfirmAction::ClearChat {
                chat_jid: jid.clone(),
            },
        ));
        ui.close();
    }
    
    ui.separator();
    
    // Contact info
    if ui.selectable_label(false, "ℹ  Contact info").clicked() {
        *info_chat_req = Some(jid.clone());
        ui.close();
    }
}


/// What a row's interactions asked for, collected so the list can be drawn in a
/// loop while the state is only touched afterwards.
#[derive(Default)]
struct ListActions {
    clicked: Option<whatsapp_rust::wacore_binary::Jid>,
    edit_contact: Option<(whatsapp_rust::wacore_binary::Jid, String)>,
    export_chat: Option<(whatsapp_rust::wacore_binary::Jid, String)>,
    schedule_chat: Option<whatsapp_rust::wacore_binary::Jid>,
    info_chat: Option<whatsapp_rust::wacore_binary::Jid>,
    confirm: Option<crate::state::ConfirmRequest>,
}

/// One row of the chat list, identical in the main and archived sections.
fn show_chat_row(
    ui: &mut Ui,
    chat: &Chat,
    is_selected: bool,
    draft: Option<&str>,
    backend: &crate::backend::BackendHandle,
    actions: &mut ListActions,
) {
    let response = ChatItem::new(chat, is_selected).draft(draft).ui(ui);

    if response.clicked() {
        actions.clicked = Some(chat.jid.clone());
    }

    response.context_menu(|ui| {
        show_chat_context_menu(
            ui,
            chat,
            backend,
            &mut actions.edit_contact,
            &mut actions.export_chat,
            &mut actions.schedule_chat,
            &mut actions.info_chat,
            &mut actions.confirm,
        );
    });
}
