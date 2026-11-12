//! Call overlay and dialogs (Incoming call, Outgoing ringing, Active call)

use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Widget, vec2};
use crate::commands::Command;
use crate::state::{ActiveCall, CallStatus, SharedState};
use crate::ui::components::Avatar;

pub fn show_call_overlay(ctx: &egui::Context, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let active_call_opt = state.read().active_call.clone();
    let Some(call) = active_call_opt else {
        return;
    };

    // Auto-dismiss call if it has ended for more than 3 seconds
    if let Some(ended_at) = call.ended_at {
        let elapsed = (chrono::Utc::now() - ended_at).num_seconds();
        if elapsed >= 3 {
            state.write().active_call = None;
            return;
        }
    }

    match &call.status {
        CallStatus::Incoming => {
            show_incoming_call_dialog(ctx, &call, backend);
        }
        CallStatus::OutgoingRinging | CallStatus::Connected | CallStatus::Ended(_) => {
            show_active_call_window(ctx, &call, backend);
        }
    }
}

fn show_incoming_call_dialog(ctx: &egui::Context, call: &ActiveCall, backend: &crate::backend::BackendHandle) {
    let time = ctx.input(|i| i.time);
    let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32; // 0.0 to 1.0

    egui::Window::new("incoming_call_overlay")
        .anchor(egui::Align2::CENTER_TOP, vec2(0.0, 50.0))
        .fixed_size(vec2(360.0, 240.0))
        .resizable(false)
        .collapsible(false)
        .title_bar(false)
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
                .stroke(egui::Stroke::new(2.0, Color32::from_rgb(
                    (37.0 + pulse * 40.0) as u8,
                    (211.0 - pulse * 30.0) as u8,
                    102,
                )))
                .corner_radius(CornerRadius::same(16))
                .inner_margin(egui::Margin::symmetric(20, 16)),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Call type badge
                let call_type_text = if call.is_video {
                    "📹  Příchozí videohovor WhatsApp"
                } else {
                    "📞  Příchozí hlasový hovor WhatsApp"
                };
                ui.label(
                    RichText::new(call_type_text)
                        .font(FontId::proportional(13.0))
                        .color(Color32::from_rgb(0x25, 0xD3, 0x66))
                        .strong(),
                );

                ui.add_space(10.0);

                // Caller Avatar with pulsing outline
                let avatar_size = 64.0;
                Avatar::new(&call.peer_name)
                    .jid(call.peer_jid.clone())
                    .size(avatar_size)
                    .ui(ui);

                ui.add_space(8.0);

                // Caller name & number
                ui.label(
                    RichText::new(&call.peer_name)
                        .font(FontId::proportional(18.0))
                        .strong()
                        .color(Color32::WHITE),
                );
                
                let phone_str = crate::util::format_jid(&call.peer_jid);
                if phone_str != call.peer_name {
                    ui.label(
                        RichText::new(phone_str)
                            .font(FontId::proportional(13.0))
                            .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                    );
                }

                ui.add_space(16.0);

                // Action buttons: Accept (Green) & Decline (Red)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 24.0;
                    let btn_width = 130.0;
                    let btn_height = 38.0;

                    // Reject button
                    let reject_btn = egui::Button::new(
                        RichText::new("❌  Odmítnout")
                            .font(FontId::proportional(14.0))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0xEA, 0x00, 0x38))
                    .min_size(vec2(btn_width, btn_height))
                    .corner_radius(CornerRadius::same(19));

                    if ui.add(reject_btn).clicked() {
                        let _ = backend.send_command(Command::RejectCall {
                            call_id: call.call_id.clone(),
                        });
                    }

                    // Accept button
                    let accept_btn = egui::Button::new(
                        RichText::new("📞  Přijmout")
                            .font(FontId::proportional(14.0))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0x25, 0xD3, 0x66))
                    .min_size(vec2(btn_width, btn_height))
                    .corner_radius(CornerRadius::same(19));

                    if ui.add(accept_btn).clicked() {
                        let _ = backend.send_command(Command::AcceptCall {
                            call_id: call.call_id.clone(),
                        });
                    }
                });
            });
        });
}

