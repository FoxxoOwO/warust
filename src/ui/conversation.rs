//! Conversation View UI - WhatsApp Web/Desktop style

use egui::{Align, Color32, Context, FontId, Frame, Key, Layout, Margin, RichText, ScrollArea, TextEdit, Ui, Widget};
use crate::state::{AppState, SharedState, Message};
use crate::ui::components::{MessageBubble, Avatar};
use crate::util::format_jid;
use crate::commands::Command;
use whatsapp_rust::wacore_binary::Jid;
use std::borrow::Cow;

/// Conversation panel - main chat area like WhatsApp Web
pub fn show_conversation(ui: &mut Ui, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let mut s = state.write();
    
    if let Some(chat_jid) = s.selected_chat.clone() {
        let canon_jid = s.canonical_jid(&chat_jid);
        let chat = s.chats.get(&canon_jid).or_else(|| s.chats.get(&chat_jid)).cloned();
        let messages = s.get_chat_messages(&canon_jid).cloned().unwrap_or_default();
        
        if let Some(chat) = chat {
            show_chat_view(ui, &mut s, &chat, &messages, &canon_jid, backend);
        }
    } else {
        // No chat selected - show welcome screen
        show_welcome_screen(ui, &s);
    }
}

fn show_welcome_screen(ui: &mut Ui, _state: &AppState) {
    Frame::new()
        .fill(Color32::from_rgb(0x11, 0x1B, 0x21))
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    // WhatsApp logo
                    ui.label(RichText::new("💬").font(FontId::proportional(64.0)));
                    ui.add_space(24.0);
                    
                    ui.label(RichText::new("Warust").font(FontId::proportional(32.0)).strong().color(Color32::WHITE));
                    ui.add_space(8.0);
                    ui.label(RichText::new("WhatsApp Desktop Client").font(FontId::proportional(16.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    
                    ui.add_space(40.0);
                    
                    ui.label(RichText::new("Select a chat from the list").font(FontId::proportional(16.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    ui.add_space(8.0);
                    ui.label(RichText::new("or start a new conversation").font(FontId::proportional(16.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    
                    ui.add_space(40.0);
                    
                    ui.label(RichText::new("End-to-end encrypted").font(FontId::proportional(13.0)).color(Color32::from_rgb(0x53, 0xB6, 0x9A)));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Your messages and calls are secured").font(FontId::proportional(13.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                });
            });
        });
}

fn show_chat_view(ui: &mut Ui, state: &mut AppState, chat: &crate::state::Chat, messages: &[Message], chat_jid: &Jid, backend: &crate::backend::BackendHandle) {
    let selecting = !state.selected_messages.is_empty();

    // The info panel spans the full height on the right, so it has to claim its
    // space before the header and message area do.
    if state.show_chat_info {
        egui::Panel::right("chat_info_panel")
            .resizable(true)
            .default_size(320.0)
            .min_size(260.0)
            .max_size(420.0)
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::chat_list::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .show(ui, |ui| {
                crate::ui::chat_info::show(ui, state, chat, backend);
            });
    }

    // While messages are ticked the header turns into the bulk-action bar and
    // the composer is hidden: sending a draft mid-selection would be surprising.
    if selecting {
        show_selection_bar(ui, state, chat_jid, backend);
    } else {
        show_chat_header(ui, state, chat, backend);
    }

    ui.separator();
    
    // Bottom-up layout pins composer to the absolute bottom of the window (no empty gap)
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        if !selecting {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    show_composer(ui, state, chat_jid, backend);
                },
            );
            ui.separator();
        }
        
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            show_messages_area(ui, state, chat, messages, chat_jid, backend);
        });
    });
}

