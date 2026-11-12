//! Storage layer - persists UI state to disk so chats and messages survive restart

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use whatsapp_rust::wacore_binary::Jid;
use crate::state::{AppState, Chat, Contact, Message, SharedState};

#[derive(Serialize, Deserialize)]
struct PersistedAppState {
    chats: Vec<Chat>,
    messages: HashMap<String, Vec<Message>>,
    contacts: Vec<Contact>,
    self_jid: Option<String>,
    self_name: Option<String>,
    lid_to_pn: HashMap<String, String>,
    #[serde(default)]
    scheduled_messages: Vec<crate::state::ScheduledMessage>,
    #[serde(default)]
    preferences: crate::state::UserPreferences,
}

fn get_storage_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("warust")
        .join("ui_state.json")
}

pub async fn persist_state(state: &SharedState) -> Result<()> {
    persist_state_inner(state, false).await
}

/// Persist only when content actually changed since the last successful write.
pub async fn persist_if_dirty(state: &SharedState) -> Result<()> {
    persist_state_inner(state, true).await
}

async fn persist_state_inner(state: &SharedState, only_if_dirty: bool) -> Result<()> {
    let path = get_storage_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let persisted = {
        let mut s = state.write();
        if only_if_dirty && !s.dirty {
            return Ok(());
        }
        // Clear the flag while still holding the lock, so any mutation that
        // happens after this snapshot immediately re-marks the state dirty
        // instead of being lost by the write below.
        s.dirty = false;
        PersistedAppState {
            chats: s.chats.values().cloned().collect(),
            messages: s.messages.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            contacts: s.contacts.values().cloned().collect(),
            self_jid: s.self_jid.as_ref().map(|j| j.to_string()),
            self_name: s.self_name.clone(),
            lid_to_pn: s.lid_to_pn.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            scheduled_messages: s.scheduled_messages.clone(),
            preferences: s.preferences.clone(),
        }
    };
    let data = serde_json::to_string(&persisted)?;

    // Write to a temp file and rename, so an interrupted write can never
    // truncate the existing history file.
    let tmp = path.with_extension("json.tmp");
    let write_result = async {
        tokio::fs::write(&tmp, data).await?;
        tokio::fs::rename(&tmp, &path).await
    }
    .await;
    if let Err(e) = write_result {
        // The data never reached disk, so try again on the next tick.
        state.write().dirty = true;
        return Err(e.into());
    }
    debug!("UI state persisted to {:?}", path);
    Ok(())
}

pub async fn load_state() -> Result<AppState> {
    let path = get_storage_path();
    if path.exists() {
        let data = tokio::fs::read_to_string(&path).await?;
        match serde_json::from_str::<PersistedAppState>(&data) {
            Ok(persisted) => {
                let mut state = AppState::new();
                for (lid_str, pn_str) in persisted.lid_to_pn {
                    if let (Ok(lid_jid), Ok(pn_jid)) = (lid_str.parse::<Jid>(), pn_str.parse::<Jid>()) {
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
                for mut chat in persisted.chats {
                    if let Some(name) = state.resolve_display_name(&chat.jid) {
                        chat.name = name;
                    }
                    state.chats.insert(chat.jid.clone(), chat);
                }
                // Ensure all contacts with verified/human names have a corresponding chat entry
                let named_contacts: Vec<(Jid, String)> = state.contacts.iter()
                    .filter_map(|(jid, contact)| {
                        let human_name = contact.name.as_ref().filter(|n| crate::util::is_real_name(n))
                            .or_else(|| contact.verified_name.as_ref().filter(|n| crate::util::is_real_name(n)));
                        human_name.map(|name| (state.canonical_jid(jid), name.clone()))
                    })
                    .collect();
                for (canon, name) in named_contacts {
                    if !state.chats.contains_key(&canon) {
                        state.get_or_create_chat(canon, name, false);
                    }
                }
                for (jid_str, mut msgs) in persisted.messages {
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
                                if let Some(target) = msgs.iter_mut().rev().find(|m| m.media_type.is_some() || !m.text.is_empty()) {
                                    target.reactions.retain(|r| r.sender != sender);
                                    target.reactions.push(crate::state::Reaction {
                                        emoji,
                                        sender,
                                    });
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
                state.preferences = persisted.preferences;
                state.clean_stray_reactions();
                state.consolidate_canonical_chats();
                info!("Loaded and consolidated UI state with {} chats from {:?}", state.chats.len(), path);
                return Ok(state);
            }
            Err(e) => {
                // Never silently start empty: the 5s persistence loop would then
                // overwrite the user's history with an empty file.
                let backup = path.with_extension("json.corrupt");
                let _ = std::fs::rename(&path, &backup);
                tracing::error!(
                    "Failed to parse {}: {}. Moved it to {:?} and starting with empty state.",
                    path.display(), e, backup
                );
            }
        }
    }
    Ok(AppState::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_persist_and_load_state() {
        let state = std::sync::Arc::new(parking_lot::RwLock::new(AppState::new()));
        let jid: Jid = "420123456789@s.whatsapp.net".parse().unwrap();
        {
            let mut s = state.write();
            s.get_or_create_chat(jid.clone(), "+420 123 456 789".to_string(), false);
            s.self_jid = Some(jid.clone());
            s.self_name = Some("Tester".to_string());
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let test_path = temp_dir.path().join("test_ui_state.json");
        
        // Serialize manually with PersistedAppState
        let persisted = {
            let s = state.read();
            PersistedAppState {
                chats: s.chats.values().cloned().collect(),
                messages: s.messages.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
                contacts: s.contacts.values().cloned().collect(),
                self_jid: s.self_jid.as_ref().map(|j| j.to_string()),
                self_name: s.self_name.clone(),
                lid_to_pn: s.lid_to_pn.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
                scheduled_messages: s.scheduled_messages.clone(),
                preferences: s.preferences.clone(),
            }
        };
        let json = serde_json::to_string_pretty(&persisted).expect("Serialization must succeed");
        std::fs::write(&test_path, json).unwrap();

        let loaded: PersistedAppState = serde_json::from_str(&std::fs::read_to_string(&test_path).unwrap()).unwrap();
        assert_eq!(loaded.chats.len(), 1);
        assert_eq!(loaded.self_name.as_deref(), Some("Tester"));
    }
}