fn show_active_call_window(ctx: &egui::Context, call: &ActiveCall, backend: &crate::backend::BackendHandle) {
    let window_title = match &call.status {
        CallStatus::OutgoingRinging => format!("📞 Vytáčení • {}", call.peer_name),
        CallStatus::Connected => if call.is_video {
            format!("📹 Videohovor • {}", call.peer_name)
        } else {
            format!("📞 Hovor • {}", call.peer_name)
        },
        CallStatus::Ended(_) => format!("📞 Konec hovoru • {}", call.peer_name),
        _ => "Hovor".to_string(),
    };

    let win_size = if call.is_video {
        vec2(440.0, 380.0)
    } else {
        vec2(320.0, 310.0)
    };

    egui::Window::new(window_title)
        .anchor(egui::Align2::RIGHT_TOP, vec2(-24.0, 56.0))
        .fixed_size(win_size)
        .resizable(false)
        .collapsible(true)
        .title_bar(true)
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(0x11, 0x1B, 0x21))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x2A, 0x39, 0x42)))
                .corner_radius(CornerRadius::same(16))
                .inner_margin(egui::Margin::symmetric(16, 16)),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Status line / duration
                match &call.status {
                    CallStatus::OutgoingRinging => {
                        let time = ctx.input(|i| i.time);
                        let dots = match ((time * 2.0) as u32) % 4 {
                            1 => ".",
                            2 => "..",
                            3 => "...",
                            _ => "",
                        };
                        ui.label(
                            RichText::new(format!("Vyzvánění{}", dots))
                                .font(FontId::proportional(14.0))
                                .color(Color32::from_rgb(0x00, 0xA8, 0x84)),
                        );
                    }
                    CallStatus::Connected => {
                        let duration_secs = call
                            .connected_at
                            .map(|start| (chrono::Utc::now() - start).num_seconds().max(0))
                            .unwrap_or(0);
                        let mins = duration_secs / 60;
                        let secs = duration_secs % 60;
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("●")
                                    .font(FontId::proportional(12.0))
                                    .color(Color32::from_rgb(0x25, 0xD3, 0x66)),
                            );
                            ui.label(
                                RichText::new(format!("{:02}:{:02}", mins, secs))
                                    .font(FontId::proportional(15.0))
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            if call.is_video {
                                ui.label(
                                    RichText::new("• 720p HD")
                                        .font(FontId::proportional(12.0))
                                        .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                                );
                            }
                        });
                    }
                    CallStatus::Ended(ref reason) => {
                        ui.label(
                            RichText::new(reason)
                                .font(FontId::proportional(14.0))
                                .color(Color32::from_rgb(0xEA, 0x00, 0x38)),
                        );
                    }
                    _ => {}
                }

                ui.add_space(8.0);

                // Avatar or Video display area
                if call.is_video && call.is_video_enabled {
                    // Video stage with remote view and PIP local preview
                    egui::Frame::new()
                        .fill(Color32::from_rgb(0x18, 0x22, 0x29))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x2A, 0x39, 0x42)))
                        .corner_radius(CornerRadius::same(12))
                        .show(ui, |ui| {
                            ui.set_min_size(vec2(400.0, 200.0));
                            ui.vertical_centered(|ui| {
                                ui.add_space(20.0);
                                Avatar::new(&call.peer_name)
                                    .jid(call.peer_jid.clone())
                                    .size(60.0)
                                    .ui(ui);
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(format!("📹  Videopřenos: {}", call.peer_name))
                                        .font(FontId::proportional(15.0))
                                        .strong()
                                        .color(Color32::WHITE),
                                );
                                ui.label(
                                    RichText::new("H.264 šifrovaný videostream WhatsApp")
                                        .font(FontId::proportional(12.0))
                                        .color(Color32::from_rgb(0x25, 0xD3, 0x66)),
                                );
                                ui.add_space(10.0);
                                // PIP indicator
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(12.0);
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(0x0C, 0x13, 0x17))
                                        .corner_radius(CornerRadius::same(6))
                                        .inner_margin(egui::Margin::symmetric(8, 4))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("Kamera: Aktivní")
                                                    .font(FontId::proportional(10.5))
                                                    .color(Color32::from_rgb(0x25, 0xD3, 0x66)),
                                            );
                                        });
                                });
                            });
                        });
                } else if call.is_video && !call.is_video_enabled {
                    // Video call with camera paused
                    egui::Frame::new()
                        .fill(Color32::from_rgb(0x18, 0x22, 0x29))
                        .corner_radius(CornerRadius::same(12))
                        .show(ui, |ui| {
                            ui.set_min_size(vec2(400.0, 200.0));
                            ui.vertical_centered(|ui| {
                                ui.add_space(45.0);
                                Avatar::new(&call.peer_name)
                                    .jid(call.peer_jid.clone())
                                    .size(56.0)
                                    .ui(ui);
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Kamera je vypnuta")
                                        .font(FontId::proportional(14.0))
                                        .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                                );
                            });
                        });
                } else {
                    // Voice call: Large Avatar
                    Avatar::new(&call.peer_name)
                        .jid(call.peer_jid.clone())
                        .size(72.0)
                        .ui(ui);

                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&call.peer_name)
                            .font(FontId::proportional(17.0))
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        RichText::new(crate::util::format_jid(&call.peer_jid))
                            .font(FontId::proportional(13.0))
                            .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                    );
                }

                ui.add_space(16.0);

                // Controls row: Mute Mic, Camera toggle, End Call
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;

                    // Mute / Unmute
                    let (mic_icon, mic_bg, mic_hover) = if call.is_muted {
                        ("🔇", Color32::from_rgb(0xEA, 0x00, 0x38), "Zapnout mikrofon")
                    } else {
                        ("🎙️", Color32::from_rgb(0x2A, 0x39, 0x42), "Ztlumit mikrofon")
                    };
                    let mic_btn = egui::Button::new(RichText::new(mic_icon).font(FontId::proportional(18.0)))
                        .fill(mic_bg)
                        .min_size(vec2(44.0, 44.0))
                        .corner_radius(CornerRadius::same(22));
                    if ui.add(mic_btn).on_hover_text(mic_hover).clicked() {
                        let _ = backend.send_command(Command::ToggleCallMute);
                    }

                    // Video toggle
                    let (cam_icon, cam_bg, cam_hover) = if call.is_video_enabled {
                        ("📹", Color32::from_rgb(0x00, 0xA8, 0x84), "Vypnout kameru")
                    } else {
                        ("🚫", Color32::from_rgb(0x2A, 0x39, 0x42), "Zapnout kameru")
                    };
                    let cam_btn = egui::Button::new(RichText::new(cam_icon).font(FontId::proportional(18.0)))
                        .fill(cam_bg)
                        .min_size(vec2(44.0, 44.0))
                        .corner_radius(CornerRadius::same(22));
                    if ui.add(cam_btn).on_hover_text(cam_hover).clicked() {
                        let _ = backend.send_command(Command::ToggleCallVideo);
                    }

                    // End Call button (Hang up)
                    let end_btn = egui::Button::new(
                        RichText::new("🔴 Zavěsit")
                            .font(FontId::proportional(14.0))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0xEA, 0x00, 0x38))
                    .min_size(vec2(110.0, 44.0))
                    .corner_radius(CornerRadius::same(22));

                    if ui.add(end_btn).clicked() {
                        let _ = backend.send_command(Command::EndCall {
                            call_id: call.call_id.clone(),
                        });
                    }
                });
            });
        });
}