/// Bulk-action bar shown in place of the chat header while messages are ticked.
fn show_selection_bar(ui: &mut Ui, state: &mut AppState, chat_jid: &Jid, _backend: &crate::backend::BackendHandle) {
    let count = state.selected_messages.len();
    let mut clear = false;
    let mut open_forward = false;

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui.button(RichText::new("✕").font(FontId::proportional(16.0))).clicked() {
            clear = true;
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("Vybráno: {}", count))
                .font(FontId::proportional(15.0))
                .color(Color32::WHITE),
        );

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            let forward_btn = ui.add_enabled(
                count > 0,
                egui::Button::new(
                    RichText::new("↪  Přeposlat")
                        .font(FontId::proportional(13.0))
                        .color(Color32::from_rgb(0x0B, 0x14, 0x18)),
                )
                .fill(crate::theme::ACCENT)
                .corner_radius(egui::CornerRadius::same(6)),
            );
            if forward_btn.clicked() {
                open_forward = true;
            }
        });
    });
    ui.add_space(8.0);

    if clear {
        state.selected_messages.clear();
    } else if open_forward {
        state.forward_dialog = Some(crate::state::ForwardDialogState {
            source_chat: chat_jid.clone(),
            message_ids: state.selected_messages.iter().cloned().collect(),
            search: String::new(),
        });
    }
}

fn show_chat_header(ui: &mut Ui, state: &mut AppState, chat: &crate::state::Chat, backend: &crate::backend::BackendHandle) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        
        // Avatar
        Avatar::new(&chat.name).jid(chat.jid.clone()).size(42.0).ui(ui);
        ui.add_space(12.0);
        
        // Name and status
        ui.vertical(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(&chat.name).font(FontId::proportional(17.0)).strong().color(Color32::WHITE));
            
            if chat.is_currently_typing() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("píše").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x25, 0xD3, 0x66)));
                    crate::ui::animations::bouncing_dots(ui, Color32::from_rgb(0x25, 0xD3, 0x66), 2.5, 3.5);
                    ui.ctx().request_repaint();
                });
            } else {
                let status_text = if chat.is_group {
                    format!("{} participants", chat.participants.len())
                } else if crate::util::is_real_name(&chat.name) {
                    crate::util::format_jid(&chat.jid)
                } else {
                    "".to_string()
                };
                
                if !status_text.is_empty() {
                    ui.label(RichText::new(status_text).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                }
            }
        });
        
        // Right side buttons
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(12.0);
            
            // More menu
            ui.menu_button(RichText::new("⋮").font(FontId::proportional(22.0)), |ui| {
                // Search in this chat
                if ui.button("🔍  Search in this chat").clicked() {
                    state.show_search_dialog = true;
                    state.search_filter.scope = crate::search::SearchScope::CurrentChat;
                    ui.close();
                }

                // Export conversation
                if ui.button("📤  Export conversation").clicked() {
                    state.export_dialog = Some(crate::state::ExportDialogState {
                        chat_jid: chat.jid.clone(),
                        chat_name: chat.name.clone(),
                        format: crate::export::ExportFormat::Txt,
                        status_message: None,
                        exported_path: None,
                    });
                    ui.close();
                }

                // Schedule message
                if ui.button("🕒  Schedule message").clicked() {
                    state.schedule_target_chat = Some(chat.jid.clone());
                    state.schedule_text = state.composer_text.clone();
                    state.show_schedule_dialog = true;
                    ui.close();
                }

                ui.separator();

                if !chat.is_group {
                    if ui.button("✏  Edit contact name").clicked() {
                        state.editing_contact = Some((chat.jid.clone(), chat.name.clone()));
                        ui.close();
                    }
                    ui.separator();
                }
                let is_pinned = chat.is_pinned;
                if ui.button(if is_pinned { "📌 Unpin chat" } else { "📌 Pin chat" }).clicked() {
                    let cmd = if is_pinned {
                        Command::UnpinChat { chat_jid: chat.jid.clone() }
                    } else {
                        Command::PinChat { chat_jid: chat.jid.clone() }
                    };
                    let _ = backend.send_command(cmd);
                    ui.close();
                }
                let is_muted = chat.is_muted();
                // A submenu, like WhatsApp Web: mute is always a choice of
                // duration, not a single toggle.
                ui.menu_button(
                    if is_muted { "🔔  Umlčení" } else { "🔕  Umlčení" },
                    |ui| {
                        if ui.button("Vypnout umlčení").clicked() {
                            let _ = backend.send_command(Command::UnmuteChat {
                                chat_jid: chat.jid.clone(),
                            });
                            ui.close();
                        }
                        for (label, hours) in [("8 hodin", 8i64), ("1 týden", 24 * 7)] {
                            if ui.button(label).clicked() {
                                let until = chrono::Utc::now() + chrono::Duration::hours(hours);
                                let _ = backend.send_command(Command::MuteChat {
                                    chat_jid: chat.jid.clone(),
                                    until: Some(until),
                                });
                                ui.close();
                            }
                        }
                        if ui.button("Vždy").clicked() {
                            let _ = backend.send_command(Command::MuteChat {
                                chat_jid: chat.jid.clone(),
                                until: None,
                            });
                            ui.close();
                        }
                    },
                );
                if ui.button("ℹ  Info o chatu").clicked() {
                    state.show_chat_info = !state.show_chat_info;
                    ui.close();
                }
                if ui.button("🗑 Clear messages").clicked() {
                    let _ = backend.send_command(Command::ClearChat { chat_jid: chat.jid.clone() });
                    ui.close();
                }
            });

            ui.add_space(4.0);

            // Search in chat shortcut
            if ui.add(egui::Button::new(RichText::new("🔍").font(FontId::proportional(18.0)))
                .frame(false)
                .fill(Color32::TRANSPARENT))
                .on_hover_text("Search messages in this chat")
                .clicked() {
                    state.show_search_dialog = true;
                    state.search_filter.scope = crate::search::SearchScope::CurrentChat;
                }
            
            ui.add_space(4.0);

            // Chat info panel toggle
            if ui
                .add(
                    egui::Button::new(RichText::new("ℹ").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(Color32::TRANSPARENT),
                )
                .on_hover_text("Info o chatu")
                .clicked()
            {
                state.show_chat_info = !state.show_chat_info;
            }

            ui.add_space(4.0);

            // Video call
            if ui.add(egui::Button::new(RichText::new("📹").font(FontId::proportional(20.0)))
                .frame(false)
                .fill(Color32::TRANSPARENT))
                .on_hover_text(format!("Videohovor s {}", chat.name))
                .clicked() {
                    let _ = backend.send_command(Command::StartCall {
                        peer_jid: chat.jid.clone(),
                        is_video: true,
                    });
                }
            
            ui.add_space(4.0);
            
            // Voice call
            if ui.add(egui::Button::new(RichText::new("📞").font(FontId::proportional(20.0)))
                .frame(false)
                .fill(Color32::TRANSPARENT))
                .on_hover_text(format!("Hlasový hovor s {}", chat.name))
                .clicked() {
                    let _ = backend.send_command(Command::StartCall {
                        peer_jid: chat.jid.clone(),
                        is_video: false,
                    });
                }
            
            ui.add_space(12.0);
        });
    });
    ui.add_space(8.0);
}

