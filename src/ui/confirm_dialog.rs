//! Confirmation modal for destructive actions.
//!
//! WhatsApp asks before blocking, leaving a group, clearing or deleting a chat,
//! and so do we: the window renders whatever `AppState.confirm` holds and the
//! caller only has to push a `ConfirmRequest`.

use egui::{Color32, FontId, RichText};

use crate::backend::BackendHandle;
use crate::commands::Command;
use crate::state::{AppState, ConfirmAction, ConfirmRequest, SharedState};

/// Build a request without owning `AppState`, for callers that only have an
/// out-parameter (menus rendered while a read borrow is live).
pub fn request(
    title: impl Into<String>,
    message: impl Into<String>,
    confirm_label: impl Into<String>,
    action: ConfirmAction,
) -> ConfirmRequest {
    ConfirmRequest {
        title: title.into(),
        message: message.into(),
        confirm_label: confirm_label.into(),
        action,
    }
}

/// Queue a confirmation. Later requests replace earlier ones, which matches
/// how only one modal can be open at a time.
pub fn ask(
    state: &mut AppState,
    title: impl Into<String>,
    message: impl Into<String>,
    confirm_label: impl Into<String>,
    action: ConfirmAction,
) {
    state.confirm = Some(request(title, message, confirm_label, action));
}

fn dispatch(action: ConfirmAction, backend: &BackendHandle) {
    match action {
        ConfirmAction::BlockContact { jid } => {
            let _ = backend.send_command(Command::BlockContact { jid });
        }
        ConfirmAction::LeaveGroup { chat_jid } => {
            let _ = backend.send_command(Command::LeaveGroup { chat_jid });
        }
        ConfirmAction::ClearChat { chat_jid } => {
            let _ = backend.send_command(Command::ClearChat { chat_jid });
        }
        ConfirmAction::DeleteChat { chat_jid } => {
            let _ = backend.send_command(Command::DeleteChat { chat_jid });
        }
        ConfirmAction::RemoveMember { chat_jid, member } => {
            let _ = backend.send_command(Command::RemoveParticipants {
                chat_jid,
                participants: vec![member],
            });
        }
    }
}

pub fn show_confirm_dialog(ctx: &egui::Context, state: &SharedState, backend: &BackendHandle) {
    let mut s = state.write();
    let Some(request) = s.confirm.clone() else {
        return;
    };

    let mut close = false;
    let mut confirmed = false;

    egui::Window::new(RichText::new("⚠  Potvrzení").font(FontId::proportional(15.0)))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.add_space(4.0);
            ui.label(
                RichText::new(&request.title)
                    .font(FontId::proportional(14.0))
                    .strong(),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(&request.message)
                    .font(FontId::proportional(13.0))
                    .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Zrušit").clicked() {
                    close = true;
                }
                let danger = egui::Button::new(
                    RichText::new(&request.confirm_label)
                        .font(FontId::proportional(13.0))
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(0xC0, 0x39, 0x2B));
                if ui.add(danger).clicked() {
                    confirmed = true;
                    close = true;
                }
            });
        });

    if close {
        s.confirm = None;
    }
    drop(s);

    if confirmed {
        dispatch(request.action, backend);
    }
}
