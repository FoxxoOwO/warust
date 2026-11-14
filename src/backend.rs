//! Backend task - runs whatsapp-rust Bot and handles commands

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use anyhow::Result;
use chrono::Utc;

use tokio::sync::{mpsc, watch};
use tracing::{debug, error, info, warn};
use whatsapp_rust::prelude::*;
use whatsapp_rust::waproto::whatsapp as wa;
use whatsapp_rust::wacore::types::events::Event;
use whatsapp_rust::wacore::types::call::{CallAction, IncomingCall};
use whatsapp_rust::voip::CallHandle;
use whatsapp_rust::wacore_binary::Jid;
use whatsapp_rust::features::{GroupCreateOptions, GroupDescription, GroupSubject, PreviousDescription};
use whatsapp_rust::pair_code::PairCodeOptions;
use whatsapp_rust::Client;
use whatsapp_rust::wacore::proto_helpers::{build_quote_context_with_info, MessageExt};

pub struct CallManager {
    pub incoming_calls: tokio::sync::Mutex<std::collections::HashMap<String, IncomingCall>>,
    pub active_handle: tokio::sync::Mutex<Option<CallHandle>>,
    pub mic_tx: tokio::sync::Mutex<Option<async_channel::Sender<Vec<i16>>>>,
    pub video_tx: tokio::sync::Mutex<Option<async_channel::Sender<Vec<u8>>>>,
}

/// Helper function to find ContextInfo in a message (replaces find_context_info_ref! macro)
fn find_context_info_ref(message: &wa::Message) -> Option<&wa::ContextInfo> {
    // Check all message types that can carry ContextInfo
    if let Some(m) = message.extended_text_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.image_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.video_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.ptv_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.audio_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.document_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.sticker_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.location_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.live_location_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.contact_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.contacts_array_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.buttons_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.buttons_response_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.list_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.list_response_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.template_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.template_button_reply_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.interactive_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.interactive_response_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.poll_creation_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.poll_creation_message_v2.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.poll_creation_message_v3.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.product_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.order_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.group_invite_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.event_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.sticker_pack_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    if let Some(m) = message.newsletter_admin_invite_message.as_option() {
        if let Some(ctx) = m.context_info.as_option() {
            return Some(ctx);
        }
    }
    None
}

use crate::commands::Command;
use crate::state::{ConnectionStatus, Message, MessageStatus, SharedState};

/// Backend handle for communicating with the bot task
pub struct BackendHandle {
    pub tx_command: mpsc::UnboundedSender<Command>,
    pub rx_status: watch::Receiver<ConnectionStatus>,
    pub state: SharedState,
    pub bot_handle: Arc<tokio::sync::Mutex<Option<BotHandle>>>,
}

impl BackendHandle {
    pub fn send_command(&self, cmd: Command) -> Result<()> {
        self.tx_command.send(cmd).map_err(|e| anyhow::anyhow!("Command channel closed: {}", e))
    }
    
    pub fn connection_status(&self) -> ConnectionStatus {
        self.rx_status.borrow().clone()
    }
    
    pub async fn shutdown(&self) -> Result<()> {
        let _ = self.send_command(Command::Shutdown);
        if let Some(handle) = self.bot_handle.lock().await.take() {
            handle.shutdown().await;
        }
        Ok(())
    }
}

static FETCHING_INITIAL_HISTORY: AtomicBool = AtomicBool::new(false);

/// Automatically load the first 20 messages for each conversation.
/// If `force` is false, it only runs on the FIRST login (when `initial_history_fetched` is false).
async fn auto_fetch_initial_messages(client: &Arc<Client>, state: &SharedState, force: bool) {
    if !force && state.read().initial_history_fetched {
        debug!("Initial 20-message history already fetched on first login; skipping.");
        return;
    }

    if FETCHING_INITIAL_HISTORY.swap(true, Ordering::SeqCst) {
        return;
    }
    // Give background tasks and handshake a brief moment to stabilize
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let chats_to_fetch: Vec<Jid> = {
        let s = state.read();
        s.filtered_chats().iter().map(|c| s.canonical_jid(&c.jid)).collect()
    };

    if chats_to_fetch.is_empty() {
        FETCHING_INITIAL_HISTORY.store(false, Ordering::SeqCst);
        return;
    }

    info!("Starting auto-fetch of initial 20 messages for {} conversations (first login: {})", chats_to_fetch.len(), !force);

    for chat_jid in chats_to_fetch {
        let needs_fetch = {
            let s = state.read();
            s.get_chat_messages(&chat_jid).map_or(0, |m| m.len()) < 20
        };

        if needs_fetch {
            debug!("Auto-fetching initial 20 messages for {}", chat_jid);
            if let Err(e) = client.fetch_message_history(&chat_jid, "", false, 0, 20).await {
                debug!("Failed auto-fetch history for {}: {}", chat_jid, e);
            }
            tokio::time::sleep(Duration::from_millis(350)).await;
        }
    }

    {
        let mut s = state.write();
        s.initial_history_fetched = true;
        s.consolidate_canonical_chats();
    }
    let _ = crate::storage::persist_state(state).await;

    FETCHING_INITIAL_HISTORY.store(false, Ordering::SeqCst);
    info!("Completed auto-fetching initial 20 messages for conversations");
}

/// Helper to determine the active SQLite database path for the current session.
fn get_session_db_path(data_dir: &std::path::Path) -> (std::path::PathBuf, String) {
    let session_file = data_dir.join("current_session.txt");
    if let Ok(name) = std::fs::read_to_string(&session_file) {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            let path = data_dir.join(trimmed);
            return (path.clone(), path.to_string_lossy().to_string());
        }
    }
    let default_path = data_dir.join("whatsapp.db");
    (default_path.clone(), default_path.to_string_lossy().to_string())
}

/// Create a Bot instance with SQLite store, wire event handlers, and spawn its supervision loop.
async fn create_and_spawn_bot(
    db_path_str: &str,
    state: &SharedState,
    tx_status: &watch::Sender<ConnectionStatus>,
    call_manager: &Arc<CallManager>,
) -> Result<whatsapp_rust::bot::BotHandle> {
    let store = SqliteStore::new(db_path_str).await?;

    let state_clone = state.clone();
    let tx_status_clone = tx_status.clone();
    let call_manager_clone = call_manager.clone();

    let builder = Bot::builder()
        .with_backend(store)
        .on_qr_code({
            let tx = tx_status_clone.clone();
            let state = state_clone.clone();
            move |code, timeout| {
                info!("QR code generated: {} chars, valid for {}s", code.len(), timeout.as_secs());
                {
                    let mut s = state.write();
                    s.initial_history_fetched = false;
                    s.connection = ConnectionStatus::QrCode(code.to_string());
                }
                let _ = tx.send(ConnectionStatus::QrCode(code.to_string()));
                async {}
            }
        })
        .on_pair_code({
            let tx = tx_status_clone.clone();
            let state = state_clone.clone();
            move |code, timeout| {
                info!("Pair code generated: {}, valid for {}s", code, timeout.as_secs());
                {
                    let mut s = state.write();
                    s.initial_history_fetched = false;
                    s.connection = ConnectionStatus::PairCode(code.to_string());
                }
                let _ = tx.send(ConnectionStatus::PairCode(code.to_string()));
                async {}
            }
        })
        .on_connected({
            let tx = tx_status_clone.clone();
            let state = state_clone.clone();
            move |client| {
                let _ = tx.send(ConnectionStatus::Connected);
                info!("WhatsApp connected!");
                
                // Update self JID
                if let Some(jid) = client.pn().or(client.lid()) {
                    let mut s = state.write();
                    s.self_jid = Some(jid);
                }

                // Trigger background sync for contacts (CriticalUnblockLow, RegularLow, and CriticalBlock)
                let client_clone = client.clone();
                let state_clone2 = state.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                    info!("Triggering AppStateSync for contacts and address book");
                    let _ = client_clone.process_sync_task(whatsapp_rust::sync_task::MajorSyncTask::AppStateSync {
                        name: whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::CriticalUnblockLow,
                        full_sync: true,
                    }).await;
                    let _ = client_clone.process_sync_task(whatsapp_rust::sync_task::MajorSyncTask::AppStateSync {
                        name: whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::RegularLow,
                        full_sync: true,
                    }).await;
                    let _ = client_clone.process_sync_task(whatsapp_rust::sync_task::MajorSyncTask::AppStateSync {
                        name: whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::CriticalBlock,
                        full_sync: true,
                    }).await;

                    let pn_jids: Vec<Jid> = {
                        let s = state_clone2.read();
                        s.lid_to_pn.values().cloned().take(50).collect()
                    };
                    if !pn_jids.is_empty() {
                        if let Ok(user_infos) = client_clone.contacts().get_user_info(&pn_jids).await {
                            let mut s = state_clone2.write();
                            let mut changed = false;
                            for (jid, info) in user_infos {
                                if let Some(vn) = info.verified_name {
                                    if let Some(vname) = vn.name {
                                        if crate::util::is_real_name(&vname) {
                                            let c = s.contacts.entry(jid.clone()).or_insert_with(|| crate::state::Contact::new(jid.clone(), None));
                                            c.verified_name = Some(vname.clone());
                                            if c.name.is_none() {
                                                c.name = Some(vname.clone());
                                                if let Some(chat) = s.chats.get_mut(&jid) {
                                                    chat.name = vname;
                                                }
                                                changed = true;
                                            }
                                        }
                                    }
                                }
                            }
                            if changed {
                                s.consolidate_canonical_chats();
                            }
                        }
                    }
                    crate::avatars::sync_all_avatars(&client_clone, &state_clone2, false).await;

                    info!("Fetching participating groups...");
                    if let Ok(groups) = client_clone.groups().get_participating().await {
                        info!("Found {} participating groups", groups.len());
                        {
                            let mut s = state_clone2.write();
                            for (jid, meta) in groups {
                                let group_name = if !meta.subject.is_empty() {
                                    meta.subject
                                } else {
                                    "Skupinový chat".to_string()
                                };
                                let chat = s.get_or_create_chat(jid, group_name, true);
                                chat.is_group = true;
                            }
                            s.consolidate_canonical_chats();
                        }
                        let _ = crate::storage::persist_state(&state_clone2).await;
                    }

                    // Automatically load the first 20 messages from each conversation on first login
                    auto_fetch_initial_messages(&client_clone, &state_clone2, false).await;
                });

                async {}
            }
        })
        .on_logged_out({
            let tx = tx_status_clone.clone();
            let state = state_clone.clone();
            move |_| {
                let mut s = state.write();
                s.initial_history_fetched = false;
                if s.self_jid.is_some() {
                    s.connection = ConnectionStatus::Disconnected;
                    let _ = tx.send(ConnectionStatus::Disconnected);
                }
                warn!("Logged out from WhatsApp");
                async {}
            }
        })
        .on_message({
            let state = state_clone.clone();
            let tx_status = tx_status_clone.clone();
            move |ctx| {
                let state = state.clone();
                let tx_status = tx_status.clone();
                // Clone needed fields to avoid lifetime issues
                let info = ctx.info.clone();
                let message = (*ctx.message).clone();
                let client = ctx.client.clone();
                async move {
                    handle_incoming_message(info, message, client, state, tx_status).await;
                }
            }
        })
        .on_event({
            let state = state_clone.clone();
            let tx_status = tx_status_clone.clone();
            let call_mgr = call_manager_clone.clone();
            move |event: Arc<Event>, client: Arc<Client>| {
                let state = state.clone();
                let tx_status = tx_status.clone();
                let call_mgr = call_mgr.clone();
                async move {
                    handle_event(event, client, state, tx_status, call_mgr).await;
                }
            }
        });

    state.write().connection = ConnectionStatus::Connecting;
    let _ = tx_status_clone.send(ConnectionStatus::Connecting);
    let bot = builder.build().await?;
    Ok(bot.spawn())
}

/// Spawn the backend task
pub async fn spawn_backend(state: SharedState) -> Result<BackendHandle> {
    let (tx_command, mut rx_command) = mpsc::unbounded_channel::<Command>();
    let (tx_status, rx_status) = watch::channel(ConnectionStatus::Disconnected);
    
    // Determine data directory and session database path
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("warust");
    std::fs::create_dir_all(&data_dir)?;

    let (db_path, db_path_str) = get_session_db_path(&data_dir);

    // Clean up older session database files that aren't the current active one
    let current_file_name = db_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(&data_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if (name.starts_with("whatsapp_") || name == "whatsapp.db") && !name.starts_with(&current_file_name) {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
    }
    
    use whatsapp_rust::wacore::store::traits::{AppSyncStore, DeviceStore, ProtocolStore};

    let store = SqliteStore::new(&db_path_str).await?;

    // Check if we need to force a contact resync:
    let need_contact_resync = {
        let s = state.read();
        s.contacts.is_empty() || s.contacts.values().all(|c| c.name.is_none())
    };

    if need_contact_resync {
        info!("No contact names found in UI state; resetting CriticalUnblockLow and RegularLow in SQLite store to trigger fresh snapshot");
        let _ = store.clear_mutation_macs("critical_unblock_low").await;
        let _ = store.set_version("critical_unblock_low", whatsapp_rust::wacore::appstate::hash::HashState::default()).await;
        let _ = store.clear_mutation_macs("regular_low").await;
        let _ = store.set_version("regular_low", whatsapp_rust::wacore::appstate::hash::HashState::default()).await;
    }

    // Pre-load authenticated user device info
    if let Ok(Some(device)) = store.load().await {
        let mut s = state.write();
        let my_jid = device.pn.clone().or_else(|| device.lid.clone());
        if let Some(jid) = my_jid {
            info!("Found stored device credentials for {}", jid);
            s.self_jid = Some(jid);
        }
        if !device.push_name.is_empty() {
            s.self_name = Some(device.push_name.clone());
        }
    }

    // Pre-load known LID <-> Phone Number mappings from whatsapp.db
    if let Ok(mappings) = store.get_all_lid_mappings().await {
        {
            let mut s = state.write();
            for item in mappings {
                if let (Ok(lid_jid), Ok(pn_jid)) = (
                    format!("{}@lid", item.lid).parse::<Jid>(),
                    format!("{}@s.whatsapp.net", item.phone_number).parse::<Jid>(),
                ) {
                    s.lid_to_pn.insert(lid_jid.clone(), pn_jid.clone());
                    let formatted_phone = crate::util::format_jid(&pn_jid);

                    // Do not store phone numbers as contact human names!
                    // Real names will be learned from ContactUpdate, PushName, or UserInfo
                    s.contacts.entry(lid_jid.clone()).or_insert_with(|| {
                        crate::state::Contact::new(lid_jid.clone(), None)
                    });
                    s.contacts.entry(pn_jid.clone()).or_insert_with(|| {
                        crate::state::Contact::new(pn_jid.clone(), None)
                    });

                    let display_name = s.resolve_display_name(&pn_jid).unwrap_or(formatted_phone);
                    let is_self = s.self_jid.as_ref().map_or(false, |sj| sj.user == item.lid || sj.user == item.phone_number);
                    if !is_self {
                        let _ = s.get_or_create_chat(pn_jid.clone(), display_name, false);
                    }
                }
            }

            // Ensure all contacts with verified/human names have a corresponding chat entry
            let named_contacts: Vec<(Jid, String)> = s.contacts.iter()
                .filter_map(|(jid, c)| {
                    let name = c.name.as_ref().filter(|n| crate::util::is_real_name(n))
                        .or_else(|| c.verified_name.as_ref().filter(|n| crate::util::is_real_name(n)));
                    name.map(|n| (s.canonical_jid(jid), n.clone()))
                })
                .collect();
            for (canon_jid, name) in named_contacts {
                let is_self = s.self_jid.as_ref().map_or(false, |sj| sj.user == canon_jid.user);
                if !is_self {
                    let chat = s.get_or_create_chat(canon_jid, name.clone(), false);
                    chat.name = name;
                }
            }

            s.consolidate_canonical_chats();
        }
        let _ = crate::storage::persist_state(&state).await;
    }

    let call_manager = Arc::new(CallManager {
        incoming_calls: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        active_handle: tokio::sync::Mutex::new(None),
        mic_tx: tokio::sync::Mutex::new(None),
        video_tx: tokio::sync::Mutex::new(None),
    });

    // Log status transitions for debugging
    let mut rx_status_log = tx_status.subscribe();
    tokio::spawn(async move {
        while rx_status_log.changed().await.is_ok() {
            let status = rx_status_log.borrow().clone();
            info!("Backend connection status changed to: {:?}", status);
        }
    });

    let active_db_path = Arc::new(tokio::sync::RwLock::new(db_path_str.clone()));
    let bot_handle = create_and_spawn_bot(&db_path_str, &state, &tx_status, &call_manager).await?;
    let active_client = Arc::new(tokio::sync::RwLock::new(bot_handle.client()));
    let bot_handle_arc = Arc::new(tokio::sync::Mutex::new(Some(bot_handle)));
    
    // Spawn command handler
    let active_client_cmds = active_client.clone();
    let bot_handle_cmds = bot_handle_arc.clone();
    let state_for_cmds = state.clone();
    let tx_status_for_cmds = tx_status.clone();
    let call_mgr_for_cmds = call_manager.clone();
    let active_db_path_cmds = active_db_path.clone();
    let data_dir_cmds = data_dir.clone();
    
    tokio::spawn(async move {
        while let Some(cmd) = rx_command.recv().await {
            if let Err(e) = handle_command(
                cmd,
                &active_client_cmds,
                &bot_handle_cmds,
                &active_db_path_cmds,
                &data_dir_cmds,
                &state_for_cmds,
                &tx_status_for_cmds,
                &call_mgr_for_cmds,
            ).await {
                error!("Command handling error: {}", e);
            }
        }
    });
    
    // Periodic state persistence (skips when nothing changed)
    let state_for_persist = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if let Err(e) = crate::storage::persist_if_dirty(&state_for_persist).await {
                warn!("State persistence failed: {}", e);
            }
        }
    });

    // Periodic scheduled message dispatcher
    let state_for_scheduled = state.clone();
    let active_client_sched = active_client.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let now = chrono::Utc::now();
            let due_items: Vec<(String, Jid, String)> = {
                let mut s = state_for_scheduled.write();
                let mut due = Vec::new();
                for msg in &mut s.scheduled_messages {
                    if msg.status == crate::state::ScheduledStatus::Pending && msg.scheduled_for <= now {
                        msg.status = crate::state::ScheduledStatus::Sent;
                        due.push((msg.id.clone(), msg.chat_jid.clone(), msg.text.clone()));
                    }
                }
                due
            };

            for (id, to, text) in due_items {
                info!("Sending scheduled message {} to {}", id, to);
                let wa_msg = wa::Message::text(text.clone());
                let client_now = active_client_sched.read().await.clone();
                match client_now.send_message(to.clone(), wa_msg).await {
                    Ok(res) => {
                        let sender = client_now.pn().or(client_now.lid()).unwrap_or_else(|| to.clone());
                        let sent_msg = Message::new_text(
                            res.message_id,
                            to.clone(),
                            sender,
                            text,
                            chrono::Utc::now(),
                            true,
                            MessageStatus::Sent,
                            None,
                            None,
                        );
                        let mut s = state_for_scheduled.write();
                        s.add_message(sent_msg);
                    }
                    Err(e) => {
                        error!("Failed to send scheduled message {}: {}", id, e);
                        let mut s = state_for_scheduled.write();
                        if let Some(msg) = s.scheduled_messages.iter_mut().find(|m| m.id == id) {
                            msg.status = crate::state::ScheduledStatus::Failed(e.to_string());
                        }
                    }
                }
            }
        }
    });
    
    Ok(BackendHandle {
        tx_command,
        rx_status,
        state,
        bot_handle: bot_handle_arc,
    })
}