fn show_messages_area(ui: &mut Ui, state: &mut AppState, chat: &crate::state::Chat, messages: &[Message], chat_jid: &Jid, backend: &crate::backend::BackendHandle) {
    let available_height = ui.available_height();
    
    // Check if we should auto-scroll to bottom
    let should_scroll_to_bottom = state.auto_scroll_to_bottom;
    let mut selected_reply: Option<usize> = None;
    let mut selected_edit: Option<String> = None;
    let mut selected_toggle: Option<String> = None;
    let selection_active = !state.selected_messages.is_empty();
    let mut message_to_delete = None;
    let mut selected_reaction: Option<(String, String)> = None;
    let mut view_reactions_id: Option<String> = None;
    let message_to_download = std::rc::Rc::new(std::cell::RefCell::new(None));
    let to_dl = message_to_download.clone();
    let open_emoji_picker_id = std::rc::Rc::new(std::cell::RefCell::new(None));
    let to_open_picker = open_emoji_picker_id.clone();
    
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(should_scroll_to_bottom)
        .max_height(available_height)
        .show(ui, |ui| {
            ui.add_space(8.0);
            
            // On-demand message history button (Peer Data Operation to primary phone)
            ui.vertical_centered(|ui| {
                if ui.button(RichText::new("⟳  Load older messages from phone").font(FontId::proportional(12.0))).clicked() {
                    let _ = backend.send_command(Command::RequestHistorySync {
                        chat_jid: Some(chat_jid.clone()),
                        count: 50,
                    });
                }
            });
            
            ui.add_space(12.0);
            
            if messages.is_empty() {
                show_empty_chat(ui, chat, backend);
            } else {
                let mut last_date: Option<chrono::NaiveDate> = None;
                
                // Cloning the full Message (media bytes included) for every bubble just
                // to support "reply" was wasteful; grab the index instead.
                for (idx, msg) in messages.iter().enumerate() {
                    // Show date separator
                    let msg_date = msg.timestamp.date_naive();
                    let show_date = last_date.map(|ld| ld != msg_date).unwrap_or(true);
                    
                    if show_date {
                        show_date_separator(ui, msg_date);
                        last_date = Some(msg_date);
                    }
                    
                    let is_own = msg.from_me;
                    let show_sender = chat.is_group && !is_own && idx > 0 
                        && messages.get(idx - 1).map(|m| m.sender_jid != msg.sender_jid).unwrap_or(true);
                    
                    let sender_name = if show_sender {
                        state.contacts.get(&msg.sender_jid)
                            .and_then(|c| c.push_name.as_deref())
                            .map(Cow::Borrowed)
                            .or_else(|| {
                                let formatted = format_jid(&msg.sender_jid);
                                Some(Cow::Owned(formatted))
                            })
                    } else {
                        None
                    };
                    
                    let sender_name_str = sender_name.as_deref().unwrap_or("");

                    MessageBubble::new(msg, is_own)
                        .show_sender(sender_name_str)
                        .on_reply(|_id| {
                            selected_reply = Some(idx);
                        })
                        .on_edit(|id| {
                            selected_edit = Some(id.to_string());
                        })
                        .selection_mode(selection_active)
                        .selected(selection_active && state.selected_messages.contains(&msg.id))
                        .on_select(|id| {
                            selected_toggle = Some(id.to_string());
                        })
                        .on_delete(|id| {
                            message_to_delete = Some(id.to_string());
                        })
                        .on_download({
                            let to_dl = to_dl.clone();
                            move |id| {
                                *to_dl.borrow_mut() = Some(id.to_string());
                            }
                        })
                        .on_react(|id, emoji| {
                            selected_reaction = Some((id.to_string(), emoji.to_string()));
                        })
                        .on_view_reactions(|id| {
                            view_reactions_id = Some(id.to_string());
                        })
                        .on_open_emoji_picker({
                            let to_open_picker = to_open_picker.clone();
                            move |id| {
                                *to_open_picker.borrow_mut() = Some(id.to_string());
                            }
                        })
                        .ui(ui);
                    
                    ui.add_space(2.0);
                }
            }
            
            // Bottom padding for stick_to_bottom
            ui.add_space(24.0);
        });
        
    if let Some(id) = selected_toggle {
        state.toggle_selected(&id);
    }

    if let Some(idx) = selected_reply {
        state.replying_to = messages.get(idx).cloned();
    }
    // Entering edit mode loads the original body into the composer and drops any
    // pending reply: an edit rewrites one stanza, it cannot also quote another.
    if let Some(id) = selected_edit {
        if let Some(msg) = messages.iter().find(|m| m.id == id) {
            state.composer_text = msg.text.clone();
            state.replying_to = None;
            state.editing_message = Some(id);
        }
    }
    if let Some(id) = message_to_delete {
        let _ = backend.send_command(Command::DeleteMessageForMe {
            chat_jid: chat_jid.clone(),
            message_id: id,
        });
    }
    if let Some((id, emoji)) = selected_reaction {
        let _ = backend.send_command(Command::SendReaction {
            chat_jid: chat_jid.clone(),
            message_id: id,
            emoji,
        });
    }
    if let Some(id) = view_reactions_id {
        state.reaction_detail_modal = Some((chat_jid.clone(), id));
    }
    if let Some(id) = open_emoji_picker_id.borrow().clone() {
        state.reaction_picker_target = Some(id);
        state.show_emoji_picker = true;
    }
    let to_download = message_to_download.borrow().clone();
    if let Some(id) = to_download {
        let _ = backend.send_command(Command::DownloadMedia {
            chat_jid: chat_jid.clone(),
            message_id: id,
        });
    }
}

