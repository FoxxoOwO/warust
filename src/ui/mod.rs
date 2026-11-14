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
pub mod palette;
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

/// Closes the topmost open window or dialog — what Escape does.
///
/// Order matters: whatever was opened last must close first, so confirmations
/// go before the dialog that raised them.
pub fn close_topmost_dialog(state: &mut AppState) -> bool {
    if state.confirm.take().is_some() {
        return true;
    }
    if state.show_command_palette {
        state.show_command_palette = false;
        return true;
    }
    if state.show_sticker_picker {
        state.show_sticker_picker = false;
        return true;
    }
    if state.show_emoji_picker {
        state.show_emoji_picker = false;
        return true;
    }
    if state.show_contacts_dialog {
        state.show_contacts_dialog = false;
        return true;
    }
    if state.new_poll.take().is_some() {
        state.poll_draft = None;
        return true;
    }
    if state.add_member.take().is_some() {
        return true;
    }
    if state.new_group.take().is_some() {
        return true;
    }
    if state.forward_dialog.take().is_some() {
        return true;
    }
    if state.view_once_open.take().is_some() {
        return true;
    }
    if state.reaction_detail_modal.take().is_some() {
        return true;
    }
    if state.show_schedule_dialog {
        state.show_schedule_dialog = false;
        return true;
    }
    if state.show_scheduled_manager {
        state.show_scheduled_manager = false;
        return true;
    }
    if state.show_new_chat_dialog {
        state.show_new_chat_dialog = false;
        return true;
    }
    if state.show_search_dialog {
        state.show_search_dialog = false;
        return true;
    }
    false
}
