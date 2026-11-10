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
    // Header bar
    show_chat_header(ui, state, chat, backend);
    
    ui.separator();
    
    // Messages area
    show_messages_area(ui, state, chat, messages, chat_jid, backend);
    
    ui.separator();
    
    // Composer
    show_composer(ui, state, chat_jid, backend);
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
                if ui.button(if is_muted { "🔔 Unmute notifications" } else { "🔕 Mute notifications" }).clicked() {
                    let cmd = if is_muted {
                        Command::UnmuteChat { chat_jid: chat.jid.clone() }
                    } else {
                        Command::MuteChat { chat_jid: chat.jid.clone(), until: None }
                    };
                    let _ = backend.send_command(cmd);
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
    let available_height = ui.available_height() - 130.0; // Reserve space for composer
    
    // Check if we should auto-scroll to bottom
    let should_scroll_to_bottom = state.auto_scroll_to_bottom;
    let mut selected_reply = None;
    let mut message_to_delete = None;
    let mut selected_reaction: Option<(String, String)> = None;
    let mut view_reactions_id: Option<String> = None;
    let message_to_download = std::rc::Rc::new(std::cell::RefCell::new(None));
    let to_dl = message_to_download.clone();
    
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
                    let msg_clone = msg.clone();
                    
                    MessageBubble::new(msg, is_own)
                        .show_sender(sender_name_str)
                        .on_reply(|_id| {
                            selected_reply = Some(msg_clone);
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
                        .ui(ui);
                    
                    ui.add_space(2.0);
                }
            }
            
            // Bottom padding for stick_to_bottom
            ui.add_space(24.0);
        });
        
    if let Some(r) = selected_reply {
        state.replying_to = Some(r);
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
                ).on_hover_text("Attach file, photo or video");

                if attach_btn.clicked() {
                    if let Some(path) = crate::media::pick_file() {
                        state.pending_attachment = Some(path);
                    }
                }

                // Text input - takes most space
                let hint = if state.pending_attachment.is_some() {
                    "Add a caption..."
                } else {
                    "Type a message..."
                };
                let input_width = (ui.available_width() - 85.0).max(50.0);
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
                ).on_hover_text("Schedule message for later");

                if schedule_btn.clicked() {
                    state.schedule_target_chat = Some(chat_jid.clone());
                    state.schedule_text = state.composer_text.clone();
                    state.show_schedule_dialog = true;
                }
                
                let send_btn = ui.add(
                    egui::Button::new(RichText::new("➤").font(FontId::proportional(22.0)).color(if can_send { Color32::WHITE } else { Color32::from_rgb(0x86, 0x96, 0xA0) }))
                        .frame(false)
                        .fill(if can_send { Color32::from_rgb(0x00, 0x80, 0x69) } else { Color32::TRANSPARENT })
                ).on_hover_text("Send");
                
                if (enter_pressed || send_btn.clicked()) && can_send {
                    let text = state.composer_text.trim().to_string();
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
                            ui.label(RichText::new(&r.emoji).font(FontId::proportional(22.0)));
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