/// Pull a poll's question, options and vote secret out of a creation stanza.
///
/// Single-choice polls travel as `poll_creation_message_v3`, multi-choice as
/// `poll_creation_message`, and older peers as `poll_creation_message_v2`; all
/// three share one proto shape.
fn extract_poll_creation(message: &wa::Message, sender: &Jid) -> Option<crate::state::PollData> {
    let base = message.get_base_message();
    let poll = base
        .poll_creation_message_v3
        .as_option()
        .or_else(|| base.poll_creation_message_v2.as_option())
        .or_else(|| base.poll_creation_message.as_option())?;
    let question = poll.name.clone().unwrap_or_default();
    let options: Vec<String> = poll
        .options
        .iter()
        .filter_map(|opt| opt.option_name.clone())
        .filter(|name| !name.is_empty())
        .collect();
    if question.is_empty() || options.is_empty() {
        return None;
    }
    // Voters derive their encryption keys from the secret WhatsApp embeds in
    // the creation stanza's context info. Without it no vote can be read, so a
    // poll received without one simply shows zero tallies.
    let secret = base
        .message_context_info
        .as_option()
        .and_then(|ctx| ctx.message_secret.clone())
        .unwrap_or_default();
    let is_quiz = poll.poll_type == Some(wa::message::PollType::QUIZ)
        || poll.correct_answer.as_option().is_some();
    Some(crate::state::PollData {
        question,
        options,
        selectable_count: poll.selectable_options_count.unwrap_or(1),
        secret,
        creator: sender.clone(),
        votes: std::collections::HashMap::new(),
        is_quiz,
    })
}

/// Decrypt one inbound vote and fold it into the stored poll's tally.
///
/// Votes are separate messages carrying only ciphertext, so they are never
/// stored as bubbles: the option hashes are mapped back to the option names we
/// keep for the poll and counted from there.
/// Map a requested pin duration onto the three lengths WhatsApp offers.
fn pin_duration(secs: u32) -> whatsapp_rust::send::PinDuration {
    match secs {
        86_400 => whatsapp_rust::send::PinDuration::Hours24,
        2_592_000 => whatsapp_rust::send::PinDuration::Days30,
        _ => whatsapp_rust::send::PinDuration::Days7,
    }
}

async fn apply_poll_vote(
    client: Arc<Client>,
    state: SharedState,
    chat_jid: Jid,
    poll_id: String,
    payload: Vec<u8>,
    iv: Vec<u8>,
    voter: Jid,
    is_group: bool,
) {
    if poll_id.is_empty() {
        return;
    }
    let found = {
        let s = state.read();
        let canon = if is_group { chat_jid.clone() } else { s.canonical_jid(&chat_jid) };
        s.messages
            .get(&canon)
            .and_then(|msgs| msgs.iter().find(|m| m.id == poll_id))
            .and_then(|m| m.poll.as_ref())
            .map(|p| (canon, p.secret.clone(), p.creator.clone(), p.options.clone()))
    };
    let Some((canon, secret, creator, options)) = found else {
        // The creation stanza has not landed (or was pruned): nothing to count.
        debug!("Vote for unknown poll {} in {}", poll_id, chat_jid);
        return;
    };
    if secret.is_empty() {
        debug!("Vote for poll {} dropped: no message secret", poll_id);
        return;
    }
    let ciphertext = whatsapp_rust::wacore::poll::PollVoteCiphertext {
        enc_payload: &payload,
        enc_iv: &iv,
    };
    let hashes = match client
        .polls()
        .decrypt_vote(ciphertext, &secret, &poll_id, &creator, &voter)
        .await
    {
        Ok(hashes) => hashes,
        Err(err) => {
            debug!("Undecryptable vote for poll {} from {}: {}", poll_id, voter, err);
            return;
        }
    };
    let picked: Vec<String> = hashes
        .iter()
        .filter_map(|hash| {
            options
                .iter()
                .find(|name| {
                    whatsapp_rust::wacore::poll::compute_option_hash(name).as_slice() == hash.as_slice()
                })
                .cloned()
        })
        .collect();
    {
        let mut s = state.write();
        let mut touched = false;
        if let Some(msgs) = s.messages.get_mut(&canon) {
            if let Some(msg) = msgs.iter_mut().find(|m| m.id == poll_id) {
                if let Some(poll) = msg.poll.as_mut() {
                    poll.votes.insert(voter.to_string(), picked);
                    touched = true;
                }
            }
        }
        if touched {
            s.dirty = true;
        }
    }
    let _ = crate::storage::persist_state(&state).await;
}

async fn handle_incoming_message(
    info: MessageInfo,
    message: wa::Message,
    client: Arc<Client>,
    state: SharedState,
    _tx_status: watch::Sender<ConnectionStatus>,
) {
    let chat_jid = info.source.chat.clone();
    let sender_jid = info.source.sender.clone();
    let from_me = info.source.is_from_me;
    
    // Check if this is a reaction message
    let base_msg = message.get_base_message();
    let reaction_opt = message.reaction_message.as_option()
        .or_else(|| base_msg.reaction_message.as_option());
    if let Some(reaction) = reaction_opt {
        let target_id = reaction.key.as_option().and_then(|k| k.id.as_deref()).unwrap_or("");
        let emoji = reaction.text.as_deref().unwrap_or("");
        let canon_chat_jid = if info.source.is_group { chat_jid.clone() } else { state.read().canonical_jid(&chat_jid) };
        if !target_id.is_empty() {
            info!("Received reaction '{}' for message {} from {} in chat {}", emoji, target_id, sender_jid, canon_chat_jid);
            {
                let mut s = state.write();
                s.apply_reaction(&canon_chat_jid, target_id, &sender_jid, emoji);
                s.consolidate_canonical_chats();
            }
            let _ = crate::storage::persist_state(&state).await;
        }

        if !from_me {
            let client = client.clone();
            let chat_jid = chat_jid.clone();
            let sender_opt = if info.source.is_group { Some(sender_jid) } else { None };
            let msg_id = info.id.to_string();
            tokio::spawn(async move {
                let _ = client.mark_as_read(&chat_jid, sender_opt.as_ref(), &[&msg_id]).await;
            });
        }
        return;
    }

    // Message edits: WhatsApp delivers the new body inside
    // `protocol_message.edited_message`, keyed by the ORIGINAL message id. Adding
    // it as a fresh message would leave a duplicate bubble, so this path only
    // rewrites the stored message and returns.
    if let Some(pm) = base_msg.protocol_message.as_option() {
        let edited = pm.edited_message.as_option();
        let Some(edited_content) = edited else {
            // Revoke, ephemeral sync and friends: nothing to store as a message.
            return;
        };
        let target_id = pm
            .key
            .as_option()
            .and_then(|k| k.id.as_deref())
            .unwrap_or_default()
            .to_string();
        let new_text = extract_message_text(edited_content).to_string();
        if target_id.is_empty() || new_text.is_empty() {
            debug!("Ignoring malformed edit for chat {}", chat_jid);
            return;
        }
        let canon_chat_jid = if info.source.is_group {
            chat_jid.clone()
        } else {
            state.read().canonical_jid(&chat_jid)
        };
        let applied = {
            let mut s = state.write();
            s.apply_edit(&canon_chat_jid, &target_id, &new_text)
        };
        debug!(
            "Edit for message {} in {} (applied={})",
            target_id, canon_chat_jid, applied
        );
        let _ = crate::storage::persist_state(&state).await;
        return;
    }

    // Poll votes arrive as their own messages. They must never become bubbles:
    // the ciphertext is opened with the poll's secret and merged into its tally.
    if let Some(update) = base_msg.poll_update_message.as_option() {
        let poll_id = update
            .poll_creation_message_key
            .as_option()
            .and_then(|key| key.id.clone())
            .unwrap_or_default();
        let enc = update.vote.as_option();
        let payload = enc.and_then(|v| v.enc_payload.clone());
        let iv = enc.and_then(|v| v.enc_iv.clone());
        if let (Some(payload), Some(iv)) = (payload, iv) {
            apply_poll_vote(
                client.clone(),
                state.clone(),
                chat_jid.clone(),
                poll_id,
                payload,
                iv,
                sender_jid.clone(),
                info.source.is_group,
            )
            .await;
        }
        return;
    }

    // Pin/unpin is a control message too: it rewrites `AppState.pinned` and
    // must not turn into a bubble. WhatsApp pins for everyone, so at most one
    // pin exists per chat and the newest one replaces the previous.
    if let Some(pin) = base_msg.pin_in_chat_message.as_option() {
        let target = pin
            .key
            .as_option()
            .and_then(|key| key.id.clone())
            .unwrap_or_default();
        let canon_chat = if info.source.is_group {
            chat_jid.clone()
        } else {
            state.read().canonical_jid(&chat_jid)
        };
        let unpin = matches!(
            pin.r#type.as_ref(),
            Some(&wa::message::pin_in_chat_message::Type::UnpinForAll)
        );
        {
            let mut s = state.write();
            if unpin {
                s.pinned.remove(&canon_chat);
            } else if !target.is_empty() {
                // Duration travels beside the pin, not inside it.
                let duration = base_msg
                    .message_context_info
                    .as_option()
                    .and_then(|ctx| ctx.message_add_on_duration_in_secs);
                let entry = crate::state::PinnedMessage::new(
                    target,
                    duration,
                    Some(sender_jid.clone()),
                );
                s.pinned.insert(canon_chat.clone(), entry);
            }
            s.dirty = true;
        }
        let _ = crate::storage::persist_state(&state).await;
        return;
    }

    // Extract text content
    let text = extract_message_text(&message).to_string();
    
    // Get message ID
    let msg_id = info.id.to_string();
    let timestamp = info.timestamp;
    
    // Create message
    let mut msg = Message::new_text(
        msg_id.clone(),
        chat_jid.clone(),
        sender_jid.clone(),
        text.clone(),
        timestamp,
        from_me,
        if from_me { MessageStatus::Sent } else { MessageStatus::Delivered },
        extract_reply_to(&message),
        info.ephemeral_expiration,
    );

    if let Some(exp) = info.ephemeral_expiration {
        // Every message sent under a disappearing-message timer carries the
        // expiry, so this keeps the per-chat timer in sync with the server
        // even when the other side changed it.
        let mut s = state.write();
        if exp == 0 {
            s.ephemeral.remove(&chat_jid);
        } else {
            s.ephemeral.insert(chat_jid.clone(), exp);
        }
    }

    if let Some(media) = extract_media_info(&message) {
        let local_path = crate::media::get_local_path(
            &msg_id,
            media.caption.as_deref(),
            media.mimetype.as_deref(),
            media.media_type,
        );
        if local_path.exists() {
            msg.media_path = Some(local_path.to_string_lossy().to_string());
        }
        msg.media_type = Some(media.media_type);
        msg.media_direct_path = media.direct_path;
        msg.media_key = media.media_key;
        msg.media_file_sha256 = media.file_sha256;
        msg.media_file_enc_sha256 = media.file_enc_sha256;
        msg.media_file_length = media.file_length;
        msg.media_mimetype = media.mimetype;
        msg.media_caption = media.caption;
        msg.media_jpeg_thumbnail = media.jpeg_thumbnail;
    }

    if message.is_view_once() {
        msg.view_once = true;
        // Nothing may survive locally before the user asks to open it: a
        // thumbnail on disk (or a leftover copy from an earlier download)
        // would defeat the point of "zobrazit jednou".
        msg.media_jpeg_thumbnail = None;
        msg.media_path = None;
    }
    
    if let Some(poll) = extract_poll_creation(&message, &sender_jid) {
        msg.poll = Some(poll);
    }

    // Update state
    {
        let mut s = state.write();
        let push_name = info.push_name.to_string();
        let has_real_push_name = crate::util::is_real_name(&push_name);

        if !from_me && has_real_push_name {
            let c = s.contacts.entry(sender_jid.clone()).or_insert_with(|| {
                crate::state::Contact::new(sender_jid.clone(), None)
            });
            c.push_name = Some(push_name.clone());

            let mapped_lids: Vec<Jid> = s.lid_to_pn.iter()
                .filter_map(|(lid, pn)| if pn == &sender_jid { Some(lid.clone()) } else { None })
                .collect();
            for lid in mapped_lids {
                let lid_c = s.contacts.entry(lid.clone()).or_insert_with(|| {
                    crate::state::Contact::new(lid.clone(), None)
                });
                lid_c.push_name = Some(push_name.clone());
            }
        }

        // Canonicalize chat JID
        let canon_chat_jid = if info.source.is_group { chat_jid.clone() } else { s.canonical_jid(&chat_jid) };

        // Get or create chat
        let resolved_name = s.resolve_display_name(&canon_chat_jid);
        let chat_name = if info.source.is_group {
            resolved_name.unwrap_or_else(|| "Group Chat".into())
        } else if from_me {
            resolved_name.unwrap_or_else(|| crate::util::format_jid(&canon_chat_jid))
        } else if has_real_push_name {
            push_name.clone()
        } else {
            resolved_name.unwrap_or_else(|| crate::util::format_jid(&canon_chat_jid))
        };
        
        let chat = s.get_or_create_chat(canon_chat_jid.clone(), chat_name, info.source.is_group);
        chat.is_group = info.source.is_group;
        if has_real_push_name && !info.source.is_group {
            chat.name = push_name;
        }
        
        // Add message
        let mut msg_canon = msg;
        msg_canon.chat_jid = canon_chat_jid.clone();
        s.add_message(msg_canon);
        s.consolidate_canonical_chats();

        // Bump unread badge for messages the user has not looked at yet
        if !from_me && s.selected_chat.as_ref() != Some(&canon_chat_jid) {
            if let Some(chat) = s.chats.get_mut(&canon_chat_jid) {
                chat.unread_count = chat.unread_count.saturating_add(1);
            }
        }
    }
    
    // Desktop notification / chime for messages the user is not looking at.
    // Muted chats stay silent; previews honour the privacy preference.
    if !from_me {
        let (title, muted, desktop, sound, show_previews) = {
            let s = state.read();
            let canon = if info.source.is_group {
                chat_jid.clone()
            } else {
                s.canonical_jid(&chat_jid)
            };
            let title = s
                .chats
                .get(&canon)
                .map(|c| c.name.clone())
                .or_else(|| s.resolve_display_name(&canon))
                .unwrap_or_else(|| crate::util::format_jid(&canon));
            let muted = s
                .chats
                .get(&canon)
                .and_then(|c| c.mute_until)
                .is_some_and(|until| until > Utc::now());
            (
                title,
                muted,
                s.preferences.desktop_notifications,
                s.preferences.sound_notifications,
                s.preferences.show_previews,
            )
        };

        if !muted {
            let body = if show_previews {
                crate::notifications::preview(&text)
            } else {
                "Nová zpráva".to_string()
            };
            if desktop {
                crate::notifications::notify(&title, &body, sound);
            } else if sound {
                crate::notifications::play_sound();
            }
        }
    }

    // Auto-send read receipt for non-own messages
    if !from_me {
        let client = client.clone();
        let chat_jid = chat_jid.clone();
        let sender_jid = sender_jid.clone();
        let msg_id = msg_id.clone();
        let is_group = info.source.is_group;
        
        tokio::spawn(async move {
            let sender_opt = if is_group {
                Some(sender_jid)
            } else {
                None
            };
            if let Err(e) = client.mark_as_read(&chat_jid, sender_opt.as_ref(), &[&msg_id]).await {
                debug!("Failed to send read receipt: {}", e);
            }
        });
    }
}