fn show_date_separator(ui: &mut Ui, date: chrono::NaiveDate) {
    let today = chrono::Utc::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);
    
    let label = if date == today {
        "Today".to_string()
    } else if date == yesterday {
        "Yesterday".to_string()
    } else {
        date.format("%A, %B %d, %Y").to_string()
    };
    
    ui.vertical_centered(|ui| {
        Frame::new()
            .fill(Color32::from_rgba_premultiplied(0, 0, 0, 80))
            .inner_margin(Margin::symmetric(16, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.label(RichText::new(label).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
            });
    });
    ui.add_space(12.0);
}

fn show_empty_chat(ui: &mut Ui, chat: &crate::state::Chat, backend: &crate::backend::BackendHandle) {
    ui.centered_and_justified(|ui| {
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("💬").font(FontId::proportional(48.0)));
            ui.add_space(16.0);
            ui.label(RichText::new(format!("Start messaging {}", chat.name)).font(FontId::proportional(18.0)).strong().color(Color32::WHITE));
            ui.add_space(8.0);
            ui.label(RichText::new("Messages and calls are end-to-end encrypted").font(FontId::proportional(13.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
            ui.add_space(4.0);
            ui.label(RichText::new("No one outside of this chat can read or listen to them").font(FontId::proportional(13.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
            ui.add_space(16.0);
            if ui.add(
                egui::Button::new(RichText::new("⟳  Načíst zprávy z telefonu").font(FontId::proportional(13.0)).color(Color32::WHITE))
                    .fill(Color32::from_rgb(0x00, 0xA8, 0x84))
                    .corner_radius(egui::CornerRadius::same(6))
            ).clicked() {
                let _ = backend.send_command(Command::RequestHistorySync {
                    chat_jid: Some(chat.jid.clone()),
                    count: 50,
                });
            }
        });
    });
}

/// Recording bar shown in place of the composer while a voice note is captured.
fn show_recorder_bar(
    ui: &mut Ui,
    state: &mut AppState,
    chat_jid: &Jid,
    backend: &crate::backend::BackendHandle,
) {
    let elapsed = std::time::Duration::from_millis(crate::recorder::elapsed_ms());
    Frame::new()
        .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Blink the dot twice a second so it reads as "live".
                let on = (elapsed.as_millis() / 500) % 2 == 0;
                let dot = if on {
                    Color32::from_rgb(0xE6, 0x4A, 0x4A)
                } else {
                    Color32::from_rgb(0x7A, 0x2B, 0x2B)
                };
                ui.label(RichText::new("●").font(FontId::proportional(16.0)).color(dot));
                ui.label(
                    RichText::new(crate::audio::format_duration(elapsed))
                        .font(FontId::proportional(15.0))
                        .color(Color32::WHITE),
                );
                ui.label(
                    RichText::new("Nahrávání…")
                        .font(FontId::proportional(12.0))
                        .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let send = ui.add(
                        egui::Button::new(
                            RichText::new("➤")
                                .font(FontId::proportional(22.0))
                                .color(Color32::WHITE),
                        )
                        .frame(false)
                        .fill(Color32::from_rgb(0x00, 0x80, 0x69)),
                    );
                    if send.on_hover_text("Odeslat hlasovou zprávu").clicked() {
                        match crate::recorder::finish() {
                            Ok(recording) => {
                                state.recorder_error = None;
                                let _ = backend.send_command(Command::SendVoiceNote {
                                    chat_jid: chat_jid.clone(),
                                    file_path: recording.path,
                                    duration_seconds: (recording.duration_ms / 1000).max(1) as u32,
                                    waveform: recording.waveform,
                                });
                            }
                            Err(message) => state.recorder_error = Some(message),
                        }
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("✕").font(FontId::proportional(18.0)),
                            )
                            .frame(false),
                        )
                        .on_hover_text("Zahodit nahrávku")
                        .clicked()
                    {
                        crate::recorder::cancel();
                    }
                });
            });
        });
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(200));
}

