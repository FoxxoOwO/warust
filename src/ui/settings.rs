//! Settings Dialog - WhatsApp Web/Desktop style modern settings
use egui::{Align, Color32, Context, CornerRadius, FontId, Frame, Layout, Margin, RichText, ScrollArea, Stroke, TextEdit, Ui, Vec2, Window};

use crate::state::{ConnectionStatus, SettingsTab, SharedState};
use crate::backend::BackendHandle;

/// Show settings window using a context (for Window API)
pub fn show_settings_with_ctx(ctx: &Context, state: &SharedState, backend: &BackendHandle, open: &mut bool) {
    let mut should_close = false;
    
    Window::new("⚙️  Nastavení")
        .open(open)
        .resizable(true)
        .default_width(760.0)
        .default_height(680.0)
        .min_width(640.0)
        .min_height(560.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            show_settings_content(ui, state, backend, &mut should_close);
        });

    // Settings are edited in place; flag them so they get flushed to disk.
    state.write().dirty = true;
        
    if should_close {
        *open = false;
    }
}

/// Show settings content with a left navigation sidebar and right detail panel
fn show_settings_content(ui: &mut Ui, state: &SharedState, backend: &BackendHandle, should_close: &mut bool) {
    let mut s = state.write();
    let current_tab = s.settings_tab;

    ui.horizontal(|ui| {
        // --- Left Sidebar: Category Tabs ---
        ui.vertical(|ui| {
            ui.set_width(210.0);
            ui.add_space(8.0);

            let tabs = [
                (SettingsTab::Profile, "👤  Účet a profil"),
                (SettingsTab::Chats, "💬  Konverzace a média"),
                (SettingsTab::Notifications, "🔔  Oznámení"),
                (SettingsTab::Privacy, "🔒  Soukromí"),
                (SettingsTab::Appearance, "🎨  Vzhled"),
                (SettingsTab::Storage, "ℹ️  O aplikaci a data"),
                (SettingsTab::System, "🖥️  Systém a spuštění"),
            ];

            for (tab, label) in tabs {
                let is_active = current_tab == tab;
                let bg_color = if is_active {
                    Color32::from_rgb(0x00, 0x5C, 0x4B) // WhatsApp teal selection
                } else {
                    Color32::TRANSPARENT
                };
                let text_color = if is_active {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(0xD1, 0xD7, 0xDB)
                };

                let btn = egui::Button::new(
                    RichText::new(label)
                        .font(FontId::proportional(13.5))
                        .color(text_color)
                )
                .fill(bg_color)
                .corner_radius(CornerRadius::same(8))
                .min_size(Vec2::new(198.0, 36.0));

                if ui.add(btn).clicked() {
                    s.settings_tab = tab;
                }
                ui.add_space(2.0);
            }

            ui.add_space(24.0);
            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                ui.add_space(8.0);
                if ui.button(RichText::new("Zavřít").font(FontId::proportional(13.0))).clicked() {
                    *should_close = true;
                }
            });
        });

        ui.separator();
        ui.add_space(8.0);

        // --- Right Content Area ---
        ui.vertical(|ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(6.0);
                    match current_tab {
                        SettingsTab::Profile => show_profile_settings(ui, &mut s, backend),
                        SettingsTab::Chats => show_chats_settings(ui, &mut s),
                        SettingsTab::Notifications => show_notifications_settings(ui, &mut s),
                        SettingsTab::Privacy => show_privacy_settings(ui, &mut s),
                        SettingsTab::Appearance => show_appearance_settings(ui, &mut s),
                        SettingsTab::Storage => show_storage_settings(ui, &mut s, backend, should_close),
                        SettingsTab::System => show_system_settings(ui, &mut s),
                    }
                    ui.add_space(16.0);
                });
        });
    });
}

fn setting_card<R>(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.label(RichText::new(title).font(FontId::proportional(14.5)).strong().color(Color32::from_rgb(0x00, 0xA8, 0x84)));
    ui.add_space(4.0);
    let resp = Frame::new()
        .fill(Color32::from_rgb(0x18, 0x22, 0x29))
        .corner_radius(CornerRadius::same(8))
        .stroke(Stroke::new(1.0, Color32::from_rgb(0x22, 0x2D, 0x34)))
        .inner_margin(Margin::same(12))
        .show(ui, add_contents);
    ui.add_space(12.0);
    resp.inner
}