struct ExtractedMedia {
    media_type: crate::state::MediaType,
    direct_path: Option<String>,
    media_key: Option<Vec<u8>>,
    file_sha256: Option<Vec<u8>>,
    file_enc_sha256: Option<Vec<u8>>,
    file_length: Option<u64>,
    mimetype: Option<String>,
    caption: Option<String>,
    jpeg_thumbnail: Option<Vec<u8>>,
}

fn extract_media_info(message: &wa::Message) -> Option<ExtractedMedia> {
    use whatsapp_rust::wacore::proto_helpers::MessageExt;
    let base = message.get_base_message();
    
    if let Some(img) = base.image_message.as_option() {
        return Some(ExtractedMedia {
            media_type: crate::state::MediaType::Image,
            direct_path: img.direct_path.clone(),
            media_key: img.media_key.clone(),
            file_sha256: img.file_sha256.clone(),
            file_enc_sha256: img.file_enc_sha256.clone(),
            file_length: img.file_length,
            mimetype: img.mimetype.clone(),
            caption: img.caption.clone(),
            jpeg_thumbnail: img.jpeg_thumbnail.clone(),
        });
    }
    if let Some(vid) = base.video_message.as_option() {
        return Some(ExtractedMedia {
            media_type: crate::state::MediaType::Video,
            direct_path: vid.direct_path.clone(),
            media_key: vid.media_key.clone(),
            file_sha256: vid.file_sha256.clone(),
            file_enc_sha256: vid.file_enc_sha256.clone(),
            file_length: vid.file_length,
            mimetype: vid.mimetype.clone(),
            caption: vid.caption.clone(),
            jpeg_thumbnail: vid.jpeg_thumbnail.clone(),
        });
    }
    if let Some(audio) = base.audio_message.as_option() {
        let is_ptt = audio.ptt.unwrap_or(false);
        return Some(ExtractedMedia {
            media_type: if is_ptt { crate::state::MediaType::Voice } else { crate::state::MediaType::Audio },
            direct_path: audio.direct_path.clone(),
            media_key: audio.media_key.clone(),
            file_sha256: audio.file_sha256.clone(),
            file_enc_sha256: audio.file_enc_sha256.clone(),
            file_length: audio.file_length,
            mimetype: audio.mimetype.clone(),
            caption: None,
            jpeg_thumbnail: None,
        });
    }
    if let Some(doc) = base.document_message.as_option() {
        let fname = doc.file_name.clone().or_else(|| doc.title.clone());
        return Some(ExtractedMedia {
            media_type: crate::state::MediaType::Document,
            direct_path: doc.direct_path.clone(),
            media_key: doc.media_key.clone(),
            file_sha256: doc.file_sha256.clone(),
            file_enc_sha256: doc.file_enc_sha256.clone(),
            file_length: doc.file_length,
            mimetype: doc.mimetype.clone(),
            caption: fname.or_else(|| doc.caption.clone()),
            jpeg_thumbnail: doc.jpeg_thumbnail.clone(),
        });
    }
    if let Some(stk) = base.sticker_message.as_option() {
        return Some(ExtractedMedia {
            media_type: crate::state::MediaType::Sticker,
            direct_path: stk.direct_path.clone(),
            media_key: stk.media_key.clone(),
            file_sha256: stk.file_sha256.clone(),
            file_enc_sha256: stk.file_enc_sha256.clone(),
            file_length: stk.file_length,
            mimetype: stk.mimetype.clone(),
            caption: None,
            jpeg_thumbnail: None,
        });
    }
    None
}

