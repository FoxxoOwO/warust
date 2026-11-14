//! Storage layer - persists UI state to disk so chats and messages survive restart.
//!
//! The state is split in two so that a busy chat does not force a rewrite of
//! everything else:
//!
//! * `ui_state.json` - chats, contacts, preferences, identity. Small.
//! * `messages/<hash>.json` - one file per conversation.
//!
//! Each file is only rewritten when its serialized bytes actually changed
//! (tracked by hash), so a new message in one chat no longer rewrites the
//! megabytes of history belonging to the others.

use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use whatsapp_rust::wacore_binary::Jid;

use crate::state::{AppState, Chat, Contact, Message, SharedState};

#[derive(Serialize, Deserialize)]
struct PersistedAppState {
    chats: Vec<Chat>,
    /// Legacy inline history. Only written by versions that predate the split;
    /// kept so an upgrade can migrate in place without losing messages.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    messages: HashMap<String, Vec<Message>>,
    contacts: Vec<Contact>,
    self_jid: Option<String>,
    self_name: Option<String>,
    lid_to_pn: HashMap<String, String>,
    #[serde(default)]
    scheduled_messages: Vec<crate::state::ScheduledMessage>,
    #[serde(default)]
    preferences: crate::state::UserPreferences,
    #[serde(default)]
    initial_history_fetched: bool,
    /// Chat jid (as a string, because JSON keys must be strings) to its pin.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pinned: HashMap<String, crate::state::PinnedMessage>,
}

/// Contents of one `messages/<hash>.json` file.
#[derive(Serialize, Deserialize)]
struct PersistedMessages {
    jid: String,
    #[serde(default)]
    messages: Vec<Message>,
}

/// Hash of the bytes last written per path. Lets a persist pass skip files whose
/// content did not change, which is what keeps writes proportional to activity
/// instead of to total history size.
static WRITTEN: Mutex<Option<HashMap<PathBuf, u64>>> = Mutex::new(None);

/// FNV-1a: enough to detect changes, and avoids pulling in a hashing crate.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn remembered_hash(path: &Path) -> Option<u64> {
    WRITTEN.lock().as_ref().and_then(|m| m.get(path).copied())
}

fn remember_hash(path: &Path, hash: u64) {
    let mut guard = WRITTEN.lock();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(path.to_path_buf(), hash);
}

/// `desktop_notifications` shipped as a dead toggle defaulting to `false`, so a
/// persisted `false` is not a real user choice. Flip it on exactly once; after
/// the marker is stored, later loads respect whatever the user picked.
/// Returns `true` when the value changed.
fn apply_notification_default(preferences: &mut crate::state::UserPreferences) -> bool {
    if preferences.notifications_default_applied {
        return false;
    }
    preferences.notifications_default_applied = true;
    if preferences.desktop_notifications {
        return false;
    }
    preferences.desktop_notifications = true;
    true
}

