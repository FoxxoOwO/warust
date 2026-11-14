//! Application state management

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use whatsapp_rust::wacore_binary::Jid;
use serde::{Deserialize, Serialize};
use parking_lot::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    Sending,
    Sent,
    Delivered,
    Read,
    Failed,
}

impl Default for MessageStatus {
    fn default() -> Self {
        Self::Sending
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Image,
    Video,
    Audio,
    Document,
    Sticker,
    Voice,
}

impl Default for MediaType {
    fn default() -> Self {
        Self::Image
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePreview {
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub from_me: bool,
    pub status: MessageStatus,
    pub media_type: Option<MediaType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    pub jid: Jid,
    pub name: String,
    pub last_message: Option<MessagePreview>,
    pub unread_count: u32,
    pub is_group: bool,
    pub is_archived: bool,
    pub is_pinned: bool,
    pub mute_until: Option<DateTime<Utc>>,
    pub last_activity: DateTime<Utc>,
    pub participants: HashSet<Jid>,
    #[serde(skip)]
    pub is_typing: bool,
    #[serde(skip)]
    pub typing_until: Option<DateTime<Utc>>,
}

impl Chat {
    pub fn display_name(&self) -> &str {
        &self.name
    }

    pub fn is_muted(&self) -> bool {
        self.mute_until.map_or(false, |until| until > Utc::now())
    }

    pub fn is_currently_typing(&self) -> bool {
        self.is_typing && self.typing_until.map_or(false, |until| until > Utc::now())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reaction {
    pub emoji: String,
    pub sender: Jid,
}

/// `true` for the default, so the common case can be omitted from the file.
fn is_false(value: &bool) -> bool {
    !*value
}

/// Serde default for the `edited` marker; `skip_serializing_if` keeps the
/// ~2000-byte-per-message JSON from carrying a `false` on every row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub chat_jid: Jid,
    pub sender_jid: Jid,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub from_me: bool,
    pub status: MessageStatus,
    // Everything below is optional and omitted when absent. Before this, every
    // row carried ten `"media_...":null` keys, which dominated the file size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral_expiration: Option<u32>,
    /// Set once the sender edits the message after it was sent.
    #[serde(default, skip_serializing_if = "is_false")]
    pub edited: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<Reaction>,
    // Media fields
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<MediaType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_direct_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_key: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_file_sha256: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_file_enc_sha256: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_file_length: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_mimetype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_jpeg_thumbnail: Option<Vec<u8>>,
}

impl Message {
    pub fn new_text(
        id: String,
        chat_jid: Jid,
        sender_jid: Jid,
        text: String,
        timestamp: DateTime<Utc>,
        from_me: bool,
        status: MessageStatus,
        reply_to: Option<String>,
        ephemeral_expiration: Option<u32>,
    ) -> Self {
        Self {
            id,
            chat_jid,
            sender_jid,
            text,
            timestamp,
            from_me,
            status,
            reply_to,
            ephemeral_expiration,
            edited: false,
            reactions: Vec::new(),
            media_type: None,
            media_path: None,
            media_direct_path: None,
            media_key: None,
            media_file_sha256: None,
            media_file_enc_sha256: None,
            media_file_length: None,
            media_mimetype: None,
            media_caption: None,
            media_jpeg_thumbnail: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub jid: Jid,
    pub name: Option<String>,
    pub push_name: Option<String>,
    pub verified_name: Option<String>,
    pub profile_pic_url: Option<String>,
    pub is_business: bool,
    pub is_on_whatsapp: bool,
    pub last_seen: Option<DateTime<Utc>>,
    pub is_online: bool,
    /// Whether this contact is blocked. Mirrors the server state so the info
    /// panel can show the right action without re-querying on every frame.
    #[serde(default)]
    pub is_blocked: bool,
}

impl Contact {
    pub fn new(jid: Jid, name: Option<String>) -> Self {
        Self {
            jid,
            push_name: name.clone(),
            name,
            verified_name: None,
            profile_pic_url: None,
            is_business: false,
            is_on_whatsapp: true,
            last_seen: None,
            is_online: false,
            is_blocked: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    QrCode(String),
    PairCode(String),
    Connected,
    Syncing(u8), // 0-100 percentage instead of f32
    Error(String),
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

impl Eq for ConnectionStatus {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduledStatus {
    Pending,
    Sent,
    Cancelled,
    Failed(String),
}

impl Default for ScheduledStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledMessage {
    pub id: String,
    pub chat_jid: Jid,
    pub text: String,
    pub scheduled_for: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub status: ScheduledStatus,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SettingsTab {
    #[default]
    Profile,
    Chats,
    Notifications,
    Privacy,
    Appearance,
    Storage,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub enter_is_send: bool,
    pub sound_notifications: bool,
    pub show_previews: bool,
    pub desktop_notifications: bool,
    /// One-time migration marker: `desktop_notifications` used to be a dead
    /// toggle, so a persisted `false` does not express a real choice.
    #[serde(default)]
    pub notifications_default_applied: bool,
    pub auto_download_photos: bool,
    pub auto_download_videos: bool,
    pub auto_download_docs: bool,
    pub font_size: f32,
    pub dark_theme_preset: usize,
    #[serde(default = "default_true")]
    pub run_in_tray: bool,
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    #[serde(default)]
    pub start_with_windows: bool,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            enter_is_send: true,
            sound_notifications: true,
            show_previews: true,
            desktop_notifications: true,
            notifications_default_applied: true,
            auto_download_photos: true,
            auto_download_videos: false,
            auto_download_docs: false,
            font_size: 14.0,
            dark_theme_preset: 0,
            run_in_tray: true,
            close_to_tray: true,
            start_with_windows: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallStatus {
    Incoming,
    OutgoingRinging,
    Connected,
    Ended(String),
}

#[derive(Debug, Clone)]
pub struct ActiveCall {
    pub call_id: String,
    pub peer_jid: Jid,
    pub peer_name: String,
    pub call_creator: Jid,
    pub is_video: bool,
    pub is_group: bool,
    pub status: CallStatus,
    pub is_muted: bool,
    pub is_video_enabled: bool,
    pub started_at: DateTime<Utc>,
    pub connected_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ExportDialogState {
    pub chat_jid: Jid,
    pub chat_name: String,
    pub format: crate::export::ExportFormat,
    pub status_message: Option<(bool, String)>,
    pub exported_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ForwardDialogState {
    /// Chat the messages are being forwarded from.
    pub source_chat: Jid,
    /// Ids of the ticked messages, in conversation order.
    pub message_ids: Vec<String>,
    pub search: String,
}

/// Group details that are not part of the chat row, filled by
/// `Command::RequestGroupInfo`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupInfoSnapshot {
    pub subject: String,
    pub description: Option<String>,
    /// Participants that may edit the group (the creator counts as one).
    #[serde(default)]
    pub admins: HashSet<Jid>,
    #[serde(default)]
    pub is_locked: bool,
    #[serde(default)]
    pub is_announcement: bool,
    /// Only present after `Command::GetGroupInviteLink`.
    pub invite_link: Option<String>,
}

impl GroupInfoSnapshot {
    pub fn is_admin(&self, jid: &Jid) -> bool {
        self.admins.contains(jid)
    }
}

/// State of the "new group" dialog.
#[derive(Debug, Clone, Default)]
pub struct NewGroupState {
    pub subject: String,
    pub search: String,
    pub selected: HashSet<Jid>,
    pub error: Option<String>,
}

/// State of the "add member" dialog for an existing group.
#[derive(Debug, Clone)]
pub struct AddMemberState {
    pub group_jid: Jid,
    pub search: String,
    pub selected: HashSet<Jid>,
}

/// A destructive action that asks for confirmation first.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfirmRequest {
    pub title: String,
    pub message: String,
    pub confirm_label: String,
    pub action: ConfirmAction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConfirmAction {
    BlockContact { jid: Jid },
    LeaveGroup { chat_jid: Jid },
    ClearChat { chat_jid: Jid },
    DeleteChat { chat_jid: Jid },
    RemoveMember { chat_jid: Jid, member: Jid },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub chats: HashMap<Jid, Chat>,
    pub messages: HashMap<Jid, Vec<Message>>,
    pub contacts: HashMap<Jid, Contact>,
    pub self_jid: Option<Jid>,
    pub self_name: Option<String>,
    pub connection: ConnectionStatus,
    pub selected_chat: Option<Jid>,
    pub search_query: String,
    pub auto_scroll_to_bottom: bool,
    pub composer_text: String,
    pub replying_to: Option<Message>,
    /// Id of the own message currently being edited in the composer.
    pub editing_message: Option<String>,
    pub pair_phone_number: String,
    pub show_new_chat_dialog: bool,
    pub new_chat_input: String,
    pub new_chat_error: Option<String>,
    /// Group details fetched from the server on demand (description, admins,
    /// invite link). Keyed by group JID and dropped when it goes stale.
    /// Keyed by the JID's string form: serde_json object keys must be strings,
    /// and `Jid` does not serialize as one.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub group_info: HashMap<String, GroupInfoSnapshot>,
    /// New-group dialog, `None` when closed.
    #[serde(skip)]
    pub new_group: Option<NewGroupState>,
    /// "Add member" dialog for an existing group.
    #[serde(skip)]
    pub add_member: Option<AddMemberState>,
    /// Contact browser.
    #[serde(skip)]
    pub show_contacts_dialog: bool,
    #[serde(skip)]
    pub contacts_search: String,
    /// Pending destructive action awaiting confirmation.
    #[serde(skip)]
    pub confirm: Option<ConfirmRequest>,
    /// Group JIDs whose metadata we already asked for, so the panel does not
    /// fire a request on every frame.
    #[serde(skip)]
    pub group_info_pending: HashSet<String>,
    /// Inline editors for the group subject and description.
    #[serde(skip)]
    pub group_subject_draft: Option<(Jid, String)>,
    #[serde(skip)]
    pub group_desc_draft: Option<(Jid, String)>,
    pub lid_to_pn: HashMap<Jid, Jid>,
    #[serde(skip)]
    pub editing_contact: Option<(Jid, String)>,
    // Scheduled messages
    #[serde(default)]
    pub scheduled_messages: Vec<ScheduledMessage>,
    #[serde(skip)]
    pub show_schedule_dialog: bool,
    #[serde(skip)]
    pub schedule_target_chat: Option<Jid>,
    #[serde(skip)]
    pub schedule_text: String,
    #[serde(skip)]
    pub schedule_delay_minutes: u32,
    #[serde(skip)]
    pub show_scheduled_manager: bool,
    // Message search with filters
    #[serde(skip)]
    pub show_search_dialog: bool,
    #[serde(skip)]
    pub search_filter: crate::search::MessageSearchFilter,
    // Conversation export dialog
    #[serde(skip)]
    pub export_dialog: Option<ExportDialogState>,
    // Media attachment pending to be sent
    #[serde(skip)]
    pub pending_attachment: Option<std::path::PathBuf>,
    // User preferences & settings
    #[serde(default)]
    pub preferences: UserPreferences,
    #[serde(skip)]
    pub settings_tab: SettingsTab,
    // Reaction details modal
    #[serde(skip)]
    pub reaction_detail_modal: Option<(Jid, String)>,
    // Active / incoming / outgoing call state
    #[serde(skip)]
    pub active_call: Option<ActiveCall>,
    // Emoji Picker state
    #[serde(skip)]
    pub show_emoji_picker: bool,
    #[serde(skip)]
    pub emoji_search: String,
    #[serde(skip)]
    pub emoji_active_category: usize,
    #[serde(skip)]
    pub reaction_picker_target: Option<String>,
    #[serde(skip)]
    pub pending_reactions: HashMap<(Jid, String), Vec<Reaction>>,
    /// Set to true once initial 20-message history has been fetched on first login
    #[serde(default)]
    pub initial_history_fetched: bool,
    /// Set by any content mutation; cleared after a successful persist so the
    /// periodic writer can skip no-op serialization of the whole message store.
    #[serde(skip)]
    pub dirty: bool,
    /// Ids of the messages ticked in the bulk-action bar of the open chat.
    /// Non-empty means selection mode is active.
    #[serde(skip)]
    pub selected_messages: std::collections::BTreeSet<String>,
    /// Open "forward to…" chat picker.
    #[serde(skip)]
    pub forward_dialog: Option<ForwardDialogState>,
    /// Whether the right-hand info panel is open for the current chat.
    #[serde(skip)]
    pub show_chat_info: bool,
    /// Disappearing-message timer per chat, in seconds. WhatsApp calls this
    /// "ephemeral" and it applies to 1:1 chats as well as groups; a zero or
    /// missing entry means the timer is off.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub ephemeral: HashMap<Jid, u32>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            chats: HashMap::new(),
            messages: HashMap::new(),
            contacts: HashMap::new(),
            self_jid: None,
            self_name: None,
            connection: ConnectionStatus::default(),
            selected_chat: None,
            search_query: String::new(),
            auto_scroll_to_bottom: true,
            composer_text: String::new(),
            replying_to: None,
            editing_message: None,
            pair_phone_number: String::new(),
            show_new_chat_dialog: false,
            new_chat_input: String::new(),
            new_chat_error: None,
            group_info: HashMap::new(),
            new_group: None,
            add_member: None,
            show_contacts_dialog: false,
            contacts_search: String::new(),
            confirm: None,
            group_info_pending: HashSet::new(),
            group_subject_draft: None,
            group_desc_draft: None,
            lid_to_pn: HashMap::new(),
            editing_contact: None,
            scheduled_messages: Vec::new(),
            show_schedule_dialog: false,
            schedule_target_chat: None,
            schedule_text: String::new(),
            schedule_delay_minutes: 10,
            show_scheduled_manager: false,
            show_search_dialog: false,
            search_filter: crate::search::MessageSearchFilter::default(),
            export_dialog: None,
            pending_attachment: None,
            preferences: UserPreferences::default(),
            settings_tab: SettingsTab::default(),
            reaction_detail_modal: None,
            active_call: None,
            show_emoji_picker: false,
            emoji_search: String::new(),
            emoji_active_category: 0,
            reaction_picker_target: None,
            pending_reactions: HashMap::new(),
            initial_history_fetched: false,
            dirty: false,
            selected_messages: std::collections::BTreeSet::new(),
            forward_dialog: None,
            show_chat_info: false,
            ephemeral: HashMap::new(),
        }
    }
}

pub fn is_reaction_only(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let stripped: String = trimmed.chars().filter(|c| *c != '\u{FE0F}').collect();
    if matches!(
        stripped.as_str(),
        "❤️" | "❤" | "💀" | "😂" | "👍" | "👎" | "🙏" | "🎉" | "😮" | "😢" | "🔥" | "👏" | "😍" | "😭" | "🤔" | "🥰" | "✨" | "👀" | "💯"
    ) {
        return true;
    }
    if stripped.chars().count() == 1 && is_emoji_char(stripped.chars().next().unwrap()) {
        return true;
    }
    false
}

pub fn is_emoji_char(c: char) -> bool {
    matches!(c as u32,
        0x1F600..=0x1F64F | // Emoticons
        0x1F300..=0x1F5FF | // Misc Symbols and Pictographs
        0x1F680..=0x1F6FF | // Transport and Map
        0x1F700..=0x1F77F | // Alchemical Symbols
        0x1F780..=0x1F7FF | // Geometric Shapes Extended
        0x1F800..=0x1F8FF | // Supplemental Arrows-C
        0x1F900..=0x1F9FF | // Supplemental Symbols and Pictographs
        0x1FA00..=0x1FA6F | // Chess Symbols
        0x1FA70..=0x1FAFF | // Symbols and Pictographs Extended-A
        0x2600..=0x26FF   | // Misc symbols (hearts, stars, etc)
        0x2700..=0x27BF   | // Dingbats
        0x2300..=0x23FF   | // Misc Technical
        0x2B50 | 0x2B55 | 0x200D | 0xFE0F // Stars, circles, ZWJ, variation selector
    )
}

/// Returns Some(count) if the text consists ONLY of 1, 2, or 3 emojis (so they can be enlarged)
pub fn count_emoji_only(text: &str) -> Option<usize> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut count = 0;
    for c in trimmed.chars() {
        if c.is_whitespace() || c == '\u{FE0F}' || c == '\u{200D}' {
            continue;
        }
        if is_emoji_char(c) {
            count += 1;
        } else {
            return None;
        }
    }
    if count > 0 && count <= 3 {
        Some(count)
    } else {
        None
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn canonical_jid(&self, jid: &Jid) -> Jid {
        if jid.server.as_str() == "lid" {
            if let Some(pn) = self.lid_to_pn.get(jid) {
                return pn.clone();
            }
        }
        jid.clone()
    }

    pub fn get_chat_messages(&self, jid: &Jid) -> Option<&Vec<Message>> {
        let canon = self.canonical_jid(jid);
        if let Some(msgs) = self.messages.get(&canon) {
            if !msgs.is_empty() {
                return Some(msgs);
            }
        }
        if canon != *jid {
            if let Some(msgs) = self.messages.get(jid) {
                if !msgs.is_empty() {
                    return Some(msgs);
                }
            }
        }
        for (lid, pn) in &self.lid_to_pn {
            if pn == jid || pn == &canon {
                if let Some(msgs) = self.messages.get(lid) {
                    if !msgs.is_empty() {
                        return Some(msgs);
                    }
                }
            }
        }
        self.messages.get(&canon).or_else(|| self.messages.get(jid))
    }

    pub fn get_or_create_chat(&mut self, jid: Jid, name: String, is_group: bool) -> &mut Chat {
        let canon = if is_group { jid } else { self.canonical_jid(&jid) };
        self.chats.entry(canon.clone()).or_insert_with(|| Chat {
            jid: canon,
            name,
            last_message: None,
            unread_count: 0,
            is_group,
            is_archived: false,
            is_pinned: false,
            mute_until: None,
            last_activity: chrono::DateTime::<Utc>::UNIX_EPOCH,
            participants: HashSet::new(),
            is_typing: false,
            typing_until: None,
        })
    }

    pub fn add_message(&mut self, mut msg: Message) {
        self.dirty = true;
        let canon_chat_jid = self.canonical_jid(&msg.chat_jid);
        msg.chat_jid = canon_chat_jid.clone();
        let msgs = self.messages.entry(canon_chat_jid.clone()).or_default();

        // If message is a standalone reaction text without media, attach as reaction to previous message instead of showing as bubble
        if is_reaction_only(&msg.text) && msg.media_type.is_none() {
            if let Some(target) = msgs.iter_mut().rev().find(|m| m.media_type.is_some() || !m.text.is_empty()) {
                target.reactions.retain(|r| r.sender != msg.sender_jid);
                target.reactions.push(Reaction {
                    emoji: msg.text.clone(),
                    sender: msg.sender_jid,
                });
                return;
            }
        }

        // Attach any pending reactions for this message
        if let Some(pending) = self.pending_reactions.remove(&(canon_chat_jid.clone(), msg.id.clone())) {
            for r in pending {
                msg.reactions.retain(|existing| existing.sender != r.sender);
                msg.reactions.push(r);
            }
        }

        if let Some(pos) = msgs.iter().position(|m| m.id == msg.id) {
            msgs[pos].status = msg.status;
        } else {
            // Keep messages sorted by timestamp
            let idx = msgs.partition_point(|m| m.timestamp <= msg.timestamp);
            msgs.insert(idx, msg);
        }

        // Always update chat preview from true latest non-reaction message on canonical and any mapped LID chat
        let preview = msgs.iter().rev().find(|m| !is_reaction_only(&m.text)).map(|latest| MessagePreview {
            text: latest.text.clone(),
            timestamp: latest.timestamp,
            from_me: latest.from_me,
            status: latest.status,
            media_type: latest.media_type,
        });
        let last_act = msgs.last().map(|latest| latest.timestamp);

        if let Some(chat) = self.chats.get_mut(&canon_chat_jid) {
            chat.last_message = preview.clone();
            if let Some(ts) = last_act {
                chat.last_activity = ts;
            }
        }
        for (lid, pn) in &self.lid_to_pn {
            if pn == &canon_chat_jid {
                if let Some(chat) = self.chats.get_mut(lid) {
                    chat.last_message = preview.clone();
                    if let Some(ts) = last_act {
                        chat.last_activity = ts;
                    }
                }
            }
        }
    }

    pub fn add_messages_batch(&mut self, new_msgs: Vec<Message>) {
        if new_msgs.is_empty() {
            return;
        }
        self.dirty = true;

        let mut affected_chats = HashSet::new();

        for mut msg in new_msgs {
            let canon_chat_jid = self.canonical_jid(&msg.chat_jid);
            msg.chat_jid = canon_chat_jid.clone();
            affected_chats.insert(canon_chat_jid.clone());
            let msgs = self.messages.entry(canon_chat_jid.clone()).or_default();

            if is_reaction_only(&msg.text) && msg.media_type.is_none() {
                if let Some(target) = msgs.iter_mut().rev().find(|m| m.media_type.is_some() || !m.text.is_empty()) {
                    target.reactions.retain(|r| r.sender != msg.sender_jid);
                    target.reactions.push(Reaction {
                        emoji: msg.text.clone(),
                        sender: msg.sender_jid,
                    });
                    continue;
                }
            }

            if let Some(pending) = self.pending_reactions.remove(&(canon_chat_jid.clone(), msg.id.clone())) {
                for r in pending {
                    msg.reactions.retain(|existing| existing.sender != r.sender);
                    msg.reactions.push(r);
                }
            }

            if let Some(pos) = msgs.iter().position(|m| m.id == msg.id) {
                msgs[pos].status = msg.status;
            } else {
                msgs.push(msg);
            }
        }

        // Sort each affected chat once and update preview
        for chat_jid in affected_chats {
            if let Some(msgs) = self.messages.get_mut(&chat_jid) {
                msgs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

                let preview = msgs.iter().rev().find(|m| !is_reaction_only(&m.text)).map(|latest| MessagePreview {
                    text: latest.text.clone(),
                    timestamp: latest.timestamp,
                    from_me: latest.from_me,
                    status: latest.status,
                    media_type: latest.media_type,
                });
                let last_act = msgs.last().map(|latest| latest.timestamp);

                if let Some(chat) = self.chats.get_mut(&chat_jid) {
                    chat.last_message = preview.clone();
                    if let Some(ts) = last_act {
                        chat.last_activity = ts;
                    }
                }
                for (lid, pn) in &self.lid_to_pn {
                    if pn == &chat_jid {
                        if let Some(chat) = self.chats.get_mut(lid) {
                            chat.last_message = preview.clone();
                            if let Some(ts) = last_act {
                                chat.last_activity = ts;
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn clean_stray_reactions(&mut self) {
        self.dirty = true;
        for (_jid, msgs) in self.messages.iter_mut() {
            let mut reactions_to_attach: Vec<(Jid, String)> = Vec::new();
            msgs.retain(|m| {
                if is_reaction_only(&m.text) && m.media_type.is_none() {
                    reactions_to_attach.push((m.sender_jid.clone(), m.text.clone()));
                    false
                } else {
                    true
                }
            });
            if !reactions_to_attach.is_empty() {
                for (sender, emoji) in reactions_to_attach {
                    if let Some(target) = msgs.iter_mut().rev().find(|m| m.media_type.is_some() || !m.text.is_empty()) {
                        target.reactions.retain(|r| r.sender != sender);
                        target.reactions.push(Reaction {
                            emoji,
                            sender,
                        });
                    }
                }
            }
        }
    }

    /// Consolidate chats and messages so that if a LID maps to a PN, they are merged under the canonical PN.
    pub fn consolidate_canonical_chats(&mut self) {
        self.dirty = true;
        let mappings: Vec<(Jid, Jid)> = self.lid_to_pn.iter().map(|(l, p)| (l.clone(), p.clone())).collect();
        for (lid_jid, pn_jid) in mappings {
            // 1. Move any messages from lid_jid into pn_jid
            if let Some(lid_msgs) = self.messages.remove(&lid_jid) {
                let pn_msgs = self.messages.entry(pn_jid.clone()).or_default();
                for msg in lid_msgs {
                    if !pn_msgs.iter().any(|m| m.id == msg.id) {
                        let mut m = msg;
                        m.chat_jid = pn_jid.clone();
                        pn_msgs.push(m);
                    }
                }
                pn_msgs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            }

            // 2. Merge chats: if lid chat exists
            if let Some(lid_chat) = self.chats.remove(&lid_jid) {
                let pn_chat = self.chats.entry(pn_jid.clone()).or_insert_with(|| Chat {
                    jid: pn_jid.clone(),
                    name: lid_chat.name.clone(),
                    last_message: lid_chat.last_message.clone(),
                    unread_count: lid_chat.unread_count,
                    is_group: false,
                    is_archived: lid_chat.is_archived,
                    is_pinned: lid_chat.is_pinned,
                    mute_until: lid_chat.mute_until,
                    last_activity: lid_chat.last_activity,
                    participants: HashSet::new(),
                    is_typing: false,
                    typing_until: None,
                });
                if crate::util::is_real_name(&lid_chat.name) && !crate::util::is_real_name(&pn_chat.name) {
                    pn_chat.name = lid_chat.name;
                }
                if lid_chat.last_activity > pn_chat.last_activity {
                    pn_chat.last_activity = lid_chat.last_activity;
                    if lid_chat.last_message.is_some() {
                        pn_chat.last_message = lid_chat.last_message;
                    }
                }
                if lid_chat.is_pinned {
                    pn_chat.is_pinned = true;
                }
            }

            // 3. If selected_chat was the LID, update to PN
            if self.selected_chat.as_ref() == Some(&lid_jid) {
                self.selected_chat = Some(pn_jid.clone());
            }

            // 4. Update preview from true latest message in pn_jid
            if let Some(msgs) = self.messages.get(&pn_jid) {
                if let Some(latest) = msgs.iter().rev().find(|m| !is_reaction_only(&m.text)) {
                    if let Some(chat) = self.chats.get_mut(&pn_jid) {
                        chat.last_message = Some(MessagePreview {
                            text: latest.text.clone(),
                            timestamp: latest.timestamp,
                            from_me: latest.from_me,
                            status: latest.status,
                            media_type: latest.media_type,
                        });
                        chat.last_activity = latest.timestamp;
                    }
                }
            }
        }

        // 5. Recompute last_message and last_activity for ALL chats from their messages or existing preview
        let lid_map = self.lid_to_pn.clone();
        for (chat_jid, chat) in self.chats.iter_mut() {
            let canon = if chat.is_group {
                chat_jid.clone()
            } else {
                lid_map.get(chat_jid).cloned().unwrap_or_else(|| chat_jid.clone())
            };
            if let Some(msgs) = self.messages.get(&canon) {
                if let Some(latest) = msgs.iter().rev().find(|m| !is_reaction_only(&m.text)) {
                    chat.last_message = Some(MessagePreview {
                        text: latest.text.clone(),
                        timestamp: latest.timestamp,
                        from_me: latest.from_me,
                        status: latest.status,
                        media_type: latest.media_type,
                    });
                    if latest.timestamp > chat.last_activity {
                        chat.last_activity = latest.timestamp;
                    }
                } else if let Some(last_any) = msgs.last() {
                    if last_any.timestamp > chat.last_activity {
                        chat.last_activity = last_any.timestamp;
                    }
                }
            } else if let Some(lm) = &chat.last_message {
                if lm.timestamp > chat.last_activity {
                    chat.last_activity = lm.timestamp;
                }
            }
        }

        // 6. Update names of all 1-on-1 chats if a human display name is available
        let updates: Vec<(Jid, String)> = self.chats.iter()
            .filter_map(|(jid, chat)| {
                if !chat.is_group {
                    if let Some(resolved) = self.resolve_display_name(jid) {
                        if resolved != chat.name {
                            return Some((jid.clone(), resolved));
                        }
                    }
                }
                None
            })
            .collect();
        for (jid, name) in updates {
            if let Some(chat) = self.chats.get_mut(&jid) {
                chat.name = name;
            }
        }

        // Prune empty 1-on-1 chats that have no message history, no real name, and are not selected/pinned
        self.chats.retain(|jid, chat| {
            chat.is_pinned
                || chat.is_group
                || crate::util::is_real_name(&chat.name)
                || chat.last_message.is_some()
                || self.messages.get(jid).map_or(false, |m| !m.is_empty())
                || self.selected_chat.as_ref() == Some(jid)
        });
    }

    /// Compute the effective most recent timestamp for a chat across all sources:
    /// 1. Messages loaded in `self.messages` (using canonical or direct JID lookup)
    /// 2. `chat.last_message` preview
    /// 3. `chat.last_activity`
    pub fn get_chat_latest_timestamp(&self, chat: &Chat) -> chrono::DateTime<chrono::Utc> {
        let mut best = chat.last_activity;
        if let Some(lm) = &chat.last_message {
            if lm.timestamp > best {
                best = lm.timestamp;
            }
        }
        if let Some(msgs) = self.get_chat_messages(&chat.jid) {
            if let Some(last_msg) = msgs.iter().map(|m| m.timestamp).max() {
                if last_msg > best {
                    best = last_msg;
                }
            }
        }
        best
    }

    pub fn filtered_chats(&self) -> Vec<&Chat> {
        let mut chats: Vec<_> = self.chats.values().collect();
        
        // Filter by search query
        if !self.search_query.is_empty() {
            let query = self.search_query.to_lowercase();
            chats.retain(|c| c.name.to_lowercase().contains(&query));
        } else {
            // When not searching, show chats that:
            // 1. Have a known human name, OR
            // 2. Have messages / last_message, OR
            // 3. Are groups, OR
            // 4. Are pinned, OR
            // 5. Are currently selected
            chats.retain(|c| {
                c.is_pinned
                    || c.is_group
                    || crate::util::is_real_name(&c.name)
                    || self.selected_chat.as_ref().map_or(false, |sel| self.canonical_jid(sel) == self.canonical_jid(&c.jid))
                    || c.last_message.is_some()
                    || self.messages.get(&self.canonical_jid(&c.jid)).map_or(false, |m| !m.is_empty())
            });
        }
        
        // Sort: pinned first, then by latest message / activity (newest first)
        chats.sort_by(|a, b| {
            match (a.is_pinned, b.is_pinned) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    let ts_a = self.get_chat_latest_timestamp(a);
                    let ts_b = self.get_chat_latest_timestamp(b);
                    match ts_b.cmp(&ts_a) {
                        std::cmp::Ordering::Equal => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        other => other,
                    }
                }
            }
        });
        
        chats
    }

    pub fn apply_reaction(&mut self, chat_jid: &Jid, target_msg_id: &str, sender_jid: &Jid, emoji: &str) -> bool {
        self.dirty = true;
        let canon_chat = self.canonical_jid(chat_jid);
        if let Some(msgs) = self.messages.get_mut(&canon_chat) {
            if let Some(msg) = msgs.iter_mut().find(|m| m.id == target_msg_id) {
                msg.reactions.retain(|r| &r.sender != sender_jid);
                if !emoji.is_empty() {
                    msg.reactions.push(Reaction {
                        emoji: emoji.to_string(),
                        sender: sender_jid.clone(),
                    });
                }
                return true;
            }
        }
        // Save in pending reactions if message has not loaded yet
        if !emoji.is_empty() {
            let entry = self.pending_reactions.entry((canon_chat, target_msg_id.to_string())).or_default();
            entry.retain(|r| &r.sender != sender_jid);
            entry.push(Reaction {
                emoji: emoji.to_string(),
                sender: sender_jid.clone(),
            });
        }
        false
    }

    /// Tick or untick a message in the bulk-action selection.
    ///
    /// Un-ticking the last message leaves the set empty, and an empty set is
    /// what ends selection mode — the same effect as the ✕ in the bar, so no
    /// separate mode flag is needed.
    pub fn toggle_selected(&mut self, message_id: &str) {
        if !self.selected_messages.remove(message_id) {
            self.selected_messages.insert(message_id.to_string());
        }
    }

    /// Apply an edit to an already-stored message.
    ///
    /// WhatsApp keeps the original timestamp and id for an edit, so only the
    /// body and the `edited` marker change. Returns false when the target is not
    /// in the local store (e.g. it predates this install).
    pub fn apply_edit(&mut self, chat_jid: &Jid, target_msg_id: &str, new_text: &str) -> bool {
        let canon_chat = self.canonical_jid(chat_jid);
        let applied = match self.messages.get_mut(&canon_chat) {
            Some(msgs) => match msgs.iter_mut().find(|m| m.id == target_msg_id) {
                Some(msg) => {
                    msg.text = new_text.to_string();
                    msg.edited = true;
                    true
                }
                None => false,
            },
            None => false,
        };
        if applied {
            self.dirty = true;
            self.refresh_preview(&canon_chat);
        }
        applied
    }

    /// Rebuild a chat's list preview (and its LID alias) from the newest
    /// non-reaction message.
    fn refresh_preview(&mut self, canon_chat: &Jid) {
        let latest = match self
            .messages
            .get(canon_chat)
            .and_then(|msgs| msgs.iter().rev().find(|m| !is_reaction_only(&m.text)))
        {
            Some(latest) => latest.clone(),
            None => return,
        };
        let preview = MessagePreview {
            text: latest.text.clone(),
            timestamp: latest.timestamp,
            from_me: latest.from_me,
            status: latest.status,
            media_type: latest.media_type,
        };
        let aliases: Vec<Jid> = self
            .lid_to_pn
            .iter()
            .filter(|(_, pn)| *pn == canon_chat)
            .map(|(lid, _)| lid.clone())
            .collect();
        if let Some(chat) = self.chats.get_mut(canon_chat) {
            chat.last_message = Some(preview.clone());
        }
        for lid in aliases {
            if let Some(chat) = self.chats.get_mut(&lid) {
                chat.last_message = Some(preview.clone());
            }
        }
    }

    /// Leaves edit mode, dropping both the id and the composer text.
    pub fn cancel_edit(&mut self) {
        if self.editing_message.take().is_some() {
            self.composer_text.clear();
        }
    }

    pub fn resolve_display_name(&self, jid: &Jid) -> Option<String> {
        use crate::util::is_real_name;

        // 1. Direct contact match (human name preferred over phone number)
        if let Some(c) = self.contacts.get(jid) {
            if let Some(name) = c.name.as_ref().filter(|s| is_real_name(s)) {
                return Some(name.clone());
            }
            if let Some(vn) = c.verified_name.as_ref().filter(|s| is_real_name(s)) {
                return Some(vn.clone());
            }
            if let Some(pn) = c.push_name.as_ref().filter(|s| is_real_name(s)) {
                return Some(pn.clone());
            }
        }
        // 2. If it's a LID, check mapped phone number JID for human name
        if let Some(pn_jid) = self.lid_to_pn.get(jid) {
            if let Some(c) = self.contacts.get(pn_jid) {
                if let Some(name) = c.name.as_ref().filter(|s| is_real_name(s)) {
                    return Some(name.clone());
                }
                if let Some(vn) = c.verified_name.as_ref().filter(|s| is_real_name(s)) {
                    return Some(vn.clone());
                }
                if let Some(pn) = c.push_name.as_ref().filter(|s| is_real_name(s)) {
                    return Some(pn.clone());
                }
            }
        }
        // 3. If it's a PN, check if any LID maps to it for human name
        for (lid_jid, pn_jid) in &self.lid_to_pn {
            if pn_jid == jid {
                if let Some(c) = self.contacts.get(lid_jid) {
                    if let Some(name) = c.name.as_ref().filter(|s| is_real_name(s)) {
                        return Some(name.clone());
                    }
                    if let Some(vn) = c.verified_name.as_ref().filter(|s| is_real_name(s)) {
                        return Some(vn.clone());
                    }
                    if let Some(pn) = c.push_name.as_ref().filter(|s| is_real_name(s)) {
                        return Some(pn.clone());
                    }
                }
            }
        }
        // 4. Check if chat itself has a real human name (direct or partner)
        if let Some(chat) = self.chats.get(jid) {
            if is_real_name(&chat.name) {
                return Some(chat.name.clone());
            }
        }
        if let Some(pn_jid) = self.lid_to_pn.get(jid) {
            if let Some(chat) = self.chats.get(pn_jid) {
                if is_real_name(&chat.name) {
                    return Some(chat.name.clone());
                }
            }
        }
        for (lid_jid, pn_jid) in &self.lid_to_pn {
            if pn_jid == jid {
                if let Some(chat) = self.chats.get(lid_jid) {
                    if is_real_name(&chat.name) {
                        return Some(chat.name.clone());
                    }
                }
            }
        }
        // 5. Fallback: Phone number formatting
        if jid.server.as_str() == "s.whatsapp.net" {
            return Some(crate::util::format_jid(jid));
        }
        if let Some(pn_jid) = self.lid_to_pn.get(jid) {
            return Some(crate::util::format_jid(pn_jid));
        }
        None
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_create_chat() {
        let mut state = AppState::new();
        let jid = Jid::pn("1234567890");
        let chat = state.get_or_create_chat(jid.clone(), "Test Chat".into(), false);
        assert_eq!(chat.name, "Test Chat");
        assert_eq!(chat.unread_count, 0);
        assert!(!chat.is_group);
    }

    #[test]
    fn test_add_message_updates_chat_preview() {
        let mut state = AppState::new();
        let chat_jid = Jid::pn("1234567890");
        let sender_jid = chat_jid.clone();
        state.get_or_create_chat(chat_jid.clone(), "Alice".into(), false);

        let msg = Message::new_text(
            "msg-1".into(),
            chat_jid.clone(),
            sender_jid,
            "Hello there!".into(),
            Utc::now(),
            false,
            MessageStatus::Delivered,
            None,
            None,
        );
        state.add_message(msg);

        let chat = state.chats.get(&chat_jid).unwrap();
        assert!(chat.last_message.is_some());
        assert_eq!(chat.last_message.as_ref().unwrap().text, "Hello there!");
        assert_eq!(state.messages.get(&chat_jid).unwrap().len(), 1);
    }

    #[test]
    fn test_filtered_chats_pinned_and_search() {
        let mut state = AppState::new();
        let jid1 = Jid::pn("1111111111");
        let jid2 = Jid::pn("2222222222");

        state.get_or_create_chat(jid1.clone(), "Alice Smith".into(), false);
        state.get_or_create_chat(jid2.clone(), "Bob Jones".into(), false);

        // Pin Bob
        state.chats.get_mut(&jid2).unwrap().is_pinned = true;
        state.selected_chat = Some(jid1.clone());

        let filtered = state.filtered_chats();
        assert_eq!(filtered[0].jid, jid2); // Pinned Bob comes first
        assert_eq!(filtered[1].jid, jid1);

        // Search for Alice
        state.search_query = "alice".into();
        let search_res = state.filtered_chats();
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].name, "Alice Smith");
    }

    #[test]
    fn test_consolidate_canonical_chats() {
        let mut state = AppState::new();
        let lid_jid = "93935397523564@lid".parse::<Jid>().unwrap();
        let pn_jid = "420773606068@s.whatsapp.net".parse::<Jid>().unwrap();

        state.lid_to_pn.insert(lid_jid.clone(), pn_jid.clone());

        // Create LID chat with 1 outgoing message
        state.chats.insert(lid_jid.clone(), Chat {
            jid: lid_jid.clone(),
            name: "+420 773 606 068".into(),
            last_message: None,
            unread_count: 0,
            is_group: false,
            is_archived: false,
            is_pinned: false,
            mute_until: None,
            last_activity: chrono::DateTime::<Utc>::UNIX_EPOCH,
            participants: HashSet::new(),
            is_typing: false,
            typing_until: None,
        });
        state.messages.insert(lid_jid.clone(), vec![
            Message::new_text("msg-lid".into(), lid_jid.clone(), pn_jid.clone(), "Hello from LID".into(), Utc::now(), true, MessageStatus::Sent, None, None)
        ]);

        // Create PN chat with history messages
        state.chats.insert(pn_jid.clone(), Chat {
            jid: pn_jid.clone(),
            name: "John Doe".into(),
            last_message: None,
            unread_count: 0,
            is_group: false,
            is_archived: false,
            is_pinned: false,
            mute_until: None,
            last_activity: Utc::now() - chrono::Duration::hours(1),
            participants: HashSet::new(),
            is_typing: false,
            typing_until: None,
        });
        state.messages.insert(pn_jid.clone(), vec![
            Message::new_text("msg-pn".into(), pn_jid.clone(), pn_jid.clone(), "Hello from PN history".into(), Utc::now() - chrono::Duration::hours(1), false, MessageStatus::Read, None, None)
        ]);

        state.selected_chat = Some(lid_jid.clone());

        // Consolidate
        state.consolidate_canonical_chats();

        // 1. Only canonical PN chat should exist in chats
        assert_eq!(state.chats.len(), 1);
        assert!(state.chats.contains_key(&pn_jid));
        assert!(!state.chats.contains_key(&lid_jid));

        // 2. Name should be human name "John Doe"
        assert_eq!(state.chats.get(&pn_jid).unwrap().name, "John Doe");

        // 3. Selected chat should now be pn_jid
        assert_eq!(state.selected_chat, Some(pn_jid.clone()));

        // 4. Messages should be merged (total 2) and sorted
        let msgs = state.get_chat_messages(&pn_jid).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].id, "msg-pn");
        assert_eq!(msgs[1].id, "msg-lid");

        // 5. Querying with lid_jid should also return the unified messages
        let msgs_via_lid = state.get_chat_messages(&lid_jid).unwrap();
        assert_eq!(msgs_via_lid.len(), 2);
    }

    #[test]
    fn test_reactions_and_named_chats() {
        let mut state = AppState::new();
        let chat_jid = "420773606068@s.whatsapp.net".parse::<Jid>().unwrap();
        let friend_jid = "420773606068@s.whatsapp.net".parse::<Jid>().unwrap();
        let self_jid = "420123456789@s.whatsapp.net".parse::<Jid>().unwrap();

        // 1. Add chat with real name and 0 messages -> should be shown in filtered_chats
        state.get_or_create_chat(chat_jid.clone(), "Petr".into(), false);
        let chats = state.filtered_chats();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].name, "Petr");

        // 2. Add message and apply reaction
        let msg = Message::new_text(
            "msg-photo-1".into(),
            chat_jid.clone(),
            self_jid.clone(),
            "📷 Image".into(),
            Utc::now(),
            true,
            MessageStatus::Sent,
            None,
            None,
        );
        state.add_message(msg);

        // Petr reacts with ❤️
        let applied = state.apply_reaction(&chat_jid, "msg-photo-1", &friend_jid, "❤️");
        assert!(applied);

        let msgs = state.get_chat_messages(&chat_jid).unwrap();
        assert_eq!(msgs.len(), 1); // Still only 1 message, NOT a new bubble!
        assert_eq!(msgs[0].reactions.len(), 1);
        assert_eq!(msgs[0].reactions[0].emoji, "❤️");

        // Petr changes reaction to 💀
        let applied2 = state.apply_reaction(&chat_jid, "msg-photo-1", &friend_jid, "💀");
        assert!(applied2);
        let msgs2 = state.get_chat_messages(&chat_jid).unwrap();
        assert_eq!(msgs2[0].reactions.len(), 1);
        assert_eq!(msgs2[0].reactions[0].emoji, "💀");

        // Check preview is NOT the emoji
        let chat = state.chats.get(&chat_jid).unwrap();
        assert_eq!(chat.last_message.as_ref().unwrap().text, "📷 Image");

        // 3. Test clean_stray_reactions if stray emoji messages existed
        state.messages.get_mut(&chat_jid).unwrap().push(Message::new_text(
            "stray-1".into(),
            chat_jid.clone(),
            friend_jid.clone(),
            "❤️".into(),
            Utc::now(),
            false,
            MessageStatus::Delivered,
            None,
            None,
        ));
        assert_eq!(state.messages.get(&chat_jid).unwrap().len(), 2);
        state.clean_stray_reactions();
        assert_eq!(state.messages.get(&chat_jid).unwrap().len(), 1); // Stripped stray emoji message bubble
        assert_eq!(state.messages.get(&chat_jid).unwrap()[0].reactions[0].emoji, "❤️");

        // 4. Test pending_reactions when reaction arrives BEFORE message
        state.apply_reaction(&chat_jid, "msg-future-99", &friend_jid, "🔥");
        let future_msg = Message::new_text(
            "msg-future-99".into(),
            chat_jid.clone(),
            self_jid.clone(),
            "New incoming photo".into(),
            Utc::now(),
            true,
            MessageStatus::Sent,
            None,
            None,
        );
        state.add_message(future_msg);
        let msgs3 = state.get_chat_messages(&chat_jid).unwrap();
        let target_future = msgs3.iter().find(|m| m.id == "msg-future-99").unwrap();
        assert_eq!(target_future.reactions.len(), 1);
        assert_eq!(target_future.reactions[0].emoji, "🔥");
    }

    #[test]
    fn test_call_state_transitions() {
        let mut state = AppState::new();
        let peer_jid = "420773606068@s.whatsapp.net".parse::<Jid>().unwrap();
        let creator_jid = peer_jid.clone();

        // 1. Incoming call
        state.active_call = Some(ActiveCall {
            call_id: "call-123".into(),
            peer_jid: peer_jid.clone(),
            peer_name: "Petr".into(),
            call_creator: creator_jid,
            is_video: false,
            is_group: false,
            status: CallStatus::Incoming,
            is_muted: false,
            is_video_enabled: false,
            started_at: Utc::now(),
            connected_at: None,
            ended_at: None,
        });

        assert!(state.active_call.is_some());
        assert_eq!(state.active_call.as_ref().unwrap().status, CallStatus::Incoming);

        // 2. Connect
        if let Some(ref mut call) = state.active_call {
            call.status = CallStatus::Connected;
            call.connected_at = Some(Utc::now());
        }
        assert_eq!(state.active_call.as_ref().unwrap().status, CallStatus::Connected);

        // 3. End
        if let Some(ref mut call) = state.active_call {
            call.status = CallStatus::Ended("Hovor ukončen".into());
            call.ended_at = Some(Utc::now());
        }
        assert_eq!(state.active_call.as_ref().unwrap().status, CallStatus::Ended("Hovor ukončen".into()));
    }

    #[test]
    fn test_chat_sorting_order() {
        let mut state = AppState::new();
        let jid_petr = "420111111111@s.whatsapp.net".parse::<Jid>().unwrap();
        let jid_aliexpress = "420222222222@s.whatsapp.net".parse::<Jid>().unwrap();
        let jid_vojta = "420333333333@s.whatsapp.net".parse::<Jid>().unwrap();
        let jid_empty = "420444444444@s.whatsapp.net".parse::<Jid>().unwrap();

        let base_time = chrono::DateTime::from_timestamp(1700000000, 0).unwrap();

        // Chat Petr: pinned, message at base_time + 100
        state.get_or_create_chat(jid_petr.clone(), "Petr".into(), false);
        state.chats.get_mut(&jid_petr).unwrap().is_pinned = true;
        state.add_message(Message::new_text(
            "m1".into(),
            jid_petr.clone(),
            jid_petr.clone(),
            "Ahoj".into(),
            base_time + chrono::Duration::seconds(100),
            false,
            MessageStatus::Read,
            None,
            None,
        ));

        // Chat AliExpress: unpinned, message at base_time + 200 (newer than Petr, but Petr is pinned!)
        state.get_or_create_chat(jid_aliexpress.clone(), "AliExpress".into(), false);
        state.add_message(Message::new_text(
            "m2".into(),
            jid_aliexpress.clone(),
            jid_aliexpress.clone(),
            "Objednavka".into(),
            base_time + chrono::Duration::seconds(200),
            false,
            MessageStatus::Read,
            None,
            None,
        ));

        // Chat Vojta: unpinned, message at base_time + 50 (older than AliExpress)
        state.get_or_create_chat(jid_vojta.clone(), "Vojta".into(), false);
        state.add_message(Message::new_text(
            "m3".into(),
            jid_vojta.clone(),
            jid_vojta.clone(),
            "Cau".into(),
            base_time + chrono::Duration::seconds(50),
            false,
            MessageStatus::Read,
            None,
            None,
        ));

        // Chat Empty: unpinned, no messages (should be at the very bottom)
        state.get_or_create_chat(jid_empty.clone(), "Empty Contact".into(), false);

        state.consolidate_canonical_chats();
        let chats = state.filtered_chats();

        // 1. Petr (pinned) MUST be first
        assert_eq!(chats[0].name, "Petr");
        // 2. AliExpress (newer message) MUST be second
        assert_eq!(chats[1].name, "AliExpress");
        // 3. Vojta (older message) MUST be third
        assert_eq!(chats[2].name, "Vojta");
        // 4. Empty contact (no messages) MUST be last
        assert_eq!(chats[3].name, "Empty Contact");

        // Now Vojta sends a new message at base_time + 300 -> Vojta must jump above AliExpress!
        state.add_message(Message::new_text(
            "m4".into(),
            jid_vojta.clone(),
            jid_vojta.clone(),
            "Nova zprava".into(),
            base_time + chrono::Duration::seconds(300),
            false,
            MessageStatus::Read,
            None,
            None,
        ));
        state.consolidate_canonical_chats();
        let updated = state.filtered_chats();

        assert_eq!(updated[0].name, "Petr"); // still pinned
        assert_eq!(updated[1].name, "Vojta"); // now newer than AliExpress!
        assert_eq!(updated[2].name, "AliExpress");
        assert_eq!(updated[3].name, "Empty Contact");
    }
    fn text_message(id: &str, jid: &Jid, body: &str, secs: i64) -> Message {
        Message::new_text(
            id.into(),
            jid.clone(),
            jid.clone(),
            body.into(),
            Utc::now() + chrono::Duration::seconds(secs),
            true,
            MessageStatus::Sent,
            None,
            None,
        )
    }

    #[test]
    fn apply_edit_rewrites_body_and_marks_edited() {
        let mut state = AppState::new();
        let jid = Jid::pn("7777777777");
        state.get_or_create_chat(jid.clone(), "Bob".into(), false);
        state.add_message(text_message("m1", &jid, "Helo", 0));

        let applied = state.apply_edit(&jid, "m1", "Hello");

        assert!(applied);
        let msg = &state.messages.get(&jid).unwrap()[0];
        assert_eq!(msg.text, "Hello");
        assert!(msg.edited);
    }

    #[test]
    fn apply_edit_keeps_the_original_timestamp() {
        let mut state = AppState::new();
        let jid = Jid::pn("7777777778");
        state.get_or_create_chat(jid.clone(), "Bob".into(), false);
        state.add_message(text_message("m1", &jid, "before", 0));
        let before = state.messages.get(&jid).unwrap()[0].timestamp;

        state.apply_edit(&jid, "m1", "after");

        assert_eq!(state.messages.get(&jid).unwrap()[0].timestamp, before);
    }

    #[test]
    fn apply_edit_refreshes_the_chat_preview() {
        let mut state = AppState::new();
        let jid = Jid::pn("7777777779");
        state.get_or_create_chat(jid.clone(), "Bob".into(), false);
        state.add_message(text_message("m1", &jid, "typographical error", 0));

        state.apply_edit(&jid, "m1", "fixed text");

        let preview = state.chats.get(&jid).unwrap().last_message.as_ref().unwrap();
        assert_eq!(preview.text, "fixed text");
    }

    #[test]
    fn apply_edit_ignores_unknown_message() {
        let mut state = AppState::new();
        let jid = Jid::pn("7777777780");
        state.get_or_create_chat(jid.clone(), "Bob".into(), false);
        state.add_message(text_message("m1", &jid, "kept", 0));

        assert!(!state.apply_edit(&jid, "does-not-exist", "nope"));
        assert_eq!(state.messages.get(&jid).unwrap()[0].text, "kept");
        assert!(!state.messages.get(&jid).unwrap()[0].edited);
    }

    #[test]
    fn cancel_edit_clears_the_composer_only_in_edit_mode() {
        let mut state = AppState::new();
        state.composer_text = "draft".into();

        // Not editing: cancelling must not throw the draft away.
        state.cancel_edit();
        assert_eq!(state.composer_text, "draft");

        state.editing_message = Some("m1".into());
        state.cancel_edit();
        assert!(state.editing_message.is_none());
        assert!(state.composer_text.is_empty());
    }

    #[test]
    fn legacy_state_without_edited_field_loads_as_unedited() {
        let jid = Jid::pn("7777777781");
        let msg = text_message("m1", &jid, "old", 0);
        let mut value = serde_json::to_value(&msg).unwrap();
        // State persisted before the edit feature shipped has no `edited` key.
        value.as_object_mut().unwrap().remove("edited");

        let restored: Message = serde_json::from_value(value).unwrap();

        assert!(!restored.edited);
        assert_eq!(restored.text, "old");
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    fn jid() -> Jid {
        Jid::pn("420777888999")
    }

    #[test]
    fn toggling_adds_and_removes_the_same_id() {
        let mut state = AppState::new();
        state.toggle_selected("a");
        state.toggle_selected("b");
        assert_eq!(state.selected_messages.len(), 2);

        state.toggle_selected("a");
        assert_eq!(
            state.selected_messages.iter().cloned().collect::<Vec<_>>(),
            vec!["b".to_string()]
        );

        state.toggle_selected("b");
        assert!(state.selected_messages.is_empty(), "unticking the last one must end the mode");
    }

    #[test]
    fn selection_order_is_stable_for_the_forward_payload() {
        let mut state = AppState::new();
        for id in ["c", "a", "b"] {
            state.toggle_selected(id);
        }
        assert_eq!(
            state.selected_messages.iter().cloned().collect::<Vec<_>>(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            "a BTreeSet keeps the forward payload deterministic"
        );
    }

    /// Selection is transient UI state: persisting it would resurrect ticks for
    /// message ids that no longer exist after a restart.
    #[test]
    fn selection_and_forward_dialog_are_not_serialized() {
        let mut state = AppState::new();
        state.toggle_selected("msg-1");
        state.forward_dialog = Some(ForwardDialogState {
            source_chat: jid(),
            message_ids: vec!["msg-1".into()],
            search: "rodina".into(),
        });

        let json = serde_json::to_string(&state).expect("state serializes");
        assert!(!json.contains("selected_messages"), "selection must stay out of the file");
        assert!(!json.contains("forward_dialog"), "an open dialog must not be persisted");
    }

    #[test]
    fn transient_dialog_state_is_not_serialized() {
        let mut state = AppState::default();
        state.new_group = Some(NewGroupState {
            subject: "Rodina".into(),
            search: String::new(),
            selected: Default::default(),
            error: None,
        });
        state.show_contacts_dialog = true;
        state.confirm = Some(ConfirmRequest {
            title: "Smazat chat?".into(),
            message: "Opravdu?".into(),
            confirm_label: "Smazat".into(),
            action: ConfirmAction::DeleteChat { chat_jid: jid() },
        });

        let json = serde_json::to_string(&state).expect("state serializes");
        assert!(!json.contains("new_group"), "an open dialog must not be persisted");
        assert!(!json.contains("show_contacts_dialog"), "panel state must stay transient");
        assert!(!json.contains("confirm"), "a pending confirmation must not be persisted");
    }

    #[test]
    fn group_snapshot_round_trips() {
        let mut state = AppState::default();
        let group = Jid::group("120363000000000000");
        let mut snapshot = GroupInfoSnapshot {
            subject: "Rodina".into(),
            description: Some("Naše skupina".into()),
            ..Default::default()
        };
        snapshot.admins.insert(jid());
        state.group_info.insert(group.to_string(), snapshot);

        let json = serde_json::to_string(&state).expect("state serializes");
        let restored: AppState = serde_json::from_str(&json).expect("state deserializes");
        let info = restored
            .group_info
            .get(&group.to_string())
            .expect("snapshot survives");
        assert_eq!(info.subject, "Rodina");
        assert_eq!(info.description.as_deref(), Some("Naše skupina"));
        assert!(info.is_admin(&jid()), "admin flags survive the round trip");
    }
}
