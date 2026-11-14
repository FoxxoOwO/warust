//! Message Search with Filters Modal Dialog

use egui::{Align, Color32, FontId, Frame, Layout, Margin, RichText, ScrollArea, TextEdit};
use crate::search::{search_messages, SearchContentType, SearchDateRange, SearchScope, SearchSender};
use crate::state::SharedState;
use whatsapp_rust::wacore_binary::Jid;

pub fn show_search_dialog(ctx: &egui::Context, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let mut s = state.write();
    if !s.show_search_dialog {
        return;
    }

    let mut close = false;
    // (chat, message) of the result the user clicked; the message id is what
    // makes the conversation jump to it instead of opening at the bottom.
    let mut selected_chat_to_open: Option<(Jid, String)> = None;

    let results = search_messages(&s, &s.search_filter);

    egui::Window::new("🔍  Search in Messages")
        .collapsible(false)
        .resizable(true)
        .default_width(580.0)
        .default_height(520.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.add_space(4.0);

            // Search query input bar
            ui.horizontal(|ui| {
                let query_resp = ui.add(
                    TextEdit::singleline(&mut s.search_filter.query)
                        .hint_text("Search message text...")
                        .desired_width(ui.available_width() - 80.0)
                        .font(FontId::proportional(14.0))
                        .margin(egui::vec2(8.0, 8.0))
                );

                if query_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }

                if ui.button("Clear").clicked() {
                    s.search_filter.query.clear();
                }
            });

            ui.add_space(8.0);

            // Filter row 1: Scope & Sender
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Scope:").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                ui.selectable_value(&mut s.search_filter.scope, SearchScope::CurrentChat, "📌 Current chat");
                ui.selectable_value(&mut s.search_filter.scope, SearchScope::AllChats, "🌐 All chats");

                ui.add_space(16.0);

                ui.label(RichText::new("Sender:").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                ui.selectable_value(&mut s.search_filter.sender, SearchSender::All, "All");
                ui.selectable_value(&mut s.search_filter.sender, SearchSender::SentByMe, "↗ Sent by me");
                ui.selectable_value(&mut s.search_filter.sender, SearchSender::Received, "↙ Received");
            });

            ui.add_space(6.0);

            // Filter row 2: Content type & Date range
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Type:").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                ui.selectable_value(&mut s.search_filter.content_type, SearchContentType::All, "All");
                ui.selectable_value(&mut s.search_filter.content_type, SearchContentType::TextOnly, "💬 Text");
                ui.selectable_value(&mut s.search_filter.content_type, SearchContentType::MediaOnly, "📷 Media");
                ui.selectable_value(&mut s.search_filter.content_type, SearchContentType::LinksOnly, "🔗 Links");

                ui.add_space(16.0);

                ui.label(RichText::new("Date:").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                ui.selectable_value(&mut s.search_filter.date_range, SearchDateRange::AllTime, "All time");
                ui.selectable_value(&mut s.search_filter.date_range, SearchDateRange::Today, "Today");
                ui.selectable_value(&mut s.search_filter.date_range, SearchDateRange::Last7Days, "7 days");
                ui.selectable_value(&mut s.search_filter.date_range, SearchDateRange::Last30Days, "30 days");
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            // Results count banner
            ui.horizontal(|ui| {
                let count = results.len();
                let label = if s.search_filter.is_empty() {
                    "Type a query or select a filter above to search".to_string()
                } else if count == 0 {
                    "No messages matching criteria".to_string()
                } else {
                    format!("Found {} matching {}", count, if count == 1 { "message" } else { "messages" })
                };
                ui.label(RichText::new(label).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Reset filters").clicked() {
                        s.search_filter.reset_filters();
                    }
                });
            });

            ui.add_space(8.0);

            // Results list scroll area
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(340.0)
                .show(ui, |ui| {
                    if results.is_empty() {
                        ui.add_space(30.0);
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("💬").font(FontId::proportional(32.0)));
                        });
                    } else {
                        for item in &results {
                            let chat_jid = item.message.chat_jid.clone();
                            Frame::new()
                                .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
                                .inner_margin(Margin::same(10))
                                .corner_radius(egui::CornerRadius::same(6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&item.chat_name).font(FontId::proportional(13.0)).strong().color(Color32::WHITE));
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(format!("• {}", item.sender_name)).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x53, 0xBD, 0xEB)));

                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(RichText::new(item.message.timestamp.format("%Y-%m-%d %H:%M").to_string())
                                                .font(FontId::proportional(11.0))
                                                .color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                                        });
                                    });

                                    ui.add_space(4.0);

                                    // Snippet
                                    let preview = if let Some(media) = item.message.media_type {
                                        let icon = match media {
                                            crate::state::MediaType::Image => "📷 Photo",
                                            crate::state::MediaType::Video => "📹 Video",
                                            crate::state::MediaType::Audio => "🎵 Audio",
                                            crate::state::MediaType::Voice => "🎤 Voice",
                                            crate::state::MediaType::Document => "📄 Document",
                                            crate::state::MediaType::Sticker => "🎨 Sticker",
                                        };
                                        if item.message.text.is_empty() {
                                            icon.to_string()
                                        } else {
                                            format!("{} {}", icon, item.message.text)
                                        }
                                    } else {
                                        item.message.text.clone()
                                    };

                                    let text_resp = ui.selectable_label(false, preview);
                                    if text_resp.clicked() {
                                        selected_chat_to_open =
                                            Some((chat_jid, item.message.id.clone()));
                                    }
                                });
                            ui.add_space(4.0);
                        }
                    }
                });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        });

    if let Some((jid, message_id)) = selected_chat_to_open {
        s.selected_chat = Some(jid.clone());
        s.jump_target = Some(crate::state::JumpTarget::new(message_id));
        s.show_search_dialog = false;
        let _ = backend.send_command(crate::commands::Command::SelectChat { chat_jid: Some(jid) });
    }

    if close {
        s.show_search_dialog = false;
    }
}