/// A state directory. Production uses [`Store::default_location`]; tests point
/// it at a temporary directory.
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn default_location() -> Self {
        let dir = dirs::data_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .join("warust");
        Self { dir }
    }

    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn state_path(&self) -> PathBuf {
        self.dir.join("ui_state.json")
    }

    fn messages_dir(&self) -> PathBuf {
        self.dir.join("messages")
    }

    fn chat_file_name(jid: &str) -> String {
        format!("{:016x}.json", fnv1a(jid.as_bytes()))
    }

    pub async fn persist(&self, state: &SharedState, only_if_dirty: bool) -> Result<()> {
        // Snapshot under the lock, write outside it: serializing two megabytes
        // while holding the state write lock would freeze the UI.
        let (core, history) = {
            let mut s = state.write();
            if only_if_dirty && !s.dirty {
                return Ok(());
            }
            // Clear the flag while still holding the lock, so any mutation that
            // happens after this snapshot immediately re-marks the state dirty
            // instead of being lost by the writes below.
            s.dirty = false;
            let core = PersistedAppState {
                chats: s.chats.values().cloned().collect(),
                messages: HashMap::new(),
                contacts: s.contacts.values().cloned().collect(),
                self_jid: s.self_jid.as_ref().map(|j| j.to_string()),
                self_name: s.self_name.clone(),
                lid_to_pn: s
                    .lid_to_pn
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                scheduled_messages: s.scheduled_messages.clone(),
                preferences: s.preferences.clone(),
                initial_history_fetched: s.initial_history_fetched,
                pinned: s
                    .pinned
                    .iter()
                    .map(|(jid, pin)| (jid.to_string(), pin.clone()))
                    .collect(),
            };
            let mut history: Vec<(String, Vec<Message>)> = s
                .messages
                .iter()
                .map(|(jid, msgs)| (jid.to_string(), msgs.clone()))
                .collect();
            history.sort_by(|a, b| a.0.cmp(&b.0));
            (core, history)
        };

        let result = self.write_all(&core, &history).await;
        if let Err(e) = result {
            // Nothing reached disk for at least one file, so try again next tick.
            state.write().dirty = true;
            return Err(e);
        }
        debug!("UI state persisted to {:?}", self.dir);
        Ok(())
    }

    async fn write_all(
        &self,
        core: &PersistedAppState,
        history: &[(String, Vec<Message>)],
    ) -> Result<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let core_bytes = serde_json::to_vec(core)?;
        write_if_changed(&self.state_path(), &core_bytes).await?;

        let messages_dir = self.messages_dir();
        tokio::fs::create_dir_all(&messages_dir).await?;
        let mut expected: Vec<PathBuf> = Vec::with_capacity(history.len());
        for (jid, messages) in history {
            let path = messages_dir.join(Self::chat_file_name(jid));
            expected.push(path.clone());
            let bytes = serde_json::to_vec(&PersistedMessages {
                jid: jid.clone(),
                messages: messages.clone(),
            })?;
            write_if_changed(&path, &bytes).await?;
        }

        // Drop files for chats that no longer exist (deleted, or merged into a
        // canonical JID by consolidation).
        if let Ok(mut entries) = tokio::fs::read_dir(&messages_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                if !expected.contains(&path) {
                    let _ = tokio::fs::remove_file(&path).await;
                    debug!("Removed orphaned chat file {:?}", path);
                }
            }
        }
        Ok(())
    }

    pub async fn load(&self) -> Result<AppState> {
        let path = self.state_path();
        if !path.exists() {
            return Ok(AppState::new());
        }
        let data = tokio::fs::read_to_string(&path).await?;
        let persisted = match serde_json::from_str::<PersistedAppState>(&data) {
            Ok(persisted) => persisted,
            Err(e) => {
                // Never silently start empty: the persistence loop would then
                // overwrite the user's history with an empty file.
                let backup = path.with_extension("json.corrupt");
                let _ = std::fs::rename(&path, &backup);
                tracing::error!(
                    "Failed to parse {}: {}. Moved it to {:?} and starting with empty state.",
                    path.display(),
                    e,
                    backup
                );
                return Ok(AppState::new());
            }
        };

        let mut state = AppState::new();
        for (lid_str, pn_str) in persisted.lid_to_pn {
            if let (Ok(lid_jid), Ok(pn_jid)) =
                (lid_str.parse::<Jid>(), pn_str.parse::<Jid>())
            {
                state.lid_to_pn.insert(lid_jid, pn_jid);
            }
        }
        for mut contact in persisted.contacts {
            if let Some(ref n) = contact.name {
                if !crate::util::is_real_name(n) {
                    contact.name = None;
                }
            }
            if let Some(ref pn) = contact.push_name {
                if !crate::util::is_real_name(pn) {
                    contact.push_name = None;
                }
            }
            state.contacts.insert(contact.jid.clone(), contact);
        }
        // `desktop_notifications` defaulted to false while the toggle did
        // nothing. Flip it on once so notifications actually work for existing
        // installs, then leave the user's choice alone.
        let mut preferences = persisted.preferences;
        if apply_notification_default(&mut preferences) {
            state.dirty = true;
        }
        state.preferences = preferences;

        for mut chat in persisted.chats {
            if let Some(name) = state.resolve_display_name(&chat.jid) {
                chat.name = name;
            }
            state.chats.insert(chat.jid.clone(), chat);
        }
        // Ensure all contacts with verified/human names have a corresponding chat entry
        let named_contacts: Vec<(Jid, String)> = state
            .contacts
            .iter()
            .filter_map(|(jid, contact)| {
                let human_name = contact
                    .name
                    .as_ref()
                    .filter(|n| crate::util::is_real_name(n))
                    .or_else(|| {
                        contact
                            .verified_name
                            .as_ref()
                            .filter(|n| crate::util::is_real_name(n))
                    });
                human_name.map(|name| (state.canonical_jid(jid), name.clone()))
            })
            .collect();
        for (canon, name) in named_contacts {
            if !state.chats.contains_key(&canon) {
                state.get_or_create_chat(canon, name, false);
            }
        }

        // Inline history means this file predates the split, so migrate it once.
        let (mut loaded, legacy_inline) = if persisted.messages.is_empty() {
            (self.load_message_files().await, false)
        } else {
            (persisted.messages.into_iter().collect::<Vec<_>>(), true)
        };
        loaded.sort_by(|a, b| a.0.cmp(&b.0));

        for (jid_str, mut msgs) in loaded {
            if let Ok(jid) = jid_str.parse::<Jid>() {
                // Migrate any stray standalone emoji reaction messages into proper message reactions
                let mut reactions_to_attach = Vec::new();
                msgs.retain(|m| {
                    if crate::state::is_reaction_only(&m.text) && m.media_type.is_none() {
                        reactions_to_attach.push((m.sender_jid.clone(), m.text.clone()));
                        false
                    } else {
                        true
                    }
                });
                if !reactions_to_attach.is_empty() {
                    for (sender, emoji) in reactions_to_attach {
                        if let Some(target) = msgs
                            .iter_mut()
                            .rev()
                            .find(|m| m.media_type.is_some() || !m.text.is_empty())
                        {
                            target.reactions.retain(|r| r.sender != sender);
                            target.reactions.push(crate::state::Reaction { emoji, sender });
                        }
                    }
                }
                state.messages.insert(jid, msgs);
            }
        }

        if let Some(sj) = persisted.self_jid {
            state.self_jid = sj.parse::<Jid>().ok();
        }
        state.self_name = persisted.self_name;
        state.scheduled_messages = persisted.scheduled_messages;
        state.initial_history_fetched = persisted.initial_history_fetched;
        state.pinned = persisted
            .pinned
            .into_iter()
            .filter_map(|(jid, pin)| jid.parse::<Jid>().ok().map(|jid| (jid, pin)))
            .collect();
        state.clean_stray_reactions();
        state.consolidate_canonical_chats();

        if legacy_inline {
            // Force one rewrite so the history lands in `messages/` and the
            // legacy map disappears from `ui_state.json`.
            state.dirty = true;
            info!(
                "Migrated inline message history from {:?} into {:?}",
                path,
                self.messages_dir()
            );
        }

        info!(
            "Loaded and consolidated UI state with {} chats from {:?}",
            state.chats.len(),
            path
        );
        Ok(state)
    }

    async fn load_message_files(&self) -> Vec<(String, Vec<Message>)> {
        let dir = self.messages_dir();
        let mut out = Vec::new();
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(_) => return out,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let bytes = match tokio::fs::read(&path).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    warn!("Cannot read chat file {:?}: {}", path, e);
                    continue;
                }
            };
            match serde_json::from_slice::<PersistedMessages>(&bytes) {
                Ok(file) => out.push((file.jid, file.messages)),
                Err(e) => {
                    // Quarantine only this conversation; the rest stays usable.
                    let backup = path.with_extension("json.corrupt");
                    let _ = std::fs::rename(&path, &backup);
                    tracing::error!(
                        "Failed to parse chat file {:?}: {}. Moved it to {:?}.",
                        path,
                        e,
                        backup
                    );
                }
            }
        }
        out
    }

}