fn show_profile_settings(ui: &mut Ui, s: &mut crate::state::AppState, _backend: &BackendHandle) {
    ui.label(RichText::new("Účet a profil").font(FontId::proportional(18.0)).strong());
    ui.label(RichText::new("Informace o vašem WhatsApp účtu a profilu").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
    ui.add_space(12.0);

    setting_card(ui, "Profilové údaje", |ui| {
        ui.horizontal(|ui| {
            // Big avatar circle
            let avatar_size = 54.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(avatar_size), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), avatar_size / 2.0, Color32::from_rgb(0x00, 0x80, 0x69));
            
            let initial = s.self_name.as_deref().and_then(|n| n.chars().next()).unwrap_or('U').to_string();
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, initial, FontId::proportional(22.0), Color32::WHITE);

            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Jméno (Push Name):").strong());
                    let mut name = s.self_name.clone().unwrap_or_default();
                    if ui.add(TextEdit::singleline(&mut name).desired_width(180.0)).changed() {
                        s.self_name = if name.is_empty() { None } else { Some(name) };
                    }
                });
                ui.add_space(4.0);
                if let Some(ref jid) = s.self_jid {
                    ui.label(RichText::new(format!("ID účtu: {}", jid)).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    let formatted = crate::util::format_jid(jid);
                    ui.label(RichText::new(format!("Telefonní číslo: {}", formatted)).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                } else {
                    ui.label(RichText::new("Stav: Nepřipojeno").color(Color32::from_rgb(0xFF, 0x6B, 0x6B)));
                }
            });
        });
    });

    setting_card(ui, "Stav připojení", |ui| {
        ui.horizontal(|ui| {
            let (status_text, color) = match s.connection {
                ConnectionStatus::Connected => ("Online - Připojeno k WhatsApp", Color32::from_rgb(0x25, 0xD3, 0x66)),
                ConnectionStatus::Connecting => ("Připojování...", Color32::YELLOW),
                ConnectionStatus::Syncing(_p) => ("Probíhá synchronizace historie...", Color32::YELLOW),
                ConnectionStatus::Error(ref _e) => ("Chyba připojení", Color32::RED),
                _ => ("Odpojeno", Color32::GRAY),
            };
            ui.colored_label(color, "●");
            ui.label(RichText::new(status_text).font(FontId::proportional(13.0)));
        });
    });
}