fn show_composer(ui: &mut Ui, state: &mut AppState, chat_jid: &Jid, backend: &crate::backend::BackendHandle) {
    ui.add_space(4.0);

    // Pending scheduled messages for this chat indicator banner
    let pending_count = state.scheduled_messages.iter()
        .filter(|m| m.chat_jid == *chat_jid && m.status == crate::state::ScheduledStatus::Pending)
        .count();
    if pending_count > 0 {
        Frame::new()
            .fill(Color32::from_rgba_premultiplied(0, 168, 132, 25))
            .inner_margin(Margin::symmetric(14, 4))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("🕒 {} scheduled message(s) for this chat", pending_count))
                        .font(FontId::proportional(11.0)).color(Color32::from_rgb(0x25, 0xD3, 0x66)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("View").font(FontId::proportional(11.0))).clicked() {
                            state.show_scheduled_manager = true;
                        }
                    });
                });
            });
        ui.add_space(2.0);
    }
    
    // Edit banner: an active edit owns the composer until it is sent or cancelled.
    let mut cancel_edit = false;
    if state.editing_message.is_some() {
        Frame::new()
            .fill(Color32::from_rgb(0x1A, 0x26, 0x2C))
            .inner_margin(Margin::symmetric(14, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("✎").font(FontId::proportional(14.0)).color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                    ui.label(RichText::new("Upravit zprávu").font(FontId::proportional(12.0)).strong().color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            cancel_edit = true;
                        }
                    });
                });
            });
    }
    if cancel_edit {
        state.cancel_edit();
    }

    // Replying preview bar if replying to a message
    let mut cancel_reply = false;
    if let Some(reply_msg) = &state.replying_to {
        let snippet = crate::util::truncate(&reply_msg.text, 50);
        let sender_name = if reply_msg.from_me {
            "You".to_string()
        } else {
            state.contacts.get(&reply_msg.sender_jid)
                .and_then(|c| c.push_name.clone())
                .unwrap_or_else(|| crate::util::format_jid(&reply_msg.sender_jid))
        };
        
        Frame::new()
            .fill(Color32::from_rgb(0x1A, 0x26, 0x2C))
            .inner_margin(Margin::symmetric(14, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("↳").font(FontId::proportional(14.0)).color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(sender_name).font(FontId::proportional(11.0)).strong().color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                        ui.label(RichText::new(snippet).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            cancel_reply = true;
                        }
                    });
                });
            });
    }
    if cancel_reply {
        state.replying_to = None;
    }
    
    // Check for drag-and-dropped files
    let dropped_path = ui.input(|i| {
        i.raw.dropped_files.first()
            .map(|f| f.path().to_path_buf())
            .filter(|p| !p.as_os_str().is_empty())
    });
    if let Some(path) = dropped_path {
        state.pending_attachment = Some(path);
    }

    // Attachment preview banner if a file is attached
    let mut cancel_attachment = false;
    if let Some(path) = &state.pending_attachment {
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("Attachment");
        let file_size_str = std::fs::metadata(path)
            .map(|m| crate::media::format_file_size(m.len()))
            .unwrap_or_default();
        
        Frame::new()
            .fill(Color32::from_rgb(0x1A, 0x26, 0x2C))
            .inner_margin(Margin::symmetric(14, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📎").font(FontId::proportional(14.0)).color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(file_name).font(FontId::proportional(12.0)).strong().color(Color32::WHITE));
                        if !file_size_str.is_empty() {
                            ui.label(RichText::new(file_size_str).font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            cancel_attachment = true;
                        }
                    });
                });
            });
        ui.add_space(2.0);
    }
    if cancel_attachment {
        state.pending_attachment = None;
    }

    if let Some(message) = state.recorder_error.clone() {
        Frame::new()
            .fill(Color32::from_rgb(0x3A, 0x1E, 0x22))
            .inner_margin(Margin::symmetric(14, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("⚠ Mikrofon: {message}"))
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(0xF2, 0x8B, 0x82)),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            state.recorder_error = None;
                        }
                    });
                });
            });
        ui.add_space(2.0);
    }

    // While a take is running the composer row is replaced by the recorder bar,
    // so a stray Enter cannot send text halfway through a voice note.
    if crate::recorder::is_recording() {
        show_recorder_bar(ui, state, chat_jid, backend);
        return;
    }

    Frame::new()
        .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Attach file button
                let attach_btn = ui.add(
                    egui::Button::new(RichText::new("📎").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(Color32::TRANSPARENT)
                ).on_hover_text("Připojit soubor, foto nebo video");

                if attach_btn.clicked() {
                    if let Some(path) = crate::media::pick_file() {
                        state.pending_attachment = Some(path);
                    }
                }

                // Emoji picker button
                let emoji_btn = ui.add(
                    egui::Button::new(RichText::new("😊").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(if state.show_emoji_picker && state.reaction_picker_target.is_none() { Color32::from_rgb(0x2A, 0x39, 0x42) } else { Color32::TRANSPARENT })
                ).on_hover_text("Vybrat emoji");

                if emoji_btn.clicked() {
                    state.show_emoji_picker = !state.show_emoji_picker;
                    state.reaction_picker_target = None;
                }

                // Text input - takes most space
                let hint = if state.editing_message.is_some() {
                    "Upravit zprávu..."
                } else if state.pending_attachment.is_some() {
                    "Přidat popisek..."
                } else {
                    "Napište zprávu..."
                };
                let input_width = (ui.available_width() - 115.0).max(50.0);
                let response = ui.add(
                    TextEdit::multiline(&mut state.composer_text)
                        .hint_text(hint)
                        .desired_rows(1)
                        .desired_width(input_width)
                        .font(FontId::proportional(15.0))
                        .margin(egui::vec2(12.0, 10.0))
                );
                
                // Handle Enter to send, Shift+Enter for newline
                let enter_pressed = response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);
                let can_send = state.pending_attachment.is_some() || !state.composer_text.trim().is_empty();

                // Schedule message button
                let schedule_btn = ui.add(
                    egui::Button::new(RichText::new("🕒").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(Color32::TRANSPARENT)
                ).on_hover_text("Naplánovat odeslání zprávy");

                if schedule_btn.clicked() {
                    state.schedule_target_chat = Some(chat_jid.clone());
                    state.schedule_text = state.composer_text.clone();
                    state.show_schedule_dialog = true;
                }
                
                // WhatsApp swaps the send arrow for a microphone while the box
                // is empty, so recording never needs a second control.
                let send_btn = if can_send {
                    ui.add(
                        egui::Button::new(RichText::new("➤").font(FontId::proportional(22.0)).color(Color32::WHITE))
                            .frame(false)
                            .fill(Color32::from_rgb(0x00, 0x80, 0x69))
                    ).on_hover_text("Send")
                } else {
                    ui.add(
                        egui::Button::new(RichText::new("🎤").font(FontId::proportional(20.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)))
                            .frame(false)
                            .fill(Color32::TRANSPARENT)
                    ).on_hover_text("Nahrát hlasovou zprávu")
                };

                if !can_send && send_btn.clicked() {
                    match crate::recorder::start() {
                        Ok(()) => ui.ctx().request_repaint(),
                        Err(message) => state.recorder_error = Some(message),
                    }
                }
                
                if (enter_pressed || send_btn.clicked()) && can_send {
                    let text = state.composer_text.trim().to_string();

                    // An active edit rewrites the original stanza instead of
                    // sending a second one, so no reply/attachment is involved.
                    if let Some(message_id) = state.editing_message.clone() {
                        if !text.is_empty() {
                            let _ = backend.send_command(Command::EditText {
                                to: chat_jid.clone(),
                                message_id,
                                text,
                            });
                        }
                        state.editing_message = None;
                        state.composer_text.clear();
                        return;
                    }

                    let reply_to = state.replying_to.take().map(|m| m.id);
                    
                    if let Some(file_path) = state.pending_attachment.take() {
                        let caption = if text.is_empty() { None } else { Some(text) };
                        let _ = backend.send_command(Command::SendMedia {
                            chat_jid: chat_jid.clone(),
                            file_path,
                            caption,
                            reply_to,
                        });
                    } else {
                        let _ = backend.send_command(Command::SendText { 
                            to: chat_jid.clone(), 
                            text, 
                            reply_to,
                        });
                    }
                    state.composer_text.clear();
                }
            });
        });
    
    // Emoji Picker Window / Popup
    if state.show_emoji_picker {
        let is_reaction = state.reaction_picker_target.is_some();
        let title = if is_reaction { "❤️  Vybrat reakci" } else { "😊  Emoji" };
        let mut open = true;
        let mut picked_emoji = None;

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::vec2(340.0, 360.0))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -80.0))
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                crate::ui::emoji::show_emoji_picker(ui, state, |emoji| {
                    picked_emoji = Some(emoji.to_string());
                });
            });

        if !open {
            state.show_emoji_picker = false;
            state.reaction_picker_target = None;
        }

        if let Some(emoji) = picked_emoji {
            if let Some(target_msg_id) = state.reaction_picker_target.take() {
                let _ = backend.send_command(Command::SendReaction {
                    chat_jid: chat_jid.clone(),
                    message_id: target_msg_id,
                    emoji,
                });
            } else {
                state.composer_text.push_str(&emoji);
            }
            state.show_emoji_picker = false;
        }
    }

    ui.add_space(6.0);
}

