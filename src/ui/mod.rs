//! UI module exports

pub mod chat_list;
pub mod chat_info;
pub mod components;
pub mod contacts_dialog;
pub mod confirm_dialog;
pub mod conversation;
pub mod group_dialog;
pub mod poll_dialog;
pub mod qr_pairing;
pub mod settings;
pub mod export_dialog;
pub mod search_dialog;
pub mod sticker_dialog;
pub mod view_once_modal;
pub mod schedule_dialog;
pub mod call_dialog;
pub mod forward_dialog;
pub mod animations;
pub mod emoji;

use whatsapp_rust::wacore_binary::Jid;

use crate::backend::BackendHandle;
use crate::commands::Command;
use crate::state::AppState;

/// Show a chat and make sure the backend knows about it.
///
/// Every entry point into a conversation (chat list, contacts browser, new
/// group) has to do the same four things, so they share this instead of
/// drifting apart.
pub fn open_chat(state: &mut AppState, jid: &Jid, backend: &BackendHandle) {
    let canon = state.canonical_jid(jid);
    // Message ids only mean something inside one chat.
    state.selected_messages.clear();
    // A half-filled poll belongs to the chat it was opened from.
    if state.new_poll.as_ref().is_some_and(|draft| draft.chat_jid != canon) {
        crate::ui::poll_dialog::cancel(state);
    }
    state.poll_draft = None;
    state.jump_target = None;

    // Keep the half-written text with the chat it belongs to, and restore the
    // new chat's own draft instead of leaking one into the other.
    if state.selected_chat.as_ref() != Some(&canon) {
        match state.selected_chat.clone() {
            Some(previous) => state.stash_draft(&previous),
            None => state.composer_text.clear(),
        }
        state.restore_draft(&canon);
        state.pending_attachment = None;
        state.send_view_once = false;
        state.editing_message = None;
        state.replying_to = None;
    }

    state.selected_chat = Some(canon.clone());

    let target_key = if state.chats.contains_key(&canon) {
        canon.clone()
    } else {
        jid.clone()
    };
    if let Some(chat) = state.chats.get_mut(&target_key) {
        chat.unread_count = 0;
    }

    let has_messages = state
        .messages
        .get(&canon)
        .map_or(false, |m| !m.is_empty());
    let _ = backend.send_command(Command::SelectChat {
        chat_jid: Some(canon.clone()),
    });
    if !has_messages {
        let _ = backend.send_command(Command::RequestHistorySync {
            chat_jid: Some(canon),
            count: 50,
        });
    }
}