/// Write `bytes` to `path` unless the identical content is already there.
/// Returns `true` when the file was written.
async fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool> {
    let hash = fnv1a(bytes);
    if remembered_hash(path) == Some(hash) {
        return Ok(false);
    }
    // Write to a temp file and rename, so an interrupted write can never
    // truncate an existing file.
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path).await?;
    remember_hash(path, hash);
    Ok(true)
}

pub async fn persist_state(state: &SharedState) -> Result<()> {
    Store::default_location().persist(state, false).await
}

/// Persist only when content actually changed since the last successful write.
pub async fn persist_if_dirty(state: &SharedState) -> Result<()> {
    Store::default_location().persist(state, true).await
}

pub async fn load_state() -> Result<AppState> {
    Store::default_location().load().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MessageStatus, UserPreferences};
    use std::sync::Arc;

    fn shared(state: AppState) -> SharedState {
        Arc::new(parking_lot::RwLock::new(state))
    }

    fn message(jid: &Jid, id: &str, body: &str) -> Message {
        Message::new_text(
            id.to_string(),
            jid.clone(),
            jid.clone(),
            body.to_string(),
            chrono::Utc::now(),
            false,
            MessageStatus::Delivered,
            None,
            None,
        )
    }

    /// The old writer put ten `"media_...":null` keys on every row; those keys
    /// were the bulk of the file, so absent fields must not be serialized.
    #[test]
    fn absent_fields_are_not_serialized() {
        let jid: Jid = "420111222333@s.whatsapp.net".parse().unwrap();
        let json = serde_json::to_string(&message(&jid, "m1", "hi")).unwrap();

        for key in [
            "media_type",
            "media_path",
            "media_key",
            "media_jpeg_thumbnail",
            "reactions",
            "edited",
            "reply_to",
            "ephemeral_expiration",
        ] {
            assert!(!json.contains(key), "{key} should be omitted: {json}");
        }
        assert!(json.contains("\"text\":\"hi\""));
    }

    #[tokio::test]
    async fn identical_content_is_not_rewritten() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("x.json");

        assert!(write_if_changed(&path, b"{\"a\":1}").await.unwrap());
        assert!(!write_if_changed(&path, b"{\"a\":1}").await.unwrap());
        assert!(write_if_changed(&path, b"{\"a\":2}").await.unwrap());
    }

    #[tokio::test]
    async fn persist_splits_history_into_per_chat_files() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path());
        let jid1: Jid = "420111222333@s.whatsapp.net".parse().unwrap();
        let jid2: Jid = "420444555666@s.whatsapp.net".parse().unwrap();

        let state = shared(AppState::new());
        {
            let mut s = state.write();
            s.get_or_create_chat(jid1.clone(), "A".into(), false);
            s.get_or_create_chat(jid2.clone(), "B".into(), false);
            s.add_message(message(&jid1, "m1", "ahoj"));
            s.add_message(message(&jid2, "m2", "nazdar"));
        }
        store.persist(&state, false).await.unwrap();

        let core = std::fs::read_to_string(temp.path().join("ui_state.json")).unwrap();
        assert!(!core.contains("\"messages\""), "core must not inline history");

        let files: Vec<_> = std::fs::read_dir(temp.path().join("messages"))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 2);
    }

    #[tokio::test]
    async fn adding_one_message_leaves_other_chat_files_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path());
        let busy: Jid = "420111222333@s.whatsapp.net".parse().unwrap();
        let quiet: Jid = "420444555666@s.whatsapp.net".parse().unwrap();

        let state = shared(AppState::new());
        {
            let mut s = state.write();
            s.get_or_create_chat(busy.clone(), "Busy".into(), false);
            s.get_or_create_chat(quiet.clone(), "Quiet".into(), false);
            s.add_message(message(&quiet, "q1", "quiet history"));
        }
        store.persist(&state, false).await.unwrap();

        let quiet_path = temp
            .path()
            .join("messages")
            .join(Store::chat_file_name(&quiet.to_string()));
        let before = std::fs::read(&quiet_path).unwrap();
        let before_mtime = std::fs::metadata(&quiet_path).unwrap().modified().unwrap();

        {
            let mut s = state.write();
            s.add_message(message(&busy, "b1", "new message"));
        }
        store.persist(&state, true).await.unwrap();

        let after = std::fs::read(&quiet_path).unwrap();
        let after_mtime = std::fs::metadata(&quiet_path).unwrap().modified().unwrap();
        assert_eq!(before, after);
        assert_eq!(before_mtime, after_mtime, "quiet chat must not be rewritten");
    }

    #[tokio::test]
    async fn state_survives_a_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path());
        let jid: Jid = "420999888777@s.whatsapp.net".parse().unwrap();

        let state = shared(AppState::new());
        {
            let mut s = state.write();
            s.get_or_create_chat(jid.clone(), "Tester".into(), false);
            s.add_message(message(&jid, "m1", "uloženo"));
            s.self_jid = Some(jid.clone());
            s.self_name = Some("Tester".into());
        }
        store.persist(&state, false).await.unwrap();

        let loaded = Store::new(temp.path()).load().await.unwrap();
        assert_eq!(loaded.chats.len(), 1);
        let msgs = loaded.messages.get(&jid).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "uloženo");
        assert_eq!(loaded.self_name.as_deref(), Some("Tester"));
    }

    #[tokio::test]
    async fn legacy_inline_history_migrates_to_chat_files() {
        let temp = tempfile::tempdir().unwrap();
        let jid: Jid = "420111000111@s.whatsapp.net".parse().unwrap();

        let legacy = PersistedAppState {
            chats: vec![Chat {
                jid: jid.clone(),
                name: "Legacy".into(),
                last_message: None,
                unread_count: 0,
                is_group: false,
                is_archived: false,
                is_pinned: false,
                mute_until: None,
                last_activity: chrono::Utc::now(),
                is_typing: false,
                typing_until: None,
                participants: Default::default(),
            }],
            messages: HashMap::from([(
                jid.to_string(),
                vec![message(&jid, "old1", "stará zpráva")],
            )]),
            contacts: vec![],
            self_jid: None,
            self_name: None,
            lid_to_pn: HashMap::new(),
            scheduled_messages: vec![],
            preferences: UserPreferences::default(),
            initial_history_fetched: false,
            pinned: HashMap::new(),
        };
        std::fs::create_dir_all(temp.path()).unwrap();
        std::fs::write(
            temp.path().join("ui_state.json"),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .unwrap();

        let store = Store::new(temp.path());
        let loaded = store.load().await.unwrap();
        assert!(loaded.dirty, "migration must schedule a rewrite");
        assert_eq!(loaded.messages.get(&jid).unwrap()[0].text, "stará zpráva");

        store.persist(&shared(loaded), true).await.unwrap();

        let core = std::fs::read_to_string(temp.path().join("ui_state.json")).unwrap();
        // The preview keeps a 20-char snippet by design; the history itself must
        // have moved out of the core file.
        assert!(!core.contains("\"messages\""), "core still inlines history");
        let files: Vec<_> = std::fs::read_dir(temp.path().join("messages"))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1);
        let chat_json = std::fs::read_to_string(files[0].path()).unwrap();
        assert!(chat_json.contains("stará zpráva"));
    }

    #[tokio::test]
    async fn corrupt_chat_file_is_quarantined_not_fatal() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path());
        let good: Jid = "420111222333@s.whatsapp.net".parse().unwrap();
        let bad: Jid = "420444555666@s.whatsapp.net".parse().unwrap();

        let state = shared(AppState::new());
        {
            let mut s = state.write();
            s.get_or_create_chat(good.clone(), "Good".into(), false);
            s.get_or_create_chat(bad.clone(), "Bad".into(), false);
            s.add_message(message(&good, "g1", "přežil"));
            s.add_message(message(&bad, "b1", "poškozeno"));
        }
        store.persist(&state, false).await.unwrap();

        let bad_path = temp
            .path()
            .join("messages")
            .join(Store::chat_file_name(&bad.to_string()));
        std::fs::write(&bad_path, b"{not json").unwrap();

        let loaded = Store::new(temp.path()).load().await.unwrap();
        assert_eq!(loaded.messages.get(&good).unwrap()[0].text, "přežil");
        assert!(loaded.messages.get(&bad).map_or(true, |m| m.is_empty()));
        assert!(
            bad_path.with_extension("json.corrupt").exists(),
            "the damaged file should be kept aside for inspection"
        );
    }

    #[tokio::test]
    async fn deleted_chat_files_are_removed() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path());
        let jid: Jid = "420111222333@s.whatsapp.net".parse().unwrap();

        let state = shared(AppState::new());
        {
            let mut s = state.write();
            s.get_or_create_chat(jid.clone(), "Doomed".into(), false);
            s.add_message(message(&jid, "m1", "bude smazáno"));
        }
        store.persist(&state, false).await.unwrap();
        assert_eq!(
            std::fs::read_dir(temp.path().join("messages")).unwrap().count(),
            1
        );

        {
            let mut s = state.write();
            s.chats.remove(&jid);
            s.messages.remove(&jid);
            s.dirty = true;
        }
        store.persist(&state, true).await.unwrap();
        assert_eq!(
            std::fs::read_dir(temp.path().join("messages")).unwrap().count(),
            0
        );
    }
}
