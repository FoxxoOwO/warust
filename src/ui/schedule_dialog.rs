//! Scheduled Messages Dialog and Management UI

use chrono::{Duration, Utc};
use egui::{Align, Color32, FontId, Frame, Layout, Margin, RichText, ScrollArea, TextEdit};
use crate::state::{ScheduledStatus, SharedState};

/// Dialog to schedule a new message
pub fn show_schedule_dialog(ctx: &egui::Context, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let mut s = state.write();
    if !s.show_schedule_dialog {
        return;
    }

    let target_jid = s.schedule_target_chat.clone().or_else(|| s.selected_chat.clone());
    let target_name = target_jid.as_ref().map(|j| {
        s.chats.get(j).map(|c| c.name.clone()).unwrap_or_else(|| crate::util::format_jid(j))
    }).unwrap_or_else(|| "No chat selected".to_string());

    let mut close = false;
    let mut submit = false;

    let now = Utc::now();
    let scheduled_time = now + Duration::minutes(s.schedule_delay_minutes as i64);

    egui::Window::new("🕒  Schedule Message")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.add_space(6.0);

            // Recipient info
            Frame::new()
                .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
                .inner_margin(Margin::same(10))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.label(RichText::new("Recipient:").font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    ui.label(RichText::new(&target_name).font(FontId::proportional(14.0)).strong().color(Color32::WHITE));
                });

            ui.add_space(10.0);

            // Message content
            ui.label(RichText::new("Message text:").font(FontId::proportional(13.0)).strong());
            ui.add_space(4.0);
            ui.add(
                TextEdit::multiline(&mut s.schedule_text)
                    .hint_text("Type the message to send later...")
                    .desired_width(ui.available_width())
                    .desired_rows(3)
                    .font(FontId::proportional(13.0))
            );

            ui.add_space(12.0);

            // Delay presets
            ui.label(RichText::new("When to send:").font(FontId::proportional(13.0)).strong());
            ui.add_space(6.0);

            ui.horizontal_wrapped(|ui| {
                if ui.button("+5 min").clicked() { s.schedule_delay_minutes = 5; }
                if ui.button("+15 min").clicked() { s.schedule_delay_minutes = 15; }
                if ui.button("+30 min").clicked() { s.schedule_delay_minutes = 30; }
                if ui.button("+1 hour").clicked() { s.schedule_delay_minutes = 60; }
                if ui.button("+3 hours").clicked() { s.schedule_delay_minutes = 180; }
                if ui.button("+1 day").clicked() { s.schedule_delay_minutes = 1440; }
            });

            ui.add_space(8.0);

            // Delay slider
            ui.horizontal(|ui| {
                ui.label("Delay (minutes):");
                ui.add(egui::Slider::new(&mut s.schedule_delay_minutes, 1..=1440).text("min"));
            });

            ui.add_space(10.0);

            // Scheduled dispatch preview
            Frame::new()
                .fill(Color32::from_rgba_premultiplied(0, 168, 132, 25))
                .inner_margin(Margin::same(10))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🕒 Scheduled dispatch:").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x25, 0xD3, 0x66)));
                        ui.label(RichText::new(scheduled_time.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                            .font(FontId::proportional(12.0)).strong().color(Color32::WHITE));
                        ui.label(RichText::new(format!("(in {} min)", s.schedule_delay_minutes))
                            .font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    });
                });

            ui.add_space(14.0);

            // Action buttons
            ui.horizontal(|ui| {
                let can_schedule = target_jid.is_some() && !s.schedule_text.trim().is_empty();
                if ui.add_enabled(can_schedule, egui::Button::new(RichText::new("🕒  Schedule Message").strong())).clicked() {
                    submit = true;
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(4.0);
        });

    if submit {
        if let Some(to) = target_jid {
            let text = s.schedule_text.trim().to_string();
            if !text.is_empty() {
                let _ = backend.send_command(crate::commands::Command::ScheduleMessage {
                    chat_jid: to,
                    text,
                    scheduled_for: scheduled_time,
                });
                s.schedule_text.clear();
                s.show_schedule_dialog = false;
            }
        }
    }

    if close {
        s.show_schedule_dialog = false;
    }
}

