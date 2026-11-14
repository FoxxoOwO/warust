//! Okno s nabídkou nové verze.

use egui::{Color32, RichText};

use crate::state::SharedState;
use crate::theme::ACCENT;
use crate::updater::{self, CURRENT_VERSION};

/// Vykreslí okno aktualizace, pokud je otevřené.
pub fn show_update_dialog(ctx: &egui::Context, state: &SharedState) {
    let snapshot = {
        let s = state.read();
        if !s.show_update_dialog {
            return;
        }
        (
            s.update_available.clone(),
            s.update_status.clone(),
            s.preferences.auto_install_updates,
        )
    };
    let (available, status, auto_install) = snapshot;

    let mut close = false;
    let mut install = false;
    let mut open_page = None;

    egui::Window::new("Aktualizace")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(true)
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("Nainstalováno: v{CURRENT_VERSION}"))
                    .color(Color32::from_gray(170)),
            );

            match &available {
                Some(info) => {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("K dispozici je v{}", info.version))
                            .strong()
                            .size(16.0)
                            .color(ACCENT),
                    );
                    if !info.notes.trim().is_empty() {
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .max_height(240.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(info.notes.trim())
                                        .monospace()
                                        .size(12.0)
                                        .color(Color32::from_gray(200)),
                                );
                            });
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if info.has_asset() {
                            if ui
                                .add_enabled(
                                    !status.is_busy(),
                                    egui::Button::new(RichText::new("⬇ Stáhnout a nainstalovat").strong()),
                                )
                                .clicked()
                            {
                                install = true;
                                close = true;
                            }
                        }
                        if ui.button("Stránka vydání").clicked() {
                            open_page = Some(info.page_url.clone());
                        }
                        if ui.button("Později").clicked() {
                            close = true;
                        }
                    });
                    if info.has_asset() {
                        ui.label(
                            RichText::new(
                                "Aplikace se po stažení zavře, vymění se spustitelný soubor a znovu se otevře.",
                            )
                            .size(11.0)
                            .color(Color32::from_gray(150)),
                        );
                    } else {
                        ui.label(
                            RichText::new("Pro tuto platformu není v tomto vydání balíček ke stažení.")
                                .size(11.0)
                                .color(Color32::from_rgb(0xE0, 0xA0, 0x40)),
                        );
                    }
                    if auto_install {
                        ui.label(
                            RichText::new("Automatická instalace je zapnutá — aktualizace proběhne sama.")
                                .size(11.0)
                                .color(Color32::from_gray(150)),
                        );
                    }
                }
                None => {
                    ui.add_space(4.0);
                    ui.label("Není k dispozici žádná novější verze.");
                    if !status.label().is_empty() {
                        ui.label(
                            RichText::new(status.label())
                                .size(11.0)
                                .color(Color32::from_gray(150)),
                        );
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Zkontrolovat znovu").clicked() {
                            updater::spawn_check(state.clone());
                        }
                        if ui.button("Zavřít").clicked() {
                            close = true;
                        }
                    });
                }
            }
        });

    if let Some(url) = open_page {
        ctx.open_url(egui::OpenUrl::new_tab(url));
    }
    if install {
        if let Some(info) = available.clone() {
            updater::spawn_install(state.clone(), info);
        }
    }
    if close {
        state.write().show_update_dialog = false;
    }
}

/// Malé tlačítko do horní lišty — vrací `true`, když se má otevřít okno.
pub fn update_badge(ui: &mut egui::Ui, state: &SharedState) -> bool {
    let available = state.read().update_available.clone();
    let Some(info) = available else {
        return false;
    };
    let label = format!("⬆ v{}", info.version);
    ui.button(label)
        .on_hover_text("Je dostupná nová verze — klikni pro podrobnosti.")
        .clicked()
}
