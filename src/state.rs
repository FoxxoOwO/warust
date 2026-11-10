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
}

impl Chat {
    pub fn display_name(&self) -> &str {
        &self.name
    }

    pub fn is_muted(&self) -> bool {
        self.mute_until.map_or(false, |until| until > Utc::now())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reaction {
    pub emoji: String,
    pub sender: Jid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub chat_jid: Jid,
    pub sender_jid: Jid,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub from_me: bool,
    pub status: MessageStatus,
    pub reply_to: Option<String>,
    pub ephemeral_expiration: Option<u32>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    // Media fields
    pub media_type: Option<MediaType>,
    pub media_path: Option<String>,
    pub media_direct_path: Option<String>,
    pub media_key: Option<Vec<u8>>,
    pub media_file_sha256: Option<Vec<u8>>,
    pub media_file_enc_sha256: Option<Vec<u8>>,
    pub media_file_length: Option<u64>,
    pub media_mimetype: Option<String>,
    pub media_caption: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SettingsTab {
    #[default]
    Profile,
    Chats,
    Notifications,
    Privacy,
    Appearance,
    Storage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub enter_is_send: bool,
    pub sound_notifications: bool,
    pub show_previews: bool,
    pub desktop_notifications: bool,
    pub auto_download_photos: bool,
    pub auto_download_videos: bool,
    pub auto_download_docs: bool,
    pub font_size: f32,
    pub dark_theme_preset: usize,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            enter_is_send: true,
            sound_notifications: true,
            show_previews: true,
            desktop_notifications: false,
            auto_download_photos: true,
            auto_download_videos: false,
            auto_download_docs: false,
            font_size: 14.0,
            dark_theme_preset: 0,
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
    pub pair_phone_number: String,
    pub show_new_chat_dialog: bool,
    pub new_chat_input: String,
    pub new_chat_error: Option<String>,
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
            pair_phone_number: String::new(),
            show_new_chat_dialog: false,
            new_chat_input: String::new(),
            new_chat_error: None,
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
        }
    }
}

pub fn is_reaction_only(text: &str) -> bool {
    matches!(text.trim(), "❤️" | "💀" | "😂" | "👍" | "👎" | "🙏" | "🎉" | "😮" | "😢" | "🔥" | "👏")
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
            last_activity: Utc::now(),
            participants: HashSet::new(),
        })
    }

    pub fn add_message(&mut self, mut msg: Message) {
        let canon_chat_jid = self.canonical_jid(&msg.chat_jid);
        msg.chat_jid = canon_chat_jid.clone();
        let msgs = self.messages.entry(canon_chat_jid.clone()).or_default();
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

        let mut affected_chats = HashSet::new();

        for mut msg in new_msgs {
            let canon_chat_jid = self.canonical_jid(&msg.chat_jid);
            msg.chat_jid = canon_chat_jid.clone();
            affected_chats.insert(canon_chat_jid.clone());
            let msgs = self.messages.entry(canon_chat_jid).or_default();
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

    /// Consolidate chats and messages so that if a LID maps to a PN, they are merged under the canonical PN.
    pub fn consolidate_canonical_chats(&mut self) {
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

        // 5. Update names of all 1-on-1 chats if a human display name is available
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
        
        // Sort: pinned first, then by last activity
        chats.sort_by(|a, b| {
            match (a.is_pinned, b.is_pinned) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.last_activity.cmp(&a.last_activity),
            }
        });
        
        chats
    }

    pub fn apply_reaction(&mut self, chat_jid: &Jid, target_msg_id: &str, sender_jid: &Jid, emoji: &str) -> bool {
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
        false
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
            last_activity: Utc::now(),
            participants: HashSet::new(),
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
}