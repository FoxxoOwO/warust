//! QR Pairing Screen

use egui::{Color32, FontId, ProgressBar, RichText, Ui, Vec2, TextEdit};
use qrcode::QrCode;
use image::Luma;

use crate::state::{ConnectionStatus, SharedState};
use crate::backend::BackendHandle;
use crate::theme;

/// QR Pairing full-screen view
pub fn show_qr_pairing(ui: &mut Ui, state: &SharedState, backend: &BackendHandle) {
    // Snapshot what we need and release the lock before building the UI:
    // nested `state.write()` below would otherwise deadlock on the held guard.
    let (connection, pair_phone_number) = {
        let s = state.read();
        (s.connection.clone(), s.pair_phone_number.clone())
    };

    match &connection {
        ConnectionStatus::QrCode(code) => {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                
                ui.label(RichText::new("Link your WhatsApp account").font(FontId::proportional(24.0)).strong());
                ui.add_space(8.0);
                ui.label(RichText::new("Scan this QR code with WhatsApp on your phone").font(FontId::proportional(14.0)).color(Color32::GRAY));
                ui.add_space(32.0);
                
                // QR Code
                if let Ok(qr) = QrCode::new(code.as_bytes()) {
                    let image = qr.render::<Luma<u8>>()
                        .min_dimensions(256, 256)
                        .build();
                    
                    let pixels: Vec<Color32> = image.pixels()
                        .map(|p| Color32::from_gray(p.0[0]))
                        .collect();
                    
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        &pixels.iter().flat_map(|c| c.to_array()).collect::<Vec<_>>(),
                    );
                    
                    let texture = ui.ctx().load_texture("qr_code", color_image, Default::default());
                    
                    let card_size = Vec2::splat(280.0);
                    let (rect, _resp) = ui.allocate_exact_size(card_size, egui::Sense::hover());
                    ui.painter().rect_filled(rect, egui::CornerRadius::same(16), Color32::WHITE);
                    let img_rect = egui::Rect::from_center_size(rect.center(), Vec2::splat(256.0));
                    ui.put(img_rect, egui::Image::new(&texture).fit_to_exact_size(Vec2::splat(256.0)));
                }
                
                ui.add_space(24.0);
                
                ui.label(RichText::new("1. Open WhatsApp on your phone").font(FontId::proportional(14.0)));
                ui.label(RichText::new("2. Go to Settings → Linked Devices").font(FontId::proportional(14.0)));
                ui.label(RichText::new("3. Tap 'Link a Device' and scan the code").font(FontId::proportional(14.0)));
                
                ui.add_space(32.0);
                
                ui.separator();
                ui.add_space(16.0);
                
                ui.label(RichText::new("Or link with phone number").font(FontId::proportional(16.0)).strong());
                ui.add_space(10.0);
                
                ui.horizontal(|ui| {
                    let total_width = 380.0;
                    let left_pad = (ui.available_width() - total_width).max(0.0) * 0.5;
                    ui.add_space(left_pad);

                    let mut phone = pair_phone_number.clone();
                    let response = ui.add(
                        TextEdit::singleline(&mut phone)
                            .hint_text("Phone number (e.g. 15551234567)")
                            .desired_width(260.0)
                            .font(FontId::proportional(14.0))
                    );
                    if response.changed() {
                        state.write().pair_phone_number = phone.clone();
                    }
                    
                    let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let button_clicked = ui.button("Get Pair Code").clicked();
                    
                    if (button_clicked || enter_pressed) && !phone.is_empty() {
                        let _ = backend.send_command(crate::commands::Command::RequestPairCode { phone_number: phone.clone() });
                    }
                });
                
                ui.add_space(16.0);
                
                if ui.button("Cancel").clicked() {
                    let _ = backend.send_command(crate::commands::Command::CancelPairCode);
                }
            });
        }
        
        ConnectionStatus::PairCode(code) => {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                
                ui.label(RichText::new("Enter this code on your phone").font(FontId::proportional(24.0)).strong());
                ui.add_space(16.0);
                
                ui.label(RichText::new(code).font(FontId::monospace(48.0)).color(theme::status::READ));
                
                ui.add_space(24.0);
                
                ui.label(RichText::new("1. Open WhatsApp on your phone").font(FontId::proportional(14.0)));
                ui.label(RichText::new("2. Go to Settings → Linked Devices").font(FontId::proportional(14.0)));
                ui.label(RichText::new("3. Tap 'Link a Device' → 'Link with phone number instead'").font(FontId::proportional(14.0)));
                ui.label(RichText::new("4. Enter the 8-character code above").font(FontId::proportional(14.0)));
                
                ui.add_space(32.0);
                
                if ui.button("Cancel").clicked() {
                    let _ = backend.send_command(crate::commands::Command::CancelPairCode);
                }
            });
        }
        
        ConnectionStatus::Connecting | ConnectionStatus::Disconnected => {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.spinner();
                ui.add_space(16.0);
                ui.label(RichText::new("Connecting to WhatsApp...").font(FontId::proportional(20.0)).strong());
                ui.add_space(8.0);
                ui.label(RichText::new("Generating QR code, please wait a moment...").font(FontId::proportional(14.0)).color(Color32::GRAY));
            });
        }
        
        ConnectionStatus::Syncing(progress) => {
            let progress_f = (*progress as f32 / 100.0).clamp(0.08, 1.0);
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.label("Syncing chat history...");
                ui.add_space(16.0);
                ui.add(ProgressBar::new(progress_f).show_percentage().desired_width(300.0));
                ui.add_space(16.0);
                if ui.button("Continue to chats").clicked() {
                    let _ = backend.send_command(crate::commands::Command::DismissSyncing);
                }
            });
        }
        
        ConnectionStatus::Error(err) => {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.colored_label(Color32::RED, format!("Connection error: {}", err));
                ui.add_space(16.0);
                if ui.button("Retry").clicked() {
                    let _ = backend.send_command(crate::commands::Command::Connect);
                }
            });
        }
        
        _ => {}
    }
}