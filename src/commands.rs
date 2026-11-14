//! Commands sent from UI to backend

use whatsapp_rust::wacore_binary::Jid;
use whatsapp_rust::features::PresenceStatus;
use whatsapp_rust::wacore::iq::privacy::{PrivacyCategory, PrivacyValue};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Command {
    // Connection
    Connect,
    Disconnect,
    DismissSyncing,
    CancelPairCode,
    RequestPairCode {
        phone_number: String,
    },
    
    // Messaging
    SendText {
        to: Jid,
        text: String,
        reply_to: Option<String>,
    },
    SendMessage {
        to: Jid,
        message: whatsapp_rust::waproto::whatsapp::Message,
    },
    /// Rewrite the body of an already-sent message (`Client::edit_message`).
    EditText {
        to: Jid,
        message_id: String,
        text: String,
    },
    SendMedia {
        chat_jid: Jid,
        file_path: std::path::PathBuf,
        caption: Option<String>,
        reply_to: Option<String>,
    },
    /// Send a recorded voice note (already encoded to OGG/Opus by the recorder).
    SendVoiceNote {
        chat_jid: Jid,
        file_path: std::path::PathBuf,
        duration_seconds: u32,
        waveform: Vec<u8>,
    },
    /// Relay already-sent messages into another chat (`Client::forward_message`).
    ///
    /// `from_chat` is where the ids live; the backend rebuilds each body from
    /// its stored text or media file, because the original protocol envelope is
    /// not kept around.
    ForwardMessages {
        to: Jid,
        from_chat: Jid,
        message_ids: Vec<String>,
    },
    DownloadMedia {
        chat_jid: Jid,
        message_id: String,
    },
    SendReaction {
        chat_jid: Jid,
        message_id: String,
        emoji: String,
    },
    
    // Calls
    StartCall {
        peer_jid: Jid,
        is_video: bool,
    },
    AcceptCall {
        call_id: String,
    },
    RejectCall {
        call_id: String,
    },
    EndCall {
        call_id: String,
    },
    ToggleCallMute,
    ToggleCallVideo,
    
    // Scheduled messages
    ScheduleMessage {
        chat_jid: Jid,
        text: String,
        scheduled_for: chrono::DateTime<chrono::Utc>,
    },
    CancelScheduledMessage {
        id: String,
    },
    SendScheduledMessageNow {
        id: String,
    },
    
    // Export conversation
    ExportChat {
        chat_jid: Jid,
        format: crate::export::ExportFormat,
        custom_dir: Option<std::path::PathBuf>,
    },
    
    // Receipts
    MarkAsRead {
        chat_jid: Jid,
        message_ids: Vec<String>,
    },
    MarkChatAsRead {
        chat_jid: Jid,
    },
    
    // Chat actions
    ArchiveChat {
        chat_jid: Jid,
    },
    UnarchiveChat {
        chat_jid: Jid,
    },
    PinChat {
        chat_jid: Jid,
    },
    UnpinChat {
        chat_jid: Jid,
    },
    MuteChat {
        chat_jid: Jid,
        until: Option<chrono::DateTime<chrono::Utc>>,
    },
    UnmuteChat {
        chat_jid: Jid,
    },
    DeleteChat {
        chat_jid: Jid,
    },
    ClearChat {
        chat_jid: Jid,
    },
    StarMessage {
        chat_jid: Jid,
        message_id: String,
    },
    UnstarMessage {
        chat_jid: Jid,
        message_id: String,
    },
    DeleteMessageForMe {
        chat_jid: Jid,
        message_id: String,
    },
    RevokeMessage {
        chat_jid: Jid,
        message_id: String,
        is_admin: bool,
        original_sender: Option<Jid>,
    },
    
    // Contact actions
    SaveContact {
        jid: Jid,
        name: String,
    },
    BlockContact {
        jid: Jid,
    },
    UnblockContact {
        jid: Jid,
    },
    
    // Group actions
    CreateGroup {
        subject: String,
        participants: Vec<Jid>,
    },
    JoinGroup {
        invite_code: String,
    },
    LeaveGroup {
        chat_jid: Jid,
    },
    AddParticipants {
        chat_jid: Jid,
        participants: Vec<Jid>,
    },
    RemoveParticipants {
        chat_jid: Jid,
        participants: Vec<Jid>,
    },
    PromoteParticipants {
        chat_jid: Jid,
        participants: Vec<Jid>,
    },
    DemoteParticipants {
        chat_jid: Jid,
        participants: Vec<Jid>,
    },
    GetGroupInviteLink {
        chat_jid: Jid,
    },
    /// Fetch description, admin flags and settings for a group.
    RequestGroupInfo {
        chat_jid: Jid,
    },
    SetGroupSubject {
        chat_jid: Jid,
        subject: String,
    },
    SetGroupDescription {
        chat_jid: Jid,
        description: String,
    },
    SetEphemeral {
        chat_jid: Jid,
        expiration: u32,
    },
    /// Only admins may send messages while this is on.
    SetGroupAnnounce {
        chat_jid: Jid,
        enabled: bool,
    },
    /// Only admins may edit the group subject, description and picture.
    SetGroupLocked {
        chat_jid: Jid,
        enabled: bool,
    },
    /// Create a poll in a chat. `selectable_count` is 1 for single choice.
    /// The message secret the vendor returns is kept so votes can be read.
    CreatePoll {
        chat_jid: Jid,
        question: String,
        options: Vec<String>,
        selectable_count: u32,
    },
    /// Vote on an existing poll. `option_names` are the full selection.
    VotePoll {
        chat_jid: Jid,
        message_id: String,
        poll_creator: Jid,
        message_secret: Vec<u8>,
        option_names: Vec<String>,
    },
    
    // Presence
    SetPresence {
        status: PresenceStatus,
    },
    SubscribePresence {
        jid: Jid,
    },
    UnsubscribePresence {
        jid: Jid,
    },
    
    // Profile
    SetPushName {
        name: String,
    },
    SetStatusText {
        text: String,
    },
    SetProfilePicture {
        data: Vec<u8>,
    },
    
    // Privacy
    SetPrivacy {
        category: PrivacyCategory,
        value: PrivacyValue,
    },
    
    // UI
    SelectChat {
        chat_jid: Option<Jid>,
    },
    SearchChats {
        query: String,
    },
    RefreshChats,
    
    // History sync
    RequestHistorySync {
        chat_jid: Option<Jid>,
        count: u32,
    },
    
    // Shutdown
    Shutdown,
}

#[allow(dead_code)]
impl Command {
    pub fn send_text(to: Jid, text: String) -> Self {
        Self::SendText { to, text, reply_to: None }
    }
    
    pub fn send_text_reply(to: Jid, text: String, reply_to: String) -> Self {
        Self::SendText { to, text, reply_to: Some(reply_to) }
    }

    pub fn edit_text(to: Jid, message_id: String, text: String) -> Self {
        Self::EditText { to, message_id, text }
    }

    pub fn forward_messages(to: Jid, from_chat: Jid, message_ids: Vec<String>) -> Self {
        Self::ForwardMessages { to, from_chat, message_ids }
    }
    
    pub fn mark_as_read(chat_jid: Jid, message_ids: Vec<String>) -> Self {
        Self::MarkAsRead { chat_jid, message_ids }
    }
    
    pub fn mark_chat_read(chat_jid: Jid) -> Self {
        Self::MarkChatAsRead { chat_jid }
    }
    
    pub fn select_chat(chat_jid: Option<Jid>) -> Self {
        Self::SelectChat { chat_jid }
    }
    
    pub fn search(query: String) -> Self {
        Self::SearchChats { query }
    }
}