fn extract_message_text(message: &wa::Message) -> &str {
    let message = message.get_base_message();
    // Check for plain text message (conversation field)
    if let Some(text) = message.conversation.as_deref() {
        return text;
    }
    // Check extended text message
    if let Some(ext) = message.extended_text_message.as_option() {
        if let Some(text) = ext.text.as_deref() {
            return text;
        }
    }
    // Check for image with caption
    if let Some(img) = message.image_message.as_option() {
        if let Some(caption) = img.caption.as_deref() {
            return caption;
        }
        return "📷 Image";
    }
    // Check for video with caption
    if let Some(vid) = message.video_message.as_option() {
        if let Some(caption) = vid.caption.as_deref() {
            return caption;
        }
        return "📹 Video";
    }
    // Check for document with caption
    if let Some(doc) = message.document_message.as_option() {
        if let Some(caption) = doc.caption.as_deref() {
            return caption;
        }
        if let Some(fname) = doc.file_name.as_deref() {
            return fname;
        }
        return "📄 Document";
    }
    // Check for sticker
    if message.sticker_message.is_set() {
        return "🎨 Sticker";
    }
    // Check for audio / voice note
    if let Some(audio) = message.audio_message.as_option() {
        if audio.ptt.unwrap_or(false) {
            return "🎤 Voice Message";
        }
        return "🎵 Audio";
    }
    // Check for contact
    if let Some(contact) = message.contact_message.as_option() {
        if let Some(display_name) = contact.display_name.as_deref() {
            return display_name;
        }
        return "[Contact]";
    }
    // Check for location
    if message.location_message.is_set() {
        return "[Location]";
    }
    // Check for live location
    if message.live_location_message.is_set() {
        return "[Live Location]";
    }
    // Check for reaction
    if message.reaction_message.is_set() {
        return "";
    }
    // Check for poll - the question doubles as the bubble text, the options
    // are drawn from the structured payload.
    for poll in [
        message.poll_creation_message_v3.as_option(),
        message.poll_creation_message_v2.as_option(),
        message.poll_creation_message.as_option(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(name) = poll.name.as_deref() {
            return name;
        }
        return "[Poll]";
    }
    // Check for poll update
    if message.poll_update_message.is_set() {
        return "[Poll Update]";
    }
    // Check for protocol message (delete, edit, etc)
    if let Some(pm) = message.protocol_message.as_option() {
        if let Some(edited) = pm.edited_message.as_option() {
            return extract_message_text(edited);
        }
        return "";
    }
    
    "[Unknown]"
}

fn extract_reply_to(message: &wa::Message) -> Option<String> {
    let base = message.get_base_message();
    // Use find_context_info_ref function to get context_info from supported message types
    let ctx = find_context_info_ref(base);
    
    // ContextInfo has stanza_id (message ID) directly
    ctx.and_then(|ci| ci.stanza_id.as_deref())
        .map(|s| s.to_string())
}

/// Rebuild a sendable envelope for a stored message so it can be forwarded.
///
/// `Client::forward_message` would normally relay the original CDN blob, but we
/// keep only the decrypted local file and the plain text, never the raw
/// protocol envelope — so media is uploaded again from `media_path`.
async fn build_forward_body(client: &Arc<Client>, msg: &Message) -> Option<wa::Message> {
    if let (Some(kind), Some(path)) = (msg.media_type, msg.media_path.as_ref()) {
        let path = std::path::PathBuf::from(path);
        if path.exists() {
            let bytes = tokio::fs::read(&path).await.ok()?;
            let wa_type = match kind {
                crate::state::MediaType::Image | crate::state::MediaType::Sticker => {
                    whatsapp_rust::download::MediaType::Image
                }
                crate::state::MediaType::Video => whatsapp_rust::download::MediaType::Video,
                crate::state::MediaType::Audio | crate::state::MediaType::Voice => {
                    whatsapp_rust::download::MediaType::Audio
                }
                crate::state::MediaType::Document => whatsapp_rust::download::MediaType::Document,
            };
            let upload = client.upload(bytes, wa_type, Default::default()).await.ok()?;
            let file_name = path.file_name().map(|n| n.to_string_lossy().to_string());
            let caption = msg
                .media_caption
                .clone()
                .or_else(|| (!msg.text.is_empty()).then(|| msg.text.clone()));
            return Some(match kind {
                crate::state::MediaType::Image | crate::state::MediaType::Sticker => {
                    whatsapp_rust::media::image_message(
                        upload,
                        whatsapp_rust::media::ImageOptions { caption, ..Default::default() },
                    )
                }
                crate::state::MediaType::Video => whatsapp_rust::media::video_message(
                    upload,
                    whatsapp_rust::media::VideoOptions { caption, ..Default::default() },
                ),
                crate::state::MediaType::Audio | crate::state::MediaType::Voice => {
                    whatsapp_rust::media::audio_message(
                        upload,
                        whatsapp_rust::media::AudioOptions {
                            ptt: Some(kind == crate::state::MediaType::Voice),
                            ..Default::default()
                        },
                    )
                }
                crate::state::MediaType::Document => whatsapp_rust::media::document_message(
                    upload,
                    whatsapp_rust::media::DocumentOptions {
                        file_name,
                        caption,
                        ..Default::default()
                    },
                ),
            });
        }
    }

    if !msg.text.is_empty() {
        return Some(wa::Message::text(msg.text.clone()));
    }
    None
}

fn is_jid_group(jid: &Jid) -> bool {
    // Check if it's a group JID by looking at the server type
    // Group JIDs end with @g.us
    jid.to_string().ends_with("@g.us")
}

async fn handle_event(
    event: Arc<Event>,
    client: Arc<Client>,
    state: SharedState,
    tx_status: watch::Sender<ConnectionStatus>,
    call_manager: Arc<CallManager>,
) {
    match &*event {
        Event::HistorySync(lazy_sync) => {
            let progress = lazy_sync.progress().unwrap_or(0) as u8;
            info!("Received HistorySync chunk: type={}, chunk_order={:?}, progress={}%", lazy_sync.sync_type(), lazy_sync.chunk_order(), progress);
            let _ = tx_status.send(ConnectionStatus::Syncing(progress));
            
            // Process history sync conversations using streaming for lenient decoding
            let mut stream = lazy_sync.stream();
            let mut stream_conversations = Vec::new();
            while let Ok(Some(conv)) = stream.next_conversation() {
                stream_conversations.push(conv);
            }
            if stream.skipped_conversations() > 0 {
                warn!("HistorySync stream skipped {} corrupt conversations", stream.skipped_conversations());
            }

            let remainder = stream.remainder().ok();
            let fallback_history = lazy_sync.get();

            let has_stream = !stream_conversations.is_empty();
            let conversations = if has_stream {
                stream_conversations
            } else if let Some(h) = fallback_history {
                h.conversations.clone()
            } else {
                Vec::new()
            };

            info!("HistorySync decoded: {} conversations found (stream={}, fallback={})",
                conversations.len(),
                has_stream,
                fallback_history.is_some()
            );

            {
                let mut batch_messages = Vec::new();
                let mut new_reactions = Vec::new();
                struct HistoryChatSyncInfo {
                    jid: Jid,
                    name: String,
                    is_group: bool,
                    timestamp: Option<chrono::DateTime<chrono::Utc>>,
                    is_pinned: bool,
                    unread_count: u32,
                    is_archived: bool,
                }
                let mut new_chats: Vec<HistoryChatSyncInfo> = Vec::new();
                let mut new_mappings = Vec::new();
                let mut new_contacts = Vec::new();
                let mut new_pushnames = Vec::new();

                // 1. Phone number to LID mappings
                let mappings_iter = remainder.as_ref().map(|r| &r.phone_number_to_lid_mappings)
                    .or_else(|| fallback_history.map(|h| &h.phone_number_to_lid_mappings));
                if let Some(mappings) = mappings_iter {
                    for mapping in mappings {
                        if let (Some(pn_str), Some(lid_str)) = (&mapping.pn_jid, &mapping.lid_jid) {
                            if let (Ok(pn_jid), Ok(lid_jid)) = (pn_str.parse::<Jid>(), lid_str.parse::<Jid>()) {
                                new_mappings.push((lid_jid, pn_jid));
                            }
                        }
                    }
                }
                
                // 2. Inline contacts
                let contacts_iter = remainder.as_ref().map(|r| &r.inline_contacts)
                    .or_else(|| fallback_history.map(|h| &h.inline_contacts));
                if let Some(contacts) = contacts_iter {
                    for ic in contacts {
                        let name = ic.full_name.as_ref()
                            .or(ic.first_name.as_ref())
                            .or(ic.username.as_ref())
                            .cloned();
                        
                        if let (Some(pn_str), Some(lid_str)) = (&ic.pn_jid, &ic.lid_jid) {
                            if let (Ok(pn_jid), Ok(lid_jid)) = (pn_str.parse::<Jid>(), lid_str.parse::<Jid>()) {
                                new_mappings.push((lid_jid.clone(), pn_jid.clone()));
                                if let Some(ref n) = name {
                                    new_contacts.push((lid_jid, n.clone()));
                                    new_contacts.push((pn_jid, n.clone()));
                                }
                            }
                        } else if let (Some(ref n), Some(lid_str)) = (&name, &ic.lid_jid) {
                            if let Ok(lid_jid) = lid_str.parse::<Jid>() {
                                new_contacts.push((lid_jid, n.clone()));
                            }
                        } else if let (Some(ref n), Some(pn_str)) = (&name, &ic.pn_jid) {
                            if let Ok(pn_jid) = pn_str.parse::<Jid>() {
                                new_contacts.push((pn_jid, n.clone()));
                            }
                        }
                    }
                }
                
                // 3. Pushnames
                let pushnames_iter = remainder.as_ref().map(|r| &r.pushnames)
                    .or_else(|| fallback_history.map(|h| &h.pushnames));
                if let Some(pushnames) = pushnames_iter {
                    for pushname in pushnames {
                        if let (Some(id_str), Some(pn)) = (&pushname.id, &pushname.pushname) {
                            if let Ok(jid) = id_str.parse::<Jid>() {
                                new_pushnames.push((jid, pn.clone()));
                            }
                        }
                    }
                }
                
                // 4. Conversations
                for conv in &conversations {
                    let chat_jid_str = &conv.id;  // required string field
                    let chat_jid = match chat_jid_str.parse::<Jid>() {
                        Ok(j) => j,
                        Err(_) => continue,
                    };
                    
                    if let Some(pn_str) = &conv.pn_jid {
                        if let Ok(pn_jid) = pn_str.parse::<Jid>() {
                            new_mappings.push((chat_jid.clone(), pn_jid));
                        }
                    }
                    
                    let is_group = is_jid_group(&chat_jid);
                    let chat_name = if is_group {
                        conv.name.clone().unwrap_or_else(|| "Group Chat".to_string())
                    } else if let Some(disp) = conv.display_name.as_ref().filter(|s| !s.is_empty()) {
                        disp.clone()
                    } else if let Some(n) = conv.name.as_ref().filter(|s| !s.is_empty()) {
                        n.clone()
                    } else if let Some(user) = conv.username.as_ref().filter(|s| !s.is_empty()) {
                        format!("@{}", user)
                    } else {
                        crate::util::format_jid(&chat_jid)
                    };
                    
                    if !is_group && crate::util::is_real_name(&chat_name) {
                        new_contacts.push((chat_jid.clone(), chat_name.clone()));
                    }

                    let conv_ts = conv.last_msg_timestamp
                        .or(conv.conversation_timestamp)
                        .and_then(|ts| chrono::DateTime::from_timestamp(ts as i64, 0));
                    let is_pinned = conv.pinned.unwrap_or(0) > 0;
                    let unread_count = conv.unread_count.unwrap_or(0);
                    let is_archived = conv.archived.unwrap_or(false);

                    new_chats.push(HistoryChatSyncInfo {
                        jid: chat_jid.clone(),
                        name: chat_name,
                        is_group,
                        timestamp: conv_ts,
                        is_pinned,
                        unread_count,
                        is_archived,
                    });
                    
                    for msg in &conv.messages {
                        // msg.message is MessageField<WebMessageInfo>
                        if let Some(inner_msg) = msg.message.as_option() {
                            // Check if this is a reaction message
                            if let Some(wa_msg) = inner_msg.message.as_option() {
                                let base_msg = wa_msg.get_base_message();
                                let reaction_opt = wa_msg.reaction_message.as_option()
                                    .or_else(|| base_msg.reaction_message.as_option());
                                if let Some(reaction) = reaction_opt {
                                    let target_id = reaction.key.as_option().and_then(|k| k.id.as_deref()).unwrap_or("");
                                    let emoji = reaction.text.as_deref().unwrap_or("");
                                    if !target_id.is_empty() {
                                        let from_me = inner_msg
                                            .key.as_option()
                                            .map(|k| k.from_me.unwrap_or(false))
                                            .unwrap_or(false);
                                        let sender = if from_me {
                                            state.read().self_jid.clone().unwrap_or_else(|| chat_jid.clone())
                                        } else if is_group {
                                            inner_msg.key.as_option()
                                                .and_then(|k| k.participant.as_deref())
                                                .or_else(|| inner_msg.participant.as_deref())
                                                .and_then(|s| s.parse::<Jid>().ok())
                                                .unwrap_or_else(|| chat_jid.clone())
                                        } else {
                                            chat_jid.clone()
                                        };
                                        new_reactions.push((chat_jid.clone(), target_id.to_string(), sender, emoji.to_string()));
                                    }
                                    continue;
                                }
                            }

                            // Get the actual Message from WebMessageInfo
                            let wa_message = inner_msg.message.as_option();
                            if let Some(wa_m) = wa_message.as_ref() {
                                let base_m = wa_m.get_base_message();
                                if let Some(pm) = base_m.protocol_message.as_option() {
                                    if !pm.edited_message.is_set() {
                                        // Ignore protocol message like delete/sync
                                        continue;
                                    }
                                }
                            }
                            if let Some(msg_text) = wa_message.as_ref().map(|m| extract_message_text(m)) {
                                let msg_id = inner_msg
                                    .key.as_option()
                                    .and_then(|k| k.id.as_deref())
                                    .unwrap_or("")
                                    .to_string();
                                let timestamp = chrono::DateTime::from_timestamp(
                                    inner_msg
                                        .message_timestamp
                                        .unwrap_or(0) as i64, 0
                                ).unwrap_or_else(|| Utc::now());
                                
                                let from_me = inner_msg
                                    .key.as_option()
                                    .map(|k| k.from_me.unwrap_or(false))
                                    .unwrap_or(false);
                                let sender = if from_me {
                                    state.read().self_jid.clone().unwrap_or_else(|| chat_jid.clone())
                                } else if is_group {
                                    inner_msg.key.as_option()
                                        .and_then(|k| k.participant.as_deref())
                                        .or_else(|| inner_msg.participant.as_deref())
                                        .and_then(|s| s.parse::<Jid>().ok())
                                        .unwrap_or_else(|| chat_jid.clone())
                                } else {
                                    chat_jid.clone()
                                };
                                
                                let mut message = Message::new_text(
                                    msg_id.clone(),
                                    chat_jid.clone(),
                                    sender,
                                    msg_text.to_string(),
                                    timestamp,
                                    from_me,
                                    if from_me { MessageStatus::Sent } else { MessageStatus::Read },
                                    wa_message
                                        .and_then(|m| {
                                             let base = m.get_base_message();
                                            find_context_info_ref(base)
                                        })
                                        .and_then(|ci| ci.stanza_id.as_deref())
                                        .map(|s| s.to_string()),
                                    wa_message
                                        .and_then(|m| m.protocol_message.as_option())
                                        .and_then(|pm| pm.ephemeral_expiration),
                                );

                                if let Some(media) = wa_message.and_then(|m| extract_media_info(m)) {
                                    let local_path = crate::media::get_local_path(
                                        &msg_id,
                                        media.caption.as_deref(),
                                        media.mimetype.as_deref(),
                                        media.media_type,
                                    );
                                    if local_path.exists() {
                                        message.media_path = Some(local_path.to_string_lossy().to_string());
                                    }
                                    message.media_type = Some(media.media_type);
                                    message.media_direct_path = media.direct_path;
                                    message.media_key = media.media_key;
                                    message.media_file_sha256 = media.file_sha256;
                                    message.media_file_enc_sha256 = media.file_enc_sha256;
                                    message.media_file_length = media.file_length;
                                    message.media_mimetype = media.mimetype;
                                    message.media_caption = media.caption;
                                    message.media_jpeg_thumbnail = media.jpeg_thumbnail;
                                }
                                
                                batch_messages.push(message);
                                if !from_me && !is_group {
                                    if let Some(pn) = inner_msg.push_name.as_ref().filter(|n| crate::util::is_real_name(n)) {
                                        new_pushnames.push((chat_jid.clone(), pn.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Apply everything in a single, fast lock acquisition
                {
                    let mut s = state.write();
                    for (lid, pn) in new_mappings {
                        s.lid_to_pn.insert(lid, pn);
                    }
                    for (jid, name) in new_contacts {
                        let c = s.contacts.entry(jid.clone()).or_insert_with(|| crate::state::Contact::new(jid, Some(name.clone())));
                        c.name = Some(name);
                    }
                    for (jid, pn) in new_pushnames {
                        let c = s.contacts.entry(jid.clone()).or_insert_with(|| crate::state::Contact::new(jid, Some(pn.clone())));
                        c.push_name = Some(pn.clone());
                        if c.name.is_none() {
                            c.name = Some(pn);
                        }
                    }
                    for item in new_chats {
                        let resolved_name = s.resolve_display_name(&item.jid).unwrap_or(item.name);
                        let chat = s.get_or_create_chat(item.jid, resolved_name, item.is_group);
                        if item.is_pinned {
                            chat.is_pinned = true;
                        }
                        if item.is_archived {
                            chat.is_archived = true;
                        }
                        if item.unread_count > 0 {
                            chat.unread_count = item.unread_count;
                        }
                        if let Some(ts) = item.timestamp {
                            if ts > chat.last_activity {
                                chat.last_activity = ts;
                            }
                        }
                    }
                    s.add_messages_batch(batch_messages);
                    for (chat_jid, target_id, sender, emoji) in new_reactions {
                        s.apply_reaction(&chat_jid, &target_id, &sender, &emoji);
                    }

                    // Final pass: update any chat whose name can now be resolved
                    let updates: Vec<(Jid, String)> = s.chats.keys()
                        .filter_map(|jid| s.resolve_display_name(jid).map(|name| (jid.clone(), name)))
                        .collect();
                    for (jid, name) in updates {
                        if let Some(chat) = s.chats.get_mut(&jid) {
                            if !chat.is_group || chat.name == jid.to_string() {
                                chat.name = name;
                            }
                        }
                    }
                    s.consolidate_canonical_chats();
                }
                let _ = crate::storage::persist_state(&state).await;
            }
            
            // Mark connected immediately if 100%, or set up auto-settle debounce
            if progress >= 100 {
                let _ = tx_status.send(ConnectionStatus::Connected);
                let client_clone = client.clone();
                let state_clone = state.clone();
                tokio::spawn(async move {
                    auto_fetch_initial_messages(&client_clone, &state_clone, false).await;
                });
            } else {
                let tx = tx_status.clone();
                let client_clone = client.clone();
                let state_clone = state.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                    let was_syncing = tx.send_if_modified(|status| {
                        if matches!(status, ConnectionStatus::Syncing(_)) {
                            info!("History sync quiescent, setting status to Connected");
                            *status = ConnectionStatus::Connected;
                            true
                        } else {
                            false
                        }
                    });
                    if was_syncing {
                        auto_fetch_initial_messages(&client_clone, &state_clone, false).await;
                    }
                });
            }
        }
        
        Event::Receipt(receipt) => {
            use whatsapp_rust::types::presence::ReceiptType;
            let status = match receipt.r#type {
                ReceiptType::Delivered => MessageStatus::Delivered,
                ReceiptType::Read | ReceiptType::ReadSelf | ReceiptType::Played | ReceiptType::PlayedSelf => MessageStatus::Read,
                ReceiptType::Sent | ReceiptType::Sender => MessageStatus::Sent,
                _ => MessageStatus::Delivered,
            };
            
            let mut s = state.write();
            let crate::state::AppState { chats, messages, .. } = &mut *s;
            for msg_id in &receipt.message_ids {
                for (chat_jid, chat_messages) in messages.iter_mut() {
                    for msg in chat_messages.iter_mut() {
                        if msg.id == *msg_id {
                            msg.status = status;
                            if let Some(chat) = chats.get_mut(chat_jid) {
                                if let Some(ref mut preview) = chat.last_message {
                                    if preview.timestamp == msg.timestamp {
                                        preview.status = status;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Event::ChatPresence(chat_presence) => {
            let mut s = state.write();
            let jid = s.canonical_jid(&chat_presence.source.chat);
            if let Some(c) = s.chats.get_mut(&jid) {
                // Presence snapshots must not clobber locally counted unread state;
                // only an explicit "read from this device" resets it.
                if chat_presence.source.is_from_me {
                    c.unread_count = 0;
                } else {
                    let is_composing = matches!(chat_presence.state, whatsapp_rust::wacore::types::presence::ChatPresence::Composing);
                    c.is_typing = is_composing;
                    if is_composing {
                        c.typing_until = Some(chrono::Utc::now() + chrono::Duration::seconds(5));
                    } else {
                        c.typing_until = None;
                    }
                }
            }
        }
        
        Event::Presence(presence) => {
            let mut s = state.write();
            if let Some(contact) = s.contacts.get_mut(&presence.from) {
                contact.last_seen = presence.last_seen;
                contact.is_online = !presence.unavailable;
            }
        }
        
        Event::Messages(batch) => {
            debug!("Processing message batch with {} messages (origin: {:?})", batch.messages.len(), batch.origin);
            for inbound in batch.messages.iter() {
                handle_incoming_message(
                    (*inbound.info).clone(),
                    (*inbound.message).clone(),
                    client.clone(),
                    state.clone(),
                    tx_status.clone(),
                ).await;
            }
        }
        
        Event::ContactUpdated(contact_update) => {
            let mut s = state.write();
            let jid = contact_update.jid.to_non_ad();
            let jid_clone = jid.clone();
            let _ = s.contacts.entry(jid).or_insert_with(|| crate::state::Contact::new(jid_clone, None));
        }

        Event::ContactUpdate(cu) => {
            let should_persist = {
                let mut s = state.write();
                let name = cu.action.full_name.clone().or_else(|| cu.action.first_name.clone());
                if let Some(name) = name.filter(|n| crate::util::is_real_name(n)) {
                    let canon_jid = cu.jid.to_non_ad();
                    info!("Contact name updated for {}: {}", canon_jid, name);
                    let contact = s.contacts.entry(canon_jid.clone()).or_insert_with(|| {
                        crate::state::Contact::new(canon_jid.clone(), Some(name.clone()))
                    });
                    contact.name = Some(name.clone());
                    
                    if let Some(chat) = s.chats.get_mut(&canon_jid) {
                        chat.name = name.clone();
                    }

                    if let Some(lid_str) = &cu.action.lid_jid {
                        if let Ok(lid_jid) = lid_str.parse::<Jid>() {
                            let lid_non_ad = lid_jid.to_non_ad();
                            if lid_non_ad != canon_jid {
                            s.lid_to_pn.insert(lid_non_ad.clone(), canon_jid.clone());
                            let lid_c = s.contacts.entry(lid_non_ad.clone()).or_insert_with(|| {
                                crate::state::Contact::new(lid_non_ad.clone(), Some(name.clone()))
                            });
                            lid_c.name = Some(name.clone());
                            if let Some(chat) = s.chats.get_mut(&lid_non_ad) {
                                chat.name = name.clone();
                            }
                            }
                        }
                    }

                    // Also check if any LID in lid_to_pn maps to canon_jid
                    let mapped_lids: Vec<Jid> = s.lid_to_pn.iter()
                        .filter_map(|(lid, pn)| if pn == &canon_jid { Some(lid.clone()) } else { None })
                        .collect();
                    for lid in mapped_lids {
                        let lid_c = s.contacts.entry(lid.clone()).or_insert_with(|| {
                            crate::state::Contact::new(lid.clone(), Some(name.clone()))
                        });
                        lid_c.name = Some(name.clone());
                        if let Some(chat) = s.chats.get_mut(&lid) {
                            chat.name = name.clone();
                        }
                    }
                    s.consolidate_canonical_chats();
                    true
                } else {
                    false
                }
            };
            if should_persist {
                let _ = crate::storage::persist_state(&state).await;
            }
        }
        
        Event::PushNameUpdate(pushname) => {
            let should_persist = {
                let mut s = state.write();
                let new_name = pushname.new_push_name.clone();
                let canon_jid = pushname.jid.to_non_ad();
                if crate::util::is_real_name(&new_name) {
                    let contact = s.contacts.entry(canon_jid.clone()).or_insert_with(|| {
                        crate::state::Contact::new(canon_jid.clone(), None)
                    });
                    contact.push_name = Some(new_name.clone());
                    if let Some(chat) = s.chats.get_mut(&canon_jid) {
                        if !chat.is_group {
                            chat.name = new_name.clone();
                        }
                    }

                    let mapped_lids: Vec<Jid> = s.lid_to_pn.iter()
                        .filter_map(|(lid, pn)| if pn == &canon_jid { Some(lid.clone()) } else { None })
                        .collect();
                    for lid in mapped_lids {
                        let lid_c = s.contacts.entry(lid.clone()).or_insert_with(|| {
                            crate::state::Contact::new(lid.clone(), None)
                        });
                        lid_c.push_name = Some(new_name.clone());
                        if let Some(chat) = s.chats.get_mut(&lid) {
                            if !chat.is_group {
                                chat.name = new_name.clone();
                            }
                        }
                    }
                    s.consolidate_canonical_chats();
                    true
                } else {
                    false
                }
            };
            if should_persist {
                let _ = crate::storage::persist_state(&state).await;
            }
        }
        
        Event::IncomingCall(call) => {
            info!("Incoming call: wire_tag={}, call_id={}", call.action.wire_tag(), call.action.call_id());
            let call_id = call.action.call_id().to_string();
            let from_jid = call.from.clone();
            let creator_jid = call.action.call_creator().clone();
            let is_video = matches!(&call.action, CallAction::Offer { is_video: true, .. });
            let is_group = call.group.is_some();

            match &call.action {
                CallAction::Offer { .. } | CallAction::OfferNotice { .. } => {
                    call_manager.incoming_calls.lock().await.insert(call_id.clone(), call.clone());
                    let peer_name = {
                        let s = state.read();
                        let canon = s.canonical_jid(&from_jid);
                        if let Some(contact) = s.contacts.get(&canon) {
                            if let Some(ref n) = contact.name {
                                n.clone()
                            } else {
                                crate::util::format_jid(&from_jid)
                            }
                        } else if let Some(chat) = s.chats.get(&canon) {
                            chat.name.clone()
                        } else {
                            crate::util::format_jid(&from_jid)
                        }
                    };

                    let mut s = state.write();
                    s.active_call = Some(crate::state::ActiveCall {
                        call_id,
                        peer_jid: from_jid,
                        peer_name,
                        call_creator: creator_jid,
                        is_video,
                        is_group,
                        status: crate::state::CallStatus::Incoming,
                        is_muted: false,
                        is_video_enabled: is_video,
                        started_at: chrono::Utc::now(),
                        connected_at: None,
                        ended_at: None,
                    });
                }
                CallAction::Terminate { reason, .. } => {
                    let mut s = state.write();
                    if let Some(ref mut ac) = s.active_call {
                        if ac.call_id == call_id {
                            let reason_str = reason.as_deref().unwrap_or("Hovor ukončen").to_string();
                            ac.status = crate::state::CallStatus::Ended(reason_str);
                            ac.ended_at = Some(chrono::Utc::now());
                        }
                    }
                }
                _ => {}
            }
        }

        Event::MissedCall(missed) => {
            info!("Missed call from {}: {}", missed.from, missed.call_id);
            let canon_chat = state.read().canonical_jid(&missed.from);
            let mut s = state.write();
            if let Some(ref mut ac) = s.active_call {
                if ac.call_id == missed.call_id {
                    ac.status = crate::state::CallStatus::Ended("Zmeškaný hovor".to_string());
                    ac.ended_at = Some(chrono::Utc::now());
                }
            }
            let msg_id = format!("call_missed_{}_{}", missed.call_id, chrono::Utc::now().timestamp());
            let log_msg = crate::state::Message::new_text(
                msg_id,
                canon_chat.clone(),
                missed.from.clone(),
                "📞 Zmeškaný hlasový hovor".to_string(),
                chrono::Utc::now(),
                false,
                crate::state::MessageStatus::Read,
                None,
                None,
            );
            s.add_message(log_msg);
        }

        Event::CallEndedElsewhere(ended) => {
            info!("Call ended elsewhere: {}", ended.call_id);
            let mut s = state.write();
            if let Some(ref ac) = s.active_call {
                if ac.call_id == ended.call_id {
                    s.active_call = None;
                }
            }
        }
        
        Event::PictureUpdate(pic_update) => {
            debug!("Picture update for {}", pic_update.jid);
            let jid = state.read().canonical_jid(&pic_update.jid.to_non_ad());
            let client = client.clone();
            tokio::spawn(async move {
                crate::avatars::fetch_avatar(&client, &jid, true).await;
            });
        }
        
        Event::UserAboutUpdate(about_update) => {
            debug!("About update for {}", about_update.jid);
        }
        
        Event::GroupUpdate(group_update) => {
            debug!("Group update for {}", group_update.group_jid);
        }
        
        Event::ArchiveUpdate(archive_update) => {
            let mut s = state.write();
            if let Some(c) = s.chats.get_mut(&archive_update.jid) {
                // ArchiveChatAction has `archived` field (bool)
                c.is_archived = archive_update.action.archived.unwrap_or(false);
                s.dirty = true;
            }
        }
        
        Event::PinUpdate(pin_update) => {
            let mut s = state.write();
            if let Some(c) = s.chats.get_mut(&pin_update.jid) {
                // PinAction has `pinned` field (bool)
                c.is_pinned = pin_update.action.pinned.unwrap_or(false);
                s.dirty = true;
            }
        }
        
        Event::MuteUpdate(mute_update) => {
            let mut s = state.write();
            if let Some(c) = s.chats.get_mut(&mute_update.jid) {
                // MuteAction has `muteEndTimestamp` field (int64)
                c.mute_until = mute_update.action.mute_end_timestamp.and_then(chrono::DateTime::from_timestamp_millis);
                s.dirty = true;
            }
        }
        
        _ => {}
    }
}

/// Pull a group's description, admin list and settings from the server and
/// mirror them into the chat row (participants, subject). Groups are the only
/// thing the UI cannot derive from message traffic, so every mutation funnels
/// through here instead of patching the snapshot locally and drifting.
async fn refresh_group_info(client: &Client, state: &SharedState, chat_jid: &Jid) {
    match client.groups().get_metadata(chat_jid).await {
        Ok(meta) => {
            let participants: std::collections::HashSet<Jid> =
                meta.participants.iter().map(|p| p.jid.clone()).collect();
            let admins: std::collections::HashSet<Jid> = meta
                .participants
                .iter()
                .filter(|p| p.is_admin())
                .map(|p| p.jid.clone())
                .collect();
            let mut s = state.write();
            let snapshot = s.group_info.entry(chat_jid.to_string()).or_default();
            snapshot.subject = meta.subject.clone();
            snapshot.description = meta.description.clone();
            snapshot.admins = admins;
            snapshot.is_locked = meta.is_locked;
            snapshot.is_announcement = meta.is_announcement;
            if let Some(chat) = s.chats.get_mut(chat_jid) {
                chat.is_group = true;
                chat.participants = participants;
                if !meta.subject.is_empty() {
                    chat.name = meta.subject.clone();
                }
            }
            s.dirty = true;
        }
        Err(e) => warn!("group info for {} failed: {}", chat_jid, e),
    }
}

/// Fetch one media message in the background and remember where it landed.
///
/// Shared by `DownloadMedia` and `OpenViewOnce`: the only difference is who
/// asked for the bytes and what happens afterwards.
fn spawn_media_download(
    client: Arc<Client>,
    state: SharedState,
    canon_chat: Jid,
    message_id: String,
) {
    let msg_clone = {
        let s = state.read();
        s.get_chat_messages(&canon_chat)
            .and_then(|msgs| msgs.iter().find(|m| m.id == message_id))
            .cloned()
    };
    let Some(msg) = msg_clone else {
        return;
    };
    let (Some(direct_path), Some(media_key), Some(file_sha256), Some(media_type)) = (
        msg.media_direct_path,
        msg.media_key,
        msg.media_file_sha256,
        msg.media_type,
    ) else {
        return;
    };

    let wa_type = match media_type {
        crate::state::MediaType::Image => whatsapp_rust::download::MediaType::Image,
        crate::state::MediaType::Video => whatsapp_rust::download::MediaType::Video,
        crate::state::MediaType::Audio | crate::state::MediaType::Voice => {
            whatsapp_rust::download::MediaType::Audio
        }
        crate::state::MediaType::Document => whatsapp_rust::download::MediaType::Document,
        crate::state::MediaType::Sticker => whatsapp_rust::download::MediaType::Sticker,
    };
    let enc_sha = msg.media_file_enc_sha256.as_deref();
    let length = msg.media_file_length.unwrap_or(0);
    let params = whatsapp_rust::download::DownloadParams::encrypted(
        direct_path,
        &media_key,
        &file_sha256,
        enc_sha.unwrap_or(&file_sha256),
        length,
        wa_type,
    );

    let target_path = crate::media::get_local_path(
        &message_id,
        msg.media_caption.as_deref(),
        msg.media_mimetype.as_deref(),
        media_type,
    );

    tokio::spawn(async move {
        tracing::info!(
            "Starting download for media message {} -> {}",
            message_id,
            target_path.display()
        );
        match client.download_from_params(&params).await {
            Ok(bytes) => {
                let _ = std::fs::create_dir_all(crate::media::media_dir());
                if let Err(e) = tokio::fs::write(&target_path, bytes).await {
                    tracing::error!("Failed to save media to {}: {}", target_path.display(), e);
                } else {
                    tracing::info!("Media successfully saved to {}", target_path.display());
                    {
                        let mut s = state.write();
                        if let Some(msgs) = s.messages.get_mut(&canon_chat) {
                            if let Some(m) = msgs.iter_mut().find(|m| m.id == message_id) {
                                m.media_path = Some(target_path.to_string_lossy().to_string());
                            }
                        }
                        s.dirty = true;
                    }
                    crate::media::MEDIA_REV.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let _ = crate::storage::persist_state(&state).await;
                }
            }
            Err(e) => tracing::error!("Failed to download media for {}: {}", message_id, e),
        }
    });
}

async fn handle_command(
    cmd: Command,
    active_client: &Arc<tokio::sync::RwLock<Arc<Client>>>,
    bot_handle_arc: &Arc<tokio::sync::Mutex<Option<whatsapp_rust::bot::BotHandle>>>,
    active_db_path: &Arc<tokio::sync::RwLock<String>>,
    data_dir: &std::path::Path,
    state: &SharedState,
    tx_status: &watch::Sender<ConnectionStatus>,
    call_manager: &Arc<CallManager>,
) -> Result<()> {
    let client = active_client.read().await.clone();
    match cmd {
        Command::DismissSyncing => {
            let _ = tx_status.send(ConnectionStatus::Connected);
        }

        Command::SendText { to, text, reply_to } => {
            let canon_to = state.read().canonical_jid(&to);
            let mut msg = wa::Message::text(text.clone());
            if let Some(ref reply_id) = reply_to {
                if let Some(reply_msg) = state.read().get_chat_messages(&canon_to).and_then(|msgs| msgs.iter().find(|m| m.id == *reply_id)) {
                    let context = build_quote_context_with_info(
                        reply_id,
                        &reply_msg.sender_jid,
                        &canon_to,
                        &canon_to,
                        &wa::Message::default(),
                    );
                    msg = wa::Message::text_with_context(text.clone(), context);
                }
            }
            let res = client.send_message(canon_to.clone(), msg).await?;
            let sender = client.pn().or(client.lid()).unwrap_or_else(|| canon_to.clone());
            let message = Message::new_text(
                res.message_id,
                canon_to.clone(),
                sender,
                text,
                Utc::now(),
                true,
                MessageStatus::Sent,
                reply_to,
                None,
            );
            {
                let mut s = state.write();
                s.add_message(message);
                s.consolidate_canonical_chats();
            }
            let _ = crate::storage::persist_state(state).await;
        }
        
        Command::SendMessage { to, message } => {
            client.send_message(to, message).await?;
        }

        Command::EditText { to, message_id, text } => {
            let edited = client
                .edit_message(to.clone(), message_id.clone(), wa::Message::text(text.clone()))
                .await;
            match edited {
                Ok(_) => {
                    // The guard must die before the await, or the whole command
                    // future stops being `Send` and tokio::spawn rejects it.
                    let applied = {
                        let mut s = state.write();
                        s.apply_edit(&to, &message_id, &text)
                    };
                    if applied {
                        debug!("Edited message {} in {}", message_id, to);
                    } else {
                        debug!("Edited message {} is not in the local store", message_id);
                    }
                    crate::storage::persist_state(state).await?;
                }
                Err(e) => error!("Failed to edit message {}: {}", message_id, e),
            }
        }
        
        Command::SendMedia { chat_jid, file_path, caption, reply_to, view_once } => {
            let canon_to = state.read().canonical_jid(&chat_jid);
            let quote_context = reply_to.as_ref().and_then(|reply_id| {
                let s = state.read();
                let reply_msg = s
                    .get_chat_messages(&canon_to)
                    .and_then(|msgs| msgs.iter().find(|m| m.id == *reply_id))?;
                Some(Box::new(build_quote_context_with_info(
                    reply_id,
                    &reply_msg.sender_jid,
                    &canon_to,
                    &canon_to,
                    &wa::Message::default(),
                )))
            });
            let client_clone = client.clone();
            let state_clone = state.clone();
            
            tokio::spawn(async move {
                match tokio::fs::read(&file_path).await {
                    Ok(bytes) => {
                        let media_type = crate::media::detect_media_type(&file_path);
                        let wa_type = match media_type {
                            crate::state::MediaType::Image => whatsapp_rust::download::MediaType::Image,
                            crate::state::MediaType::Video => whatsapp_rust::download::MediaType::Video,
                            crate::state::MediaType::Audio | crate::state::MediaType::Voice => whatsapp_rust::download::MediaType::Audio,
                            crate::state::MediaType::Document => whatsapp_rust::download::MediaType::Document,
                            crate::state::MediaType::Sticker => whatsapp_rust::download::MediaType::Sticker,
                        };
                        
                        let file_size = bytes.len() as u64;
                        tracing::info!("Uploading media {:?} ({} bytes) to {:?}", media_type, file_size, canon_to);
                        match client_clone.upload(bytes, wa_type, Default::default()).await {
                            Ok(upload) => {
                                let file_name = file_path.file_name().map(|n| n.to_string_lossy().to_string());
                                let wa_msg = match media_type {
                                    crate::state::MediaType::Image => {
                                        whatsapp_rust::media::image_message(upload, whatsapp_rust::media::ImageOptions {
                                            caption: caption.clone(),
                                            context_info: quote_context.clone(),
                                            ..Default::default()
                                        })
                                    }
                                    crate::state::MediaType::Video => {
                                        whatsapp_rust::media::video_message(upload, whatsapp_rust::media::VideoOptions {
                                            caption: caption.clone(),
                                            context_info: quote_context.clone(),
                                            ..Default::default()
                                        })
                                    }
                                    crate::state::MediaType::Audio | crate::state::MediaType::Voice => {
                                        whatsapp_rust::media::audio_message(upload, whatsapp_rust::media::AudioOptions {
                                            ptt: Some(media_type == crate::state::MediaType::Voice),
                                            context_info: quote_context.clone(),
                                            ..Default::default()
                                        })
                                    }
                                    crate::state::MediaType::Document => {
                                        whatsapp_rust::media::document_message(upload, whatsapp_rust::media::DocumentOptions {
                                            file_name: file_name.clone(),
                                            caption: caption.clone(),
                                            context_info: quote_context.clone(),
                                            ..Default::default()
                                        })
                                    }
                                    crate::state::MediaType::Sticker => {
                                        whatsapp_rust::media::image_message(upload, whatsapp_rust::media::ImageOptions {
                                            caption: caption.clone(),
                                            ..Default::default()
                                        })
                                    }
                                };

                                // "Zobrazit jednou": the payload travels inside a
                                // future-proof wrapper, which is what keeps clients
                                // from caching it.
                                let wa_msg = if view_once {
                                    let mut inner = wa_msg;
                                    if let Some(img) = inner.image_message.as_option_mut() {
                                        img.view_once = Some(true);
                                    }
                                    if let Some(vid) = inner.video_message.as_option_mut() {
                                        vid.view_once = Some(true);
                                    }
                                    wa::Message {
                                        view_once_message_v2: whatsapp_rust::waproto::buffa::MessageField::some(
                                            wa::message::FutureProofMessage {
                                                message: whatsapp_rust::waproto::buffa::MessageField::from_box(
                                                    Box::new(inner),
                                                ),
                                                ..Default::default()
                                            },
                                        ),
                                        ..Default::default()
                                    }
                                } else {
                                    wa_msg
                                };

                                match client_clone.send_message(canon_to.clone(), wa_msg).await {
                                    Ok(res) => {
                                        tracing::info!("Media message sent successfully with id {}", res.message_id);
                                        let sender = client_clone.pn().or(client_clone.lid()).unwrap_or_else(|| canon_to.clone());
                                        let display_text = caption.clone().unwrap_or_else(|| match media_type {
                                            crate::state::MediaType::Image => "📷 Photo".into(),
                                            crate::state::MediaType::Video => "📹 Video".into(),
                                            crate::state::MediaType::Audio => "🎵 Audio".into(),
                                            crate::state::MediaType::Voice => "🎤 Voice message".into(),
                                            crate::state::MediaType::Document => file_name.clone().unwrap_or_else(|| "📄 Document".into()),
                                            crate::state::MediaType::Sticker => "🎨 Sticker".into(),
                                        });
                                        let mut local_msg = Message::new_text(
                                            res.message_id,
                                            canon_to.clone(),
                                            sender,
                                            display_text,
                                            chrono::Utc::now(),
                                            true,
                                            MessageStatus::Sent,
                                            reply_to,
                                            None,
                                        );
                                        local_msg.media_type = Some(media_type);
                                        local_msg.media_path = Some(file_path.to_string_lossy().to_string());
                                        local_msg.media_caption = caption;
                                        local_msg.media_file_length = Some(file_size);
                                        {
                                            let mut s = state_clone.write();
                                            s.add_message(local_msg);
                                            s.consolidate_canonical_chats();
                                        }
                                        crate::media::MEDIA_REV.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        let _ = crate::storage::persist_state(&state_clone).await;
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to send media message: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to upload media {}: {}", file_path.display(), e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to read media file {}: {}", file_path.display(), e);
                    }
                }
            });
        }

        Command::ForwardMessages { to, from_chat, message_ids } => {
            let (canon_to, self_jid, msgs) = {
                let s = state.read();
                let canon_to = s.canonical_jid(&to);
                let canon_from = s.canonical_jid(&from_chat);
                let msgs: Vec<Message> = s
                    .get_chat_messages(&canon_from)
                    .map(|all| {
                        all.iter()
                            .filter(|m| message_ids.contains(&m.id))
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                (canon_to, s.self_jid.clone(), msgs)
            };

            if msgs.is_empty() {
                warn!("Forward requested but none of {:?} is still stored", message_ids);
            } else {
                let client_clone = client.clone();
                let state_clone = state.clone();
                tokio::spawn(async move {
                    let sender = self_jid
                        .or_else(|| client_clone.pn())
                        .unwrap_or_else(|| canon_to.clone());
                    let mut delivered = 0usize;
                    for msg in msgs {
                        let Some(body) = build_forward_body(&client_clone, &msg).await else {
                            warn!("Skipping forward of {} — no text and no local media", msg.id);
                            continue;
                        };
                        match client_clone.forward_message(canon_to.clone(), &body).await {
                            Ok(res) => {
                                delivered += 1;
                                let mut copy = msg.clone();
                                copy.id = res.message_id;
                                copy.chat_jid = canon_to.clone();
                                copy.sender_jid = sender.clone();
                                copy.from_me = true;
                                copy.status = MessageStatus::Sent;
                                copy.timestamp = chrono::Utc::now();
                                copy.reply_to = None;
                                copy.edited = false;
                                copy.reactions.clear();
                                state_clone.write().add_message(copy);
                            }
                            Err(e) => error!("Failed to forward {}: {}", msg.id, e),
                        }
                    }
                    tracing::info!("Forwarded {} message(s) to {}", delivered, canon_to);
                    {
                        let mut s = state_clone.write();
                        s.consolidate_canonical_chats();
                    }
                    let _ = crate::storage::persist_state(&state_clone).await;
                });
            }
        }

        Command::DownloadMedia { chat_jid, message_id } => {
            let canon_chat = state.read().canonical_jid(&chat_jid);
            spawn_media_download(client.clone(), state.clone(), canon_chat, message_id);
        }

        Command::OpenViewOnce { chat_jid, message_id } => {
            let canon_chat = state.read().canonical_jid(&chat_jid);
            spawn_media_download(client.clone(), state.clone(), canon_chat, message_id);
        }

        Command::ConsumeViewOnce { chat_jid, message_id } => {
            let canon_chat = state.read().canonical_jid(&chat_jid);
            let stored_path = {
                let s = state.read();
                s.get_chat_messages(&canon_chat)
                    .and_then(|msgs| msgs.iter().find(|m| m.id == message_id))
                    .and_then(|m| m.media_path.clone())
            };
            if let Some(path) = stored_path {
                // The point of a view-once message is that no copy survives.
                let _ = tokio::fs::remove_file(&path).await;
            }
            {
                let mut s = state.write();
                if let Some(msgs) = s.messages.get_mut(&canon_chat) {
                    if let Some(m) = msgs.iter_mut().find(|m| m.id == message_id) {
                        m.media_path = None;
                        m.media_jpeg_thumbnail = None;
                        m.view_once_opened = true;
                    }
                }
                s.dirty = true;
            }
            crate::media::MEDIA_REV.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let _ = crate::storage::persist_state(state).await;
        }

        Command::SendReaction { chat_jid, message_id, emoji } => {
            let canon_chat = {
                let s = state.read();
                s.canonical_jid(&chat_jid)
            };
            let (from_me, sender_jid, is_group) = {
                let s = state.read();
                let is_grp = s.chats.get(&canon_chat).map_or(false, |c| c.is_group);
                if let Some(msgs) = s.get_chat_messages(&canon_chat) {
                    if let Some(msg) = msgs.iter().find(|m| m.id == message_id) {
                        (msg.from_me, msg.sender_jid.clone(), is_grp)
                    } else {
                        (false, canon_chat.clone(), is_grp)
                    }
                } else {
                    (false, canon_chat.clone(), is_grp)
                }
            };
            let self_jid = {
                let s = state.read();
                s.self_jid.clone().unwrap_or_else(|| canon_chat.clone())
            };

            // 1. Update local state immediately for instant feedback
            {
                let mut s = state.write();
                s.apply_reaction(&canon_chat, &message_id, &self_jid, &emoji);
                s.consolidate_canonical_chats();
            }
            let _ = crate::storage::persist_state(&state).await;

            // 2. Dispatch reaction to WhatsApp server
            let client = client.clone();
            let target_key = whatsapp_rust::waproto::whatsapp::MessageKey {
                id: Some(message_id.clone()),
                from_me: Some(from_me),
                remote_jid: Some(canon_chat.to_string()),
                participant: if is_group { Some(sender_jid.to_string()) } else { None },
                ..Default::default()
            };
            tokio::spawn(async move {
                info!("Sending reaction '{}' for message {} in {}", emoji, message_id, canon_chat);
                if let Err(e) = client.send_reaction(&canon_chat, target_key, &emoji).await {
                    warn!("Failed to send reaction to server: {}", e);
                }
            });
        }

        Command::StartCall { peer_jid, is_video } => {
            let canon_peer = state.read().canonical_jid(&peer_jid);
            let peer_name = {
                let s = state.read();
                if let Some(contact) = s.contacts.get(&canon_peer) {
                    if let Some(ref n) = contact.name {
                        n.clone()
                    } else {
                        crate::util::format_jid(&canon_peer)
                    }
                } else if let Some(chat) = s.chats.get(&canon_peer) {
                    chat.name.clone()
                } else {
                    crate::util::format_jid(&canon_peer)
                }
            };

            let self_jid = state.read().self_jid.clone().unwrap_or_else(|| canon_peer.clone());

            {
                let mut s = state.write();
                s.active_call = Some(crate::state::ActiveCall {
                    call_id: String::new(),
                    peer_jid: canon_peer.clone(),
                    peer_name,
                    call_creator: self_jid,
                    is_video,
                    is_group: false,
                    status: crate::state::CallStatus::OutgoingRinging,
                    is_muted: false,
                    is_video_enabled: is_video,
                    started_at: chrono::Utc::now(),
                    connected_at: None,
                    ended_at: None,
                });
            }

            let (mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(20);
            let (speaker_tx, speaker_rx) = async_channel::bounded::<Vec<i16>>(20);

            *call_manager.mic_tx.lock().await = Some(mic_tx.clone());

            let mic_tx_task = mic_tx.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(60));
                loop {
                    interval.tick().await;
                    if mic_tx_task.is_closed() {
                        break;
                    }
                    if mic_tx_task.send(vec![0i16; 960]).await.is_err() {
                        break;
                    }
                }
            });

            tokio::spawn(async move {
                while let Ok(_frame) = speaker_rx.recv().await {}
            });

            let (v_src_rx, v_sink_tx) = if is_video {
                let (v_src_tx, v_src_rx) = async_channel::bounded::<Vec<u8>>(15);
                let (v_sink_tx, v_sink_rx) = async_channel::bounded::<whatsapp_rust::voip::VideoFrame>(15);
                *call_manager.video_tx.lock().await = Some(v_src_tx);
                tokio::spawn(async move {
                    while let Ok(_vf) = v_sink_rx.recv().await {}
                });
                (Some(v_src_rx), Some(v_sink_tx))
            } else {
                (None, None)
            };

            let client_clone = client.clone();
            let state_clone = state.clone();
            let call_mgr = call_manager.clone();
            tokio::spawn(async move {
                info!("Starting outgoing call to {} (video={})", canon_peer, is_video);
                let voip = client_clone.voip();
                let mut builder = voip.call(&canon_peer).audio(mic_rx, speaker_tx);
                if let (Some(v_rx), Some(v_tx)) = (v_src_rx, v_sink_tx) {
                    builder = builder.video(v_rx, v_tx);
                }
                match builder.start().await {
                    Ok(handle) => {
                        let call_id = handle.call_id().to_string();
                        info!("Outgoing call started successfully: call_id={}", call_id);
                        {
                            let mut s = state_clone.write();
                            if let Some(ref mut ac) = s.active_call {
                                ac.call_id = call_id.clone();
                                ac.status = crate::state::CallStatus::Connected;
                                ac.connected_at = Some(chrono::Utc::now());
                            }
                        }
                        let handle_clone = handle.clone();
                        *call_mgr.active_handle.lock().await = Some(handle);
                        
                        handle_clone.wait_ended().await;
                        info!("Call ended on wire: call_id={}", call_id);
                        let mut s = state_clone.write();
                        if let Some(ref mut ac) = s.active_call {
                            if ac.call_id == call_id {
                                ac.status = crate::state::CallStatus::Ended("Hovor ukončen".to_string());
                                ac.ended_at = Some(chrono::Utc::now());
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Outgoing call failed to start: {}", e);
                        let mut s = state_clone.write();
                        if let Some(ref mut ac) = s.active_call {
                            ac.status = crate::state::CallStatus::Ended(format!("Hovor se nezdařil: {}", e));
                            ac.ended_at = Some(chrono::Utc::now());
                        }
                    }
                }
            });
        }

        Command::AcceptCall { call_id } => {
            let incoming_opt = call_manager.incoming_calls.lock().await.remove(&call_id);
            if let Some(incoming) = incoming_opt {
                let (mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(20);
                let (speaker_tx, speaker_rx) = async_channel::bounded::<Vec<i16>>(20);

                *call_manager.mic_tx.lock().await = Some(mic_tx.clone());

                let mic_tx_task = mic_tx.clone();
                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(std::time::Duration::from_millis(60));
                    loop {
                        interval.tick().await;
                        if mic_tx_task.is_closed() { break; }
                        if mic_tx_task.send(vec![0i16; 960]).await.is_err() { break; }
                    }
                });

                tokio::spawn(async move {
                    while let Ok(_frame) = speaker_rx.recv().await {}
                });

                let is_video = matches!(&incoming.action, CallAction::Offer { is_video: true, .. });
                let (v_src_rx, v_sink_tx) = if is_video {
                    let (v_src_tx, v_src_rx) = async_channel::bounded::<Vec<u8>>(15);
                    let (v_sink_tx, v_sink_rx) = async_channel::bounded::<whatsapp_rust::voip::VideoFrame>(15);
                    *call_manager.video_tx.lock().await = Some(v_src_tx);
                    tokio::spawn(async move {
                        while let Ok(_vf) = v_sink_rx.recv().await {}
                    });
                    (Some(v_src_rx), Some(v_sink_tx))
                } else {
                    (None, None)
                };

                let client_clone = client.clone();
                let state_clone = state.clone();
                let call_mgr = call_manager.clone();
                tokio::spawn(async move {
                    info!("Accepting incoming call: {} (video={})", call_id, is_video);
                    let voip = client_clone.voip();
                    let mut builder = voip.accept(&incoming).audio(mic_rx, speaker_tx);
                    if let (Some(v_rx), Some(v_tx)) = (v_src_rx, v_sink_tx) {
                        builder = builder.video(v_rx, v_tx);
                    }
                    match builder.start().await {
                        Ok(handle) => {
                            info!("Call accepted and attached: call_id={}", handle.call_id());
                            {
                                let mut s = state_clone.write();
                                if let Some(ref mut ac) = s.active_call {
                                    ac.status = crate::state::CallStatus::Connected;
                                    ac.connected_at = Some(chrono::Utc::now());
                                }
                            }
                            let handle_clone = handle.clone();
                            *call_mgr.active_handle.lock().await = Some(handle);

                            handle_clone.wait_ended().await;
                            info!("Incoming call finished on wire: {}", call_id);
                            let mut s = state_clone.write();
                            if let Some(ref mut ac) = s.active_call {
                                if ac.call_id == call_id {
                                    ac.status = crate::state::CallStatus::Ended("Hovor ukončen".to_string());
                                    ac.ended_at = Some(chrono::Utc::now());
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to accept call: {}", e);
                            let mut s = state_clone.write();
                            if let Some(ref mut ac) = s.active_call {
                                ac.status = crate::state::CallStatus::Ended(format!("Chyba: {}", e));
                                ac.ended_at = Some(chrono::Utc::now());
                            }
                        }
                    }
                });
            }
        }

        Command::RejectCall { call_id } => {
            let incoming_opt = call_manager.incoming_calls.lock().await.remove(&call_id);
            if let Some(incoming) = incoming_opt {
                let client_clone = client.clone();
                tokio::spawn(async move {
                    let _ = client_clone.voip().reject(&incoming).await;
                });
            }
            let mut s = state.write();
            if let Some(ref mut ac) = s.active_call {
                if ac.call_id == call_id {
                    ac.status = crate::state::CallStatus::Ended("Odmítnuto".to_string());
                    ac.ended_at = Some(chrono::Utc::now());
                }
            }
        }

        Command::EndCall { call_id } => {
            if let Some(handle) = call_manager.active_handle.lock().await.take() {
                tokio::spawn(async move {
                    handle.hangup().await;
                });
            } else {
                let s = state.read();
                if let Some(ref ac) = s.active_call {
                    let client_clone = client.clone();
                    let call_id_c = ac.call_id.clone();
                    let peer_c = ac.peer_jid.clone();
                    let creator_c = ac.call_creator.clone();
                    tokio::spawn(async move {
                        let _ = client_clone.voip().terminate(&call_id_c, &peer_c, &creator_c).await;
                    });
                }
            }
            let end_call_info = {
                let mut s = state.write();
                if let Some(ref mut ac) = s.active_call {
                    let duration_str = if let Some(conn) = ac.connected_at {
                        let secs = (chrono::Utc::now() - conn).num_seconds().max(0);
                        format!("{} min {} s", secs / 60, secs % 60)
                    } else {
                        "0 s".to_string()
                    };
                    let peer_jid = ac.peer_jid.clone();
                    let is_video = ac.is_video;
                    ac.status = crate::state::CallStatus::Ended("Hovor ukončen".to_string());
                    ac.ended_at = Some(chrono::Utc::now());
                    Some((duration_str, peer_jid, is_video))
                } else {
                    None
                }
            };
            if let Some((duration_str, peer_jid, is_video)) = end_call_info {
                let mut s = state.write();
                let canon_chat = s.canonical_jid(&peer_jid);
                let call_icon = if is_video { "📹" } else { "📞" };
                let call_text = format!("{} Hovor ukončen • {}", call_icon, duration_str);
                let msg_id = format!("call_end_{}_{}", call_id, chrono::Utc::now().timestamp());
                let sender = s.self_jid.clone().unwrap_or_else(|| peer_jid.clone());
                let log_msg = crate::state::Message::new_text(
                    msg_id,
                    canon_chat,
                    sender,
                    call_text,
                    chrono::Utc::now(),
                    true,
                    crate::state::MessageStatus::Sent,
                    None,
                    None,
                );
                s.add_message(log_msg);
            }
        }

        Command::ToggleCallMute => {
            let mut s = state.write();
            if let Some(ref mut ac) = s.active_call {
                ac.is_muted = !ac.is_muted;
            }
        }

        Command::ToggleCallVideo => {
            let is_enabled = {
                let mut s = state.write();
                if let Some(ref mut ac) = s.active_call {
                    ac.is_video_enabled = !ac.is_video_enabled;
                    ac.is_video_enabled
                } else {
                    false
                }
            };
            let handle_opt = call_manager.active_handle.lock().await.clone();
            if let Some(handle) = handle_opt {
                tokio::spawn(async move {
                    if is_enabled {
                        let _ = handle.announce_video_enabled().await;
                    } else {
                        let _ = handle.stop_video().await;
                    }
                });
            }
        }
        
        Command::ScheduleMessage { chat_jid, text, scheduled_for } => {
            let msg = crate::state::ScheduledMessage {
                id: uuid::Uuid::new_v4().to_string(),
                chat_jid,
                text,
                scheduled_for,
                created_at: chrono::Utc::now(),
                status: crate::state::ScheduledStatus::Pending,
            };
            {
                let mut s = state.write();
                s.scheduled_messages.push(msg);
            }
            let _ = crate::storage::persist_state(state).await;
        }

        Command::CancelScheduledMessage { id } => {
            {
                let mut s = state.write();
                if let Some(msg) = s.scheduled_messages.iter_mut().find(|m| m.id == id) {
                    msg.status = crate::state::ScheduledStatus::Cancelled;
                }
            }
            let _ = crate::storage::persist_state(state).await;
        }

        Command::SendScheduledMessageNow { id } => {
            let item = {
                let mut s = state.write();
                if let Some(msg) = s.scheduled_messages.iter_mut().find(|m| m.id == id) {
                    msg.status = crate::state::ScheduledStatus::Sent;
                    Some((msg.chat_jid.clone(), msg.text.clone()))
                } else {
                    None
                }
            };
            if let Some((to, text)) = item {
                let wa_msg = wa::Message::text(text.clone());
                if let Ok(res) = client.send_message(to.clone(), wa_msg).await {
                    let sender = client.pn().or(client.lid()).unwrap_or_else(|| to.clone());
                    let sent_msg = Message::new_text(
                        res.message_id,
                        to.clone(),
                        sender,
                        text,
                        chrono::Utc::now(),
                        true,
                        MessageStatus::Sent,
                        None,
                        None,
                    );
                    state.write().add_message(sent_msg);
                }
                let _ = crate::storage::persist_state(state).await;
            }
        }

        Command::ExportChat { chat_jid, format, custom_dir } => {
            let (chat_opt, msgs_opt, contacts_clone, self_name) = {
                let s = state.read();
                let c = s.chats.get(&chat_jid).cloned();
                let m = s.messages.get(&chat_jid).cloned().unwrap_or_default();
                let contacts = s.contacts.clone();
                let name = s.self_name.clone().unwrap_or_else(|| "You".to_string());
                (c, m, contacts, name)
            };
            if let Some(chat) = chat_opt {
                match crate::export::save_export(&chat, &msgs_opt, &contacts_clone, &self_name, format, custom_dir.as_deref()) {
                    Ok(path) => {
                        info!("Chat exported successfully to {:?}", path);
                        let mut s = state.write();
                        if let Some(ref mut dlg) = s.export_dialog {
                            dlg.status_message = Some((true, format!("Exported to {}", path.display())));
                            dlg.exported_path = Some(path);
                        }
                    }
                    Err(e) => {
                        error!("Chat export failed: {}", e);
                        let mut s = state.write();
                        if let Some(ref mut dlg) = s.export_dialog {
                            dlg.status_message = Some((false, format!("Export failed: {}", e)));
                        }
                    }
                }
            }
        }
        
        Command::MarkAsRead { chat_jid, message_ids } => {
            let refs: Vec<&str> = message_ids.iter().map(|s| s.as_str()).collect();
            client.mark_as_read(&chat_jid, None, &refs).await?;
        }
        
        Command::MarkChatAsRead { chat_jid } => {
            client.chat_actions().mark_chat_as_read(&chat_jid, true, None).await?;
        }
        
        Command::ArchiveChat { chat_jid } => {
            client.chat_actions().archive_chat(&chat_jid, None).await?;
            let mut s = state.write();
            s.dirty = true;
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                chat.is_archived = true;
            }
        }
        
        Command::UnarchiveChat { chat_jid } => {
            client.chat_actions().unarchive_chat(&chat_jid, None).await?;
            let mut s = state.write();
            s.dirty = true;
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                chat.is_archived = false;
            }
        }
        
        Command::PinChat { chat_jid } => {
            client.chat_actions().pin_chat(&chat_jid).await?;
            let mut s = state.write();
            s.dirty = true;
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                chat.is_pinned = true;
            }
        }
        
        Command::UnpinChat { chat_jid } => {
            client.chat_actions().unpin_chat(&chat_jid).await?;
            let mut s = state.write();
            s.dirty = true;
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                chat.is_pinned = false;
            }
        }
        
        Command::MuteChat { chat_jid, until } => {
            if let Some(until_time) = until {
                let timestamp_ms = until_time.timestamp_millis();
                client.chat_actions().mute_chat_until(&chat_jid, timestamp_ms).await?;
            } else {
                client.chat_actions().mute_chat(&chat_jid).await?;
            }
            let mut s = state.write();
            s.dirty = true;
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                chat.mute_until = until;
            }
        }
        
        Command::UnmuteChat { chat_jid } => {
            client.chat_actions().unmute_chat(&chat_jid).await?;
            let mut s = state.write();
            s.dirty = true;
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                chat.mute_until = None;
            }
        }
        
        Command::DeleteChat { chat_jid } => {
            let res = client.chat_actions().delete_chat(&chat_jid, false, None).await;
            let mut s = state.write();
            s.dirty = true;
            s.chats.remove(&chat_jid);
            s.messages.remove(&chat_jid);
            if s.selected_chat.as_ref() == Some(&chat_jid) {
                s.selected_chat = None;
            }
            res?;
        }
        
        Command::ClearChat { chat_jid } => {
            let res = client.chat_actions().clear_chat(&chat_jid, false, false, None).await;
            {
                let mut s = state.write();
                s.dirty = true;
                if let Some(msgs) = s.messages.get_mut(&chat_jid) {
                    msgs.clear();
                }
            }
            res?;
        }
        
        Command::StarMessage { chat_jid, message_id } => {
            // star_message takes: chat_jid, participant_jid, message_id, from_me
            // We need to find the message to know from_me and participant_jid
            let (from_me, participant_jid) = state.read().messages.get(&chat_jid)
                .and_then(|msgs| msgs.iter().find(|m| m.id == message_id))
                .map(|m| (m.from_me, if m.from_me { None } else { Some(m.sender_jid.clone()) }))
                .unwrap_or((true, None));
            client.chat_actions().star_message(&chat_jid, participant_jid.as_ref(), &message_id, from_me).await?;
        }
        
        Command::UnstarMessage { chat_jid, message_id } => {
            let (from_me, participant_jid) = state.read().messages.get(&chat_jid)
                .and_then(|msgs| msgs.iter().find(|m| m.id == message_id))
                .map(|m| (m.from_me, if m.from_me { None } else { Some(m.sender_jid.clone()) }))
                .unwrap_or((true, None));
            client.chat_actions().unstar_message(&chat_jid, participant_jid.as_ref(), &message_id, from_me).await?;
        }
        
        Command::PinMessage { chat_jid, message_id, duration_secs } => {
            let canon_chat = state.read().canonical_jid(&chat_jid);
            let (from_me, sender_jid) = state
                .read()
                .messages
                .get(&canon_chat)
                .and_then(|msgs| msgs.iter().find(|m| m.id == message_id))
                .map(|m| (m.from_me, m.sender_jid.clone()))
                .unwrap_or((true, canon_chat.clone()));
            let is_group = state
                .read()
                .chats
                .get(&canon_chat)
                .map_or(false, |chat| chat.is_group);
            let key = wa::MessageKey {
                id: Some(message_id.clone()),
                from_me: Some(from_me),
                remote_jid: Some(canon_chat.to_string()),
                // Group pins must name the original sender, or the server
                // cannot resolve the key.
                participant: if is_group && !from_me {
                    Some(sender_jid.to_string())
                } else {
                    None
                },
                ..Default::default()
            };
            match duration_secs {
                Some(secs) => {
                    client
                        .pin_message(canon_chat.clone(), key, pin_duration(secs))
                        .await?
                }
                None => client.unpin_message(canon_chat.clone(), key).await?,
            }
            {
                let mut s = state.write();
                match duration_secs {
                    Some(secs) => {
                        let me = client.pn().or(client.lid());
                        let entry =
                            crate::state::PinnedMessage::new(message_id, Some(secs), me);
                        s.pinned.insert(canon_chat.clone(), entry);
                    }
                    None => {
                        s.pinned.remove(&canon_chat);
                    }
                }
                s.dirty = true;
            }
        }

        Command::DeleteMessageForMe { chat_jid, message_id } => {
            let res = client.revoke_message(chat_jid.clone(), message_id.clone(), whatsapp_rust::send::RevokeType::Sender).await;
            {
                let mut s = state.write();
                s.dirty = true;
                if let Some(msgs) = s.messages.get_mut(&chat_jid) {
                    msgs.retain(|m| m.id != message_id);
                }
            }
            res?;
        }
        
        Command::RevokeMessage { chat_jid, message_id, is_admin, original_sender } => {
            let revoke_type = if is_admin {
                whatsapp_rust::send::RevokeType::Admin { original_sender: original_sender.unwrap_or_else(|| client.pn().unwrap_or_else(|| client.lid().unwrap())) }
            } else {
                whatsapp_rust::send::RevokeType::Sender
            };
            let res = client.revoke_message(chat_jid.clone(), message_id.clone(), revoke_type).await;
            {
                let mut s = state.write();
                s.dirty = true;
                if let Some(msgs) = s.messages.get_mut(&chat_jid) {
                    msgs.retain(|m| m.id != message_id);
                }
            }
            res?;
        }
        
        Command::SaveContact { jid, name } => {
            {
                let mut s = state.write();
                let contact = s.contacts.entry(jid.clone()).or_insert_with(|| {
                    crate::state::Contact::new(jid.clone(), Some(name.clone()))
                });
                contact.name = Some(name.clone());
                if let Some(chat) = s.chats.get_mut(&jid) {
                    chat.name = name.clone();
                }
                let mapped_lids: Vec<Jid> = s.lid_to_pn.iter()
                    .filter_map(|(lid, pn)| if pn == &jid { Some(lid.clone()) } else { None })
                    .collect();
                for lid in mapped_lids {
                    let lid_c = s.contacts.entry(lid.clone()).or_insert_with(|| {
                        crate::state::Contact::new(lid.clone(), Some(name.clone()))
                    });
                    lid_c.name = Some(name.clone());
                    if let Some(chat) = s.chats.get_mut(&lid) {
                        chat.name = name.clone();
                    }
                }
                if let Some(pn) = s.lid_to_pn.get(&jid).cloned() {
                    let pn_c = s.contacts.entry(pn.clone()).or_insert_with(|| {
                        crate::state::Contact::new(pn.clone(), Some(name.clone()))
                    });
                    pn_c.name = Some(name.clone());
                    if let Some(chat) = s.chats.get_mut(&pn) {
                        chat.name = name.clone();
                    }
                }
            }
            let _ = crate::storage::persist_state(state).await;

            let pn_jid = {
                let s = state.read();
                if jid.is_pn() {
                    Some(jid.clone())
                } else {
                    s.lid_to_pn.get(&jid).cloned()
                }
            };
            if let Some(pn) = pn_jid {
                let _ = client.chat_actions().save_contact(&pn, Some(name), None, true).await;
            }
        }
        
        Command::BlockContact { jid } => {
            let canon = state.read().canonical_jid(&jid);
            client.blocking().block(&canon).await?;
            let mut s = state.write();
            s.contacts
                .entry(canon.clone())
                .or_insert_with(|| crate::state::Contact::new(canon.clone(), None))
                .is_blocked = true;
            s.dirty = true;
        }
        
        Command::UnblockContact { jid } => {
            let canon = state.read().canonical_jid(&jid);
            client.blocking().unblock(&canon).await?;
            let mut s = state.write();
            if let Some(contact) = s.contacts.get_mut(&canon) {
                contact.is_blocked = false;
            }
            s.dirty = true;
        }
        
        Command::CreateGroup { subject, participants } => {
            let mut options = GroupCreateOptions::new(subject.clone());
            for p in &participants {
                options = options.with_participant(whatsapp_rust::wacore::iq::groups::GroupParticipantOptions::new(p.clone()));
            }
            let result = client.groups().create_group(options).await?;
            let group_jid = result.metadata.id.clone();
            let participants: std::collections::HashSet<Jid> =
                result.metadata.participants.iter().map(|p| p.jid.clone()).collect();
            let admins: std::collections::HashSet<Jid> = result
                .metadata
                .participants
                .iter()
                .filter(|p| p.is_admin())
                .map(|p| p.jid.clone())
                .collect();
            {
                let mut s = state.write();
                let chat = s.get_or_create_chat(group_jid.clone(), subject.clone(), true);
                chat.is_group = true;
                chat.participants = participants;
                let snapshot = s.group_info.entry(group_jid.to_string()).or_default();
                snapshot.subject = result.metadata.subject.clone();
                snapshot.description = result.metadata.description.clone();
                snapshot.admins = admins;
                snapshot.is_locked = result.metadata.is_locked;
                snapshot.is_announcement = result.metadata.is_announcement;
                // Land in the new group so the user sees what they created.
                s.selected_chat = Some(group_jid.clone());
                s.selected_messages.clear();
                s.dirty = true;
            }
            let _ = crate::storage::persist_state(&state).await;
        }
        
        Command::JoinGroup { invite_code } => {
            // The vendor pulls the code out of a full chat.whatsapp.com link too.
            let result = client.groups().join_with_invite_code(&invite_code).await?;
            let group_jid = result.group_jid().clone();
            let pending = matches!(
                result,
                whatsapp_rust::wacore::iq::groups::JoinGroupResult::PendingApproval(_)
            );
            {
                let mut s = state.write();
                let chat = s.get_or_create_chat(group_jid.clone(), "Skupina".to_string(), true);
                chat.is_group = true;
                s.selected_chat = Some(group_jid.clone());
                s.selected_messages.clear();
                if pending {
                    info!("Join request for {group_jid} awaits admin approval");
                }
                s.dirty = true;
            }
            refresh_group_info(&client, state, &group_jid).await;
            let _ = crate::storage::persist_state(state).await;
        }

        Command::SendVoiceNote { chat_jid, file_path, duration_seconds, waveform } => {
            let canon_to = state.read().canonical_jid(&chat_jid);
            let client_clone = client.clone();
            let state_clone = state.clone();
            tokio::spawn(async move {
                // Keep the take in the media dir: the temp file the recorder
                // produced is what the local echo will try to play.
                let stored = crate::media::media_dir().join(
                    file_path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| "voice.ogg".to_string()),
                );
                let _ = tokio::fs::create_dir_all(crate::media::media_dir()).await;
                let _ = tokio::fs::copy(&file_path, &stored).await;
                let playable = if stored.exists() { stored.clone() } else { file_path.clone() };

                let bytes = match tokio::fs::read(&file_path).await {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        tracing::error!("Failed to read voice note {}: {}", file_path.display(), err);
                        return;
                    }
                };
                let file_size = bytes.len() as u64;
                let upload = match client_clone
                    .upload(bytes, whatsapp_rust::download::MediaType::Audio, Default::default())
                    .await
                {
                    Ok(upload) => upload,
                    Err(err) => {
                        tracing::error!("Failed to upload voice note: {err}");
                        return;
                    }
                };
                let msg = whatsapp_rust::media::audio_message(
                    upload,
                    whatsapp_rust::media::AudioOptions {
                        ptt: Some(true),
                        duration_seconds: Some(duration_seconds),
                        waveform: if waveform.is_empty() { None } else { Some(waveform) },
                        ..Default::default()
                    },
                );
                match client_clone.send_message(canon_to.clone(), msg).await {
                    Ok(res) => {
                        let sender = client_clone
                            .pn()
                            .or(client_clone.lid())
                            .unwrap_or_else(|| canon_to.clone());
                        let mut local_msg = Message::new_text(
                            res.message_id,
                            canon_to.clone(),
                            sender,
                            "🎤 Voice message".to_string(),
                            chrono::Utc::now(),
                            true,
                            MessageStatus::Sent,
                            None,
                            None,
                        );
                        local_msg.media_type = Some(crate::state::MediaType::Voice);
                        local_msg.media_path = Some(playable.to_string_lossy().to_string());
                        local_msg.media_file_length = Some(file_size);
                        {
                            let mut s = state_clone.write();
                            s.add_message(local_msg);
                            s.consolidate_canonical_chats();
                        }
                        crate::media::MEDIA_REV.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let _ = crate::storage::persist_state(&state_clone).await;
                    }
                    Err(err) => tracing::error!("Failed to send voice note: {err}"),
                }
            });
        }
        
        Command::CreatePoll { chat_jid, question, options, selectable_count } => {
            let canon_to = state.read().canonical_jid(&chat_jid);
            let selectable = selectable_count.max(1);
            match client
                .polls()
                .create(canon_to.clone(), &question, &options, selectable)
                .await
            {
                Ok((res, secret)) => {
                    let sender = client
                        .pn()
                        .or(client.lid())
                        .unwrap_or_else(|| canon_to.clone());
                    let mut local_msg = Message::new_text(
                        res.message_id,
                        canon_to.clone(),
                        sender.clone(),
                        question.clone(),
                        chrono::Utc::now(),
                        true,
                        MessageStatus::Sent,
                        None,
                        None,
                    );
                    local_msg.poll = Some(crate::state::PollData {
                        question,
                        options,
                        selectable_count: selectable,
                        secret,
                        creator: sender,
                        votes: Default::default(),
                        is_quiz: false,
                    });
                    {
                        let mut s = state.write();
                        s.add_message(local_msg);
                        s.consolidate_canonical_chats();
                    }
                    let _ = crate::storage::persist_state(&state).await;
                }
                Err(err) => error!("Failed to create poll: {err}"),
            }
        }

        Command::VotePoll { chat_jid, message_id, poll_creator, message_secret, option_names } => {
            // Same normalisation as `CreatePoll`: a chat may be addressed by its
            // LID while the poll lives under the PN (or the other way round).
            let canon_chat = state.read().canonical_jid(&chat_jid);
            match client
                .polls()
                .vote(canon_chat.clone(), &message_id, &poll_creator, &message_secret, &option_names)
                .await
            {
                Ok(_) => {
                    let me = client
                        .pn()
                        .or(client.lid())
                        .unwrap_or_else(|| poll_creator.clone())
                        .to_string();
                    let mut s = state.write();
                    let mut touched = false;
                    if let Some(msgs) = s.messages.get_mut(&canon_chat) {
                        if let Some(msg) = msgs.iter_mut().find(|m| m.id == message_id) {
                            if let Some(poll) = msg.poll.as_mut() {
                                poll.votes.insert(me, option_names);
                                touched = true;
                            }
                        }
                    }
                    if touched {
                        s.dirty = true;
                    }
                }
                Err(err) => error!("Failed to vote in poll {message_id}: {err}"),
            }
        }

        Command::LeaveGroup { chat_jid } => {
            client.groups().leave(&chat_jid).await?;
            let mut s = state.write();
            let key = chat_jid.to_string();
            s.group_info.remove(&key);
            s.group_info_pending.remove(&key);
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                chat.is_archived = true;
            }
            if s.selected_chat.as_ref() == Some(&chat_jid) {
                s.selected_chat = None;
                s.show_chat_info = false;
            }
            s.dirty = true;
        }
        
        Command::AddParticipants { chat_jid, participants } => {
            client.groups().add_participants(&chat_jid, &participants).await?;
            let mut s = state.write();
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                for p in &participants {
                    chat.participants.insert(p.clone());
                }
            }
        }
        
        Command::RemoveParticipants { chat_jid, participants } => {
            client.groups().remove_participants(&chat_jid, &participants).await?;
            let mut s = state.write();
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                for p in &participants {
                    chat.participants.remove(p);
                }
            }
        }
        
        Command::PromoteParticipants { chat_jid, participants } => {
            client.groups().promote_participants(&chat_jid, &participants).await?;
            refresh_group_info(&client, state, &chat_jid).await;
        }
        
        Command::DemoteParticipants { chat_jid, participants } => {
            client.groups().demote_participants(&chat_jid, &participants).await?;
            refresh_group_info(&client, state, &chat_jid).await;
        }
        
        Command::GetGroupInviteLink { chat_jid } => {
            let link = client.groups().get_invite_link(&chat_jid, false).await?;
            info!("Group invite link: {}", link);
            let mut s = state.write();
            s.group_info.entry(chat_jid.to_string()).or_default().invite_link = Some(link);
        }
        
        Command::RequestGroupInfo { chat_jid } => {
            refresh_group_info(&client, state, &chat_jid).await;
        }
        
        Command::SetGroupSubject { chat_jid, subject } => {
            let validated = GroupSubject::new(&subject)
                .map_err(|e| anyhow::anyhow!("Invalid group subject: {}", e))?;
            client.groups().set_subject(&chat_jid, validated).await?;
            {
                let mut s = state.write();
                if let Some(chat) = s.chats.get_mut(&chat_jid) {
                    chat.name = subject.clone();
                }
                s.group_info.entry(chat_jid.to_string()).or_default().subject = subject;
            }
        }
        
        Command::SetGroupDescription { chat_jid, description } => {
            let validated = GroupDescription::new(&description)
                .map_err(|e| anyhow::anyhow!("Invalid group description: {}", e))?;
            client.groups().set_description(&chat_jid, Some(validated), PreviousDescription::Resolve).await?;
            let mut s = state.write();
            s.group_info.entry(chat_jid.to_string()).or_default().description = Some(description);
            s.dirty = true;
        }
        
        Command::SetEphemeral { chat_jid, expiration } => {
            // Works for 1:1 chats too, not just groups: the vendor sends the
            // same protocol message either way.
            client.groups().set_ephemeral(&chat_jid, expiration).await?;
            let mut s = state.write();
            if expiration == 0 {
                s.ephemeral.remove(&chat_jid);
            } else {
                s.ephemeral.insert(chat_jid.clone(), expiration);
            }
            s.dirty = true;
        }

        Command::SetGroupAnnounce { chat_jid, enabled } => {
            client.groups().set_announce(&chat_jid, enabled).await?;
            if let Some(snapshot) = state.write().group_info.get_mut(&chat_jid.to_string()) {
                snapshot.is_announcement = enabled;
            }
            refresh_group_info(&client, state, &chat_jid).await;
        }

        Command::SetGroupLocked { chat_jid, enabled } => {
            client.groups().set_locked(&chat_jid, enabled).await?;
            if let Some(snapshot) = state.write().group_info.get_mut(&chat_jid.to_string()) {
                snapshot.is_locked = enabled;
            }
            refresh_group_info(&client, state, &chat_jid).await;
        }
        
        Command::SetPresence { status } => {
            client.presence().set(status).await?;
        }
        
        Command::SubscribePresence { jid } => {
            client.presence().subscribe(&jid).await?;
        }
        
        Command::UnsubscribePresence { jid } => {
            client.presence().unsubscribe(&jid).await?;
        }
        
        Command::SetPushName { name } => {
            client.profile().set_push_name(&name).await?;
        }
        
        Command::SetStatusText { text } => {
            client.profile().set_status_text(&text).await?;
        }
        
        Command::SetProfilePicture { data } => {
            client.profile().set_profile_picture(data).await?;
        }
        
        Command::SetPrivacy { category, value } => {
            client.set_privacy_setting(category, value).await?;
        }
        
        Command::SelectChat { chat_jid } => {
            let canon_jid = chat_jid.as_ref().map(|j| state.read().canonical_jid(j));
            {
                let mut s = state.write();
                s.selected_chat = canon_jid.clone().or(chat_jid.clone());
            }
            if let Some(target_jid) = canon_jid {
                let (oldest_id, from_me, timestamp_ms, msg_count) = {
                    let s = state.read();
                    if let Some(msgs) = s.get_chat_messages(&target_jid) {
                        if let Some(first) = msgs.first() {
                            (first.id.clone(), first.from_me, first.timestamp.timestamp_millis(), msgs.len())
                        } else {
                            (String::new(), false, 0, 0)
                        }
                    } else {
                        (String::new(), false, 0, 0)
                    }
                };
                // Auto-fetch history from primary phone via PDO if chat has few messages
                if msg_count < 20 {
                    let client = client.clone();
                    let target = target_jid.clone();
                    tokio::spawn(async move {
                        info!("Requesting on-demand history for {} via PDO", target);
                        if let Err(e) = client.fetch_message_history(&target, &oldest_id, from_me, timestamp_ms, 20).await {
                            debug!("On-demand history request for {}: {}", target, e);
                        }
                    });
                }
            }
        }
        
        Command::SearchChats { query } => {
            let mut s = state.write();
            s.search_query = query;
        }
        
        Command::RefreshChats => {
            let client = client.clone();
            let state_clone = state.clone();
            let active_db_path = active_db_path.clone();
            tokio::spawn(async move {
                let db_path_str = active_db_path.read().await.clone();
                info!("Refreshing chats and resetting app state sync versions to force fresh snapshot");
                if let Ok(store) = SqliteStore::new(&db_path_str).await {
                    use whatsapp_rust::wacore::store::traits::AppSyncStore;
                    let _ = store.clear_mutation_macs("critical_unblock_low").await;
                    let _ = store.set_version("critical_unblock_low", whatsapp_rust::wacore::appstate::hash::HashState::default()).await;
                    let _ = store.clear_mutation_macs("regular_low").await;
                    let _ = store.set_version("regular_low", whatsapp_rust::wacore::appstate::hash::HashState::default()).await;
                }

                let _ = client.process_sync_task(whatsapp_rust::sync_task::MajorSyncTask::AppStateSync {
                    name: whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::CriticalUnblockLow,
                    full_sync: true,
                }).await;
                let _ = client.process_sync_task(whatsapp_rust::sync_task::MajorSyncTask::AppStateSync {
                    name: whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::RegularLow,
                    full_sync: true,
                }).await;
                let _ = client.process_sync_task(whatsapp_rust::sync_task::MajorSyncTask::AppStateSync {
                    name: whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::CriticalBlock,
                    full_sync: true,
                }).await;
                let pn_jids: Vec<Jid> = {
                    let s = state_clone.read();
                    s.lid_to_pn.values().cloned().take(50).collect()
                };
                if !pn_jids.is_empty() {
                    let _ = client.contacts().get_user_info(&pn_jids).await;
                }
                crate::avatars::sync_all_avatars(&client, &state_clone, true).await;
                FETCHING_INITIAL_HISTORY.store(false, Ordering::SeqCst);
                auto_fetch_initial_messages(&client, &state_clone, true).await;
            });
        }
        
        Command::RequestHistorySync { chat_jid, count } => {
            let target_jid = {
                let s = state.read();
                let raw = chat_jid.or_else(|| s.selected_chat.clone());
                raw.map(|j| s.canonical_jid(&j))
            };
            if let Some(target) = target_jid {
                let (oldest_id, from_me, timestamp_ms) = {
                    let s = state.read();
                    if let Some(msgs) = s.get_chat_messages(&target) {
                        if let Some(first) = msgs.first() {
                            (first.id.clone(), first.from_me, first.timestamp.timestamp_millis())
                        } else {
                            (String::new(), false, 0)
                        }
                    } else {
                        (String::new(), false, 0)
                    }
                };
                let client = client.clone();
                tokio::spawn(async move {
                    info!("Explicitly requesting {} messages history for {} via PDO", count, target);
                    if let Err(e) = client.fetch_message_history(&target, &oldest_id, from_me, timestamp_ms, count as i32).await {
                        warn!("Failed to fetch message history for {}: {}", target, e);
                    }
                });
            }
        }
        
        Command::Connect => {
            info!("Connect requested - triggering QR code generation");
            // The bot will auto-generate QR code on connect if not already connected
            // No explicit action needed - connection happens on bot startup
        }
        
        Command::RequestPairCode { phone_number } => {
            info!("Pair code requested for phone: {}", phone_number);
            let options = PairCodeOptions {
                phone_number: phone_number.clone(),
                show_push_notification: true,
                custom_code: None,
                ..Default::default()
            };
            let client_clone = client.clone();
            tokio::spawn(async move {
                match client_clone.pair_with_code(options).await {
                    Ok(code) => info!("Pair code generated: {}", code),
                    Err(e) => error!("Failed to request pair code: {}", e),
                }
            });
        }
        
        Command::Disconnect => {
            info!("Logout requested: deregistering companion session...");
            let client = client.clone();
            let bot_handle_arc = bot_handle_arc.clone();
            let active_db_path = active_db_path.clone();
            let data_dir = data_dir.to_path_buf();
            let state = state.clone();
            let tx_status = tx_status.clone();
            let call_manager = call_manager.clone();
            let active_client = active_client.clone();

            tokio::spawn(async move {
                // 1. Send logout IQ to WhatsApp servers so primary phone unlinks companion
                info!("Sending logout IQ to WhatsApp server...");
                client.logout().await;

                // 2. Shut down and abort running bot instance
                if let Some(h) = bot_handle_arc.lock().await.take() {
                    h.client().disconnect().await;
                    h.abort();
                }

                // 3. Small pause to ensure background sockets drop
                tokio::time::sleep(Duration::from_millis(300)).await;

                // 4. Create a completely brand new session database
                let new_session_name = format!("whatsapp_{}.db", Utc::now().timestamp_millis());
                let new_db_path = data_dir.join(&new_session_name);
                let new_db_path_str = new_db_path.to_string_lossy().to_string();
                let _ = std::fs::write(data_dir.join("current_session.txt"), &new_session_name);
                *active_db_path.write().await = new_db_path_str.clone();

                // 5. Delete cached UI state
                let ui_state_path = data_dir.join("ui_state.json");
                let _ = std::fs::remove_file(&ui_state_path);

                // 6. Reset in-memory state
                {
                    let mut s = state.write();
                    s.self_jid = None;
                    s.self_name = None;
                    s.chats.clear();
                    s.messages.clear();
                    s.contacts.clear();
                    s.lid_to_pn.clear();
                    s.selected_chat = None;
                    s.initial_history_fetched = false;
                    s.connection = ConnectionStatus::Connecting;
                    s.dirty = false;
                }
                let _ = tx_status.send(ConnectionStatus::Connecting);

                // 7. Spawn fresh bot instance with brand new, empty DB to generate new pairing QR code
                match create_and_spawn_bot(
                    &new_db_path_str,
                    &state,
                    &tx_status,
                    &call_manager,
                ).await {
                    Ok(new_handle) => {
                        *active_client.write().await = new_handle.client();
                        *bot_handle_arc.lock().await = Some(new_handle);
                        info!("Fresh bot spawned after logout with database {}; waiting for new pairing QR code", new_session_name);
                    }
                    Err(e) => {
                        error!("Failed to spawn fresh bot after logout: {}", e);
                        let err_msg = format!("Nepodařilo se vytvořit novou relaci: {}", e);
                        state.write().connection = ConnectionStatus::Error(err_msg.clone());
                        let _ = tx_status.send(ConnectionStatus::Error(err_msg));
                    }
                }
            });
        }
        
        Command::Shutdown => {
            info!("Shutdown requested");
        }
        
        _ => {}
    }
    Ok(())
}