fn show_chats_settings(ui: &mut Ui, s: &mut crate::state::AppState) {
    ui.label(RichText::new("Konverzace a média").font(FontId::proportional(18.0)).strong());
    ui.label(RichText::new("Nastavení chování odesílání zpráv a stahování příloh").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
    ui.add_space(12.0);

    setting_card(ui, "Odesílání zpráv", |ui| {
        ui.checkbox(&mut s.preferences.enter_is_send, "Klávesa Enter odešle zprávu");
        ui.label(RichText::new("Když je zapnuto, klávesa Enter zprávu odešle a Shift+Enter vloží nový řádek.").font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
        ui.add_space(6.0);
        ui.checkbox(&mut s.auto_scroll_to_bottom, "Automaticky posouvat konverzaci dolů při nových zprávách");
    });

    setting_card(ui, "Automatické stahování médií", |ui| {
        ui.checkbox(&mut s.preferences.auto_download_photos, "Automaticky stahovat fotografie a obrázky");
        ui.checkbox(&mut s.preferences.auto_download_videos, "Automaticky stahovat videa");
        ui.checkbox(&mut s.preferences.auto_download_docs, "Automaticky stahovat dokumenty a soubory");
        ui.add_space(4.0);
        ui.label(RichText::new("Videa a dokumenty lze vždy stáhnout ručně kliknutím na přílohu v bublině.").font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
    });

    setting_card(ui, "Velikost textu zpráv", |ui| {
        ui.horizontal(|ui| {
            ui.label("Velikost písma:");
            ui.add(egui::Slider::new(&mut s.preferences.font_size, 12.0..=18.0).suffix(" px"));
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Ukázka: Ahoj! Jak se dnes máš?").font(FontId::proportional(s.preferences.font_size)).color(Color32::from_rgb(0x25, 0xD3, 0x66)));
    });
}

fn show_notifications_settings(ui: &mut Ui, s: &mut crate::state::AppState) {
    ui.label(RichText::new("Oznámení").font(FontId::proportional(18.0)).strong());
    ui.label(RichText::new("Nastavte, jakým způsobem vás aplikace upozorní na nové zprávy").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
    ui.add_space(12.0);

    setting_card(ui, "Upozornění a zvuky", |ui| {
        ui.checkbox(&mut s.preferences.sound_notifications, "Přehrát zvuk při příchozí zprávě");
        ui.add_space(4.0);
        ui.checkbox(&mut s.preferences.show_previews, "Zobrazovat náhled textu v oznámení");
        ui.add_space(4.0);
        ui.checkbox(&mut s.preferences.desktop_notifications, "Povolit systémová oznámení na ploše Windows");
    });
}

fn show_privacy_settings(ui: &mut Ui, _s: &mut crate::state::AppState) {
    ui.label(RichText::new("Soukromí a zabezpečení").font(FontId::proportional(18.0)).strong());
    ui.label(RichText::new("Nastavení šifrování a potvrzení o přečtení").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
    ui.add_space(12.0);

    setting_card(ui, "Potvrzení o přečtení (Read Receipts)", |ui| {
        let mut read_receipts = true;
        ui.checkbox(&mut read_receipts, "Automaticky odesílat potvrzení o přečtení (modré fajfky ✓✓)");
        ui.label(RichText::new("Pokud je vypnuto, odesílatel neuvidí, kdy přesně jste zprávu otevřeli.").font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
    });

    setting_card(ui, "End-to-End Šifrování", |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔒").font(FontId::proportional(22.0)));
            ui.vertical(|ui| {
                ui.label(RichText::new("Koncové šifrování je aktivní").strong().color(Color32::from_rgb(0x25, 0xD3, 0x66)));
                ui.label(RichText::new("Všechny vaše zprávy, reakce a média jsou šifrovány pomocí protokolu Signal. Klíče jsou uloženy pouze v lokální bezpečné SQLite databázi ve vašem počítači.").font(FontId::proportional(11.5)).color(Color32::from_rgb(0xD1, 0xD7, 0xDB)));
            });
        });
    });
}

fn show_appearance_settings(ui: &mut Ui, s: &mut crate::state::AppState) {
    ui.label(RichText::new("Vzhled aplikace").font(FontId::proportional(18.0)).strong());
    ui.label(RichText::new("Přizpůsobte si barevné schéma a styl rozhraní").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
    ui.add_space(12.0);

    setting_card(ui, "Barevné schéma", |ui| {
        ui.radio_value(&mut s.preferences.dark_theme_preset, 0, "Výchozí WhatsApp Dark (#111B21)");
        ui.radio_value(&mut s.preferences.dark_theme_preset, 1, "OLED Černá (#000000)");
        ui.radio_value(&mut s.preferences.dark_theme_preset, 2, "Půlnoční modrá (#0A192F)");
    });
}

fn show_storage_settings(ui: &mut Ui, _s: &mut crate::state::AppState, backend: &BackendHandle, should_close: &mut bool) {
    ui.label(RichText::new("O aplikaci a úložiště").font(FontId::proportional(18.0)).strong());
    ui.label(RichText::new("Informace o verzích softwaru a správě mezipaměti").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
    ui.add_space(12.0);

    setting_card(ui, "Verze aplikace", |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Warust:").strong());
            ui.monospace(option_env!("CARGO_PKG_VERSION").unwrap_or("0.1.0"));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("whatsapp-rust:").strong());
            ui.monospace("0.7.0 (Rust native client)");
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("GUI toolkit:").strong());
            ui.monospace("egui 0.37 / wgpu");
        });
    });

    setting_card(ui, "Správa stažených médií", |ui| {
        let media_path = crate::media::media_dir();
        ui.label(RichText::new(format!("Složka médií: {}", media_path.display())).font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("📂  Otevřít složku médií").clicked() {
                let _ = std::process::Command::new("explorer").arg(&media_path).spawn();
            }
        });
    });

    setting_card(ui, "Účet a odhlášení", |ui| {
        ui.label(RichText::new("Pokud se odhlásíte, relace bude ukončena a pro další použití bude nutné znovu naskenovat QR kód.").font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
        ui.add_space(8.0);
        if ui.button(RichText::new("Odhlásit se z účtu WhatsApp").color(Color32::from_rgb(0xFF, 0x6B, 0x6B))).clicked() {
            let _ = backend.send_command(crate::commands::Command::Disconnect);
            *should_close = true;
        }
    });
}

fn show_system_settings(ui: &mut Ui, s: &mut crate::state::AppState) {
    ui.label(RichText::new("Systém a spuštění").font(FontId::proportional(18.0)).strong());
    ui.label(RichText::new("Nastavení chování aplikace v systému Windows a oznamovací oblasti (Tray)").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
    ui.add_space(12.0);

    setting_card(ui, "Spuštění se systémem Windows", |ui| {
        let mut autostart = s.preferences.start_with_windows;
        if ui.checkbox(&mut autostart, "Spouštět Warust při přihlášení do Windows").changed() {
            s.preferences.start_with_windows = autostart;
            let _ = crate::autostart::set_autostart_enabled(autostart);
        }
        ui.label(
            RichText::new("Aplikace se po přihlášení automaticky spustí na pozadí minimalizovaná do oznamovací oblasti.")
                .font(FontId::proportional(11.0))
                .color(Color32::from_rgb(0x86, 0x96, 0xA0))
        );
    });

    setting_card(ui, "Oznamovací oblast (Systémová lišta / Tray)", |ui| {
        ui.checkbox(&mut s.preferences.close_to_tray, "Zavřením okna (křížkem) minimalizovat do systémové lišty");
        ui.label(
            RichText::new("Při kliknutí na křížek se okno pouze skryje a Warust zůstane aktivní na pozadí pro příjem zpráv a hovorů.")
                .font(FontId::proportional(11.0))
                .color(Color32::from_rgb(0x86, 0x96, 0xA0))
        );
        ui.add_space(8.0);
        ui.checkbox(&mut s.preferences.run_in_tray, "Zobrazovat ikonu v oznamovací oblasti (Tray)");
        ui.label(
            RichText::new("Umožňuje snadné otevření nebo úplné ukončení aplikace kliknutím na ikonu vedle hodin.")
                .font(FontId::proportional(11.0))
                .color(Color32::from_rgb(0x86, 0x96, 0xA0))
        );
    });
}
