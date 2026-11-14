//! Poll composer.
//!
//! WhatsApp caps a poll at twelve options and needs a question plus at least
//! two distinct options, so the dialog validates before it queues
//! `Command::CreatePoll` and lets the backend deal with encryption.

use egui::{Color32, FontId, RichText};

use crate::backend::BackendHandle;
use crate::commands::Command;
use crate::state::{AppState, SharedState};

const MAX_OPTIONS: usize = 12;

pub fn show_new_poll_dialog(ctx: &egui::Context, state: &SharedState, backend: &BackendHandle) {
    let mut s = state.write();
    let Some(mut dialog) = s.new_poll.clone() else {
        return;
    };

    let mut close = false;
    let mut create = false;

    egui::Window::new(RichText::new("📊  Nová anketa").font(FontId::proportional(15.0)))
        .collapsible(false)
        .resizable(false)
        .default_size([360.0, 320.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Otázka")
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
            );
            ui.add(
                egui::TextEdit::singleline(&mut dialog.question)
                    .hint_text("Na co se chceš zeptat?")
                    .desired_width(ui.available_width()),
            );

            ui.add_space(10.0);
            ui.label(
                RichText::new("Možnosti")
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
            );

            let mut remove_row: Option<usize> = None;
            // Hoisted: the remove button is only offered from the third row on,
            // and `options` is mutably borrowed by the loop.
            let can_remove = dialog.options.len() > 2;
            for (index, option) in dialog.options.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(option)
                            .hint_text(format!("Možnost {}", index + 1))
                            .desired_width(ui.available_width() - 34.0),
                    );
                    // WhatsApp keeps a minimum of two rows, so only offer the
                    // remove button once a third one exists.
                    if ui
                        .add_enabled(
                            can_remove,
                            egui::Button::new(RichText::new("➖").font(FontId::proportional(13.0)))
                                .frame(false),
                        )
                        .on_hover_text("Odebrat možnost")
                        .clicked()
                    {
                        remove_row = Some(index);
                    }
                });
            }
            if let Some(index) = remove_row {
                dialog.options.remove(index);
            }

            ui.add_space(4.0);
            if dialog.options.len() < MAX_OPTIONS
                && ui
                    .button(RichText::new("➕ Přidat možnost").font(FontId::proportional(12.0)))
                    .clicked()
            {
                dialog.options.push(String::new());
            }

            ui.add_space(8.0);
            ui.checkbox(&mut dialog.multi, "Povolit více odpovědí");

            if let Some(error) = dialog.validation_error() {
                if !dialog.question.trim().is_empty() || dialog.filled_options().len() > 1 {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!("⚠ {error}"))
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(0xF1, 0x5C, 0x5C)),
                    );
                }
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Zrušit").clicked() {
                    close = true;
                }
                let can_create = dialog.validation_error().is_none();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            can_create,
                            egui::Button::new(RichText::new("Vytvořit").strong()),
                        )
                        .clicked()
                    {
                        create = true;
                    }
                });
            });
        });

    if create {
        if dialog.validation_error().is_none() {
            let options = dialog.filled_options();
            // Single-choice is the default; "více odpovědí" means one may tick
            // every option.
            let selectable_count = if dialog.multi { options.len() as u32 } else { 1 };
            let _ = backend.send_command(Command::CreatePoll {
                chat_jid: dialog.chat_jid.clone(),
                question: dialog.question.trim().to_string(),
                options,
                selectable_count,
            });
            close = true;
        }
    }

    s.new_poll = if close { None } else { Some(dialog) };
}

/// Kept for symmetry with the other dialogs: a stray `NewPollState` from a
/// previous chat must not leak into a different conversation.
pub fn cancel(state: &mut AppState) {
    state.new_poll = None;
    state.poll_draft = None;
}