/// Modal dialog showing details of who reacted to a message
pub fn show_reaction_detail_modal(ctx: &Context, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let (chat_jid, msg_id, reactions) = {
        let s = state.read();
        if let Some((ref chat_jid, ref msg_id)) = s.reaction_detail_modal {
            let canon = s.canonical_jid(chat_jid);
            if let Some(msgs) = s.get_chat_messages(&canon) {
                if let Some(msg) = msgs.iter().find(|m| &m.id == msg_id) {
                    (chat_jid.clone(), msg_id.clone(), msg.reactions.clone())
                } else {
                    return;
                }
            } else {
                return;
            }
        } else {
            return;
        }
    };

    let mut is_open = true;
    let mut should_close = false;
    let mut reaction_to_remove = false;

    egui::Window::new("❤️  Reakce na zprávu")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::vec2(360.0, 300.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut is_open)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            if reactions.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new("Tato zpráva zatím nemá žádné reakce.").color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                });
            } else {
                let self_jid = state.read().self_jid.clone();
                
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Celkem reakcí: {}", reactions.len())).font(FontId::proportional(13.0)).strong());
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for r in &reactions {
                        let is_mine = self_jid.as_ref().map_or(false, |sj| sj.user == r.sender.user);
                        let name = {
                            let s = state.read();
                            s.resolve_display_name(&r.sender).unwrap_or_else(|| {
                                if is_mine { "Vy".to_string() } else { crate::util::format_jid(&r.sender) }
                            })
                        };
                        let name_display = if is_mine && name != "Vy" {
                            format!("{} (Vy)", name)
                        } else {
                            name
                        };

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&r.emoji).font(crate::theme::message_font(22.0)));
                            ui.add_space(8.0);
                            ui.label(RichText::new(name_display).font(FontId::proportional(14.0)).color(Color32::from_rgb(0xE9, 0xED, 0xEF)));
                            if is_mine {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button(RichText::new("Odebrat").color(Color32::from_rgb(0xFF, 0x6B, 0x6B))).clicked() {
                                        reaction_to_remove = true;
                                    }
                                });
                            }
                        });
                        ui.add_space(6.0);
                    }
                });
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Zavřít").clicked() {
                    should_close = true;
                }
            });
        });

    if reaction_to_remove {
        let _ = backend.send_command(Command::SendReaction {
            chat_jid: chat_jid.clone(),
            message_id: msg_id.clone(),
            emoji: String::new(),
        });
        should_close = true;
    }

    if should_close || !is_open {
        state.write().reaction_detail_modal = None;
    }
}