/// Modal / Window to view and manage pending scheduled messages
pub fn show_scheduled_manager(ctx: &egui::Context, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let mut s = state.write();
    if !s.show_scheduled_manager {
        return;
    }

    let mut close = false;
    let mut send_now_id = None;
    let mut cancel_id = None;
    let mut delete_id = None;

    let now = Utc::now();

    egui::Window::new("🕒  Scheduled Messages Manager")
        .collapsible(false)
        .resizable(true)
        .default_width(540.0)
        .default_height(400.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.add_space(4.0);

            let pending_count = s.scheduled_messages.iter().filter(|m| m.status == ScheduledStatus::Pending).count();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} pending scheduled {}", pending_count, if pending_count == 1 { "message" } else { "messages" }))
                    .font(FontId::proportional(13.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("➕ Schedule new").clicked() {
                        s.show_schedule_dialog = true;
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(320.0)
                .show(ui, |ui| {
                    if s.scheduled_messages.is_empty() {
                        ui.add_space(30.0);
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("No scheduled messages").font(FontId::proportional(14.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                        });
                    } else {
                        for item in &s.scheduled_messages {
                            let chat_name = s.chats.get(&item.chat_jid)
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| crate::util::format_jid(&item.chat_jid));

                            Frame::new()
                                .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
                                .inner_margin(Margin::same(10))
                                .corner_radius(egui::CornerRadius::same(6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&chat_name).font(FontId::proportional(14.0)).strong().color(Color32::WHITE));

                                        // Status badge
                                        let (status_text, status_color) = match &item.status {
                                            ScheduledStatus::Pending => {
                                                let diff = (item.scheduled_for - now).num_minutes();
                                                let time_label = if diff > 0 {
                                                    format!("in {} min", diff)
                                                } else {
                                                    "due now".to_string()
                                                };
                                                (format!("⏳ Pending ({})", time_label), Color32::from_rgb(0x25, 0xD3, 0x66))
                                            }
                                            ScheduledStatus::Sent => ("✓ Sent".to_string(), Color32::from_rgb(0x53, 0xBD, 0xEB)),
                                            ScheduledStatus::Cancelled => ("✕ Cancelled".to_string(), Color32::from_rgb(0x86, 0x96, 0xA0)),
                                            ScheduledStatus::Failed(err) => (format!("⚠ Failed: {}", err), Color32::from_rgb(0xF1, 0x5C, 0x5C)),
                                        };
                                        ui.colored_label(status_color, status_text);

                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(RichText::new(item.scheduled_for.format("%Y-%m-%d %H:%M").to_string())
                                                .font(FontId::proportional(11.0))
                                                .color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                                        });
                                    });

                                    ui.add_space(4.0);
                                    ui.label(RichText::new(&item.text).font(FontId::proportional(13.0)).color(Color32::from_rgb(0xE9, 0xED, 0xEF)));
                                    ui.add_space(6.0);

                                    // Actions for this message
                                    ui.horizontal(|ui| {
                                        if item.status == ScheduledStatus::Pending {
                                            if ui.button("🚀 Send now").clicked() {
                                                send_now_id = Some(item.id.clone());
                                            }
                                            if ui.button("❌ Cancel").clicked() {
                                                cancel_id = Some(item.id.clone());
                                            }
                                        } else {
                                            if ui.button("🗑 Delete").clicked() {
                                                delete_id = Some(item.id.clone());
                                            }
                                        }
                                    });
                                });
                            ui.add_space(6.0);
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

    if let Some(id) = send_now_id {
        let _ = backend.send_command(crate::commands::Command::SendScheduledMessageNow { id });
    }
    if let Some(id) = cancel_id {
        let _ = backend.send_command(crate::commands::Command::CancelScheduledMessage { id });
    }
    if let Some(id) = delete_id {
        s.scheduled_messages.retain(|m| m.id != id);
    }

    if close {
        s.show_scheduled_manager = false;
    }
}
