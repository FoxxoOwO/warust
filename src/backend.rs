//! Backend task - runs whatsapp-rust Bot and handles commands

use std::sync::Arc;
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

/// Spawn the backend task
pub async fn spawn_backend(state: SharedState) -> Result<BackendHandle> {
    let (tx_command, mut rx_command) = mpsc::unbounded_channel::<Command>();
    let (tx_status, rx_status) = watch::channel(ConnectionStatus::Disconnected);
    
    let state_clone = state.clone();
    
    // Build the bot
    let db_path = dirs::data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("warust")
        .join("whatsapp.db");
    
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // SqliteStore expects a file path, not a sqlite:// URL
    let db_path_str = db_path.to_string_lossy().to_string();
    
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

    let store_clone = store.clone();
    let call_manager = Arc::new(CallManager {
        incoming_calls: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        active_handle: tokio::sync::Mutex::new(None),
        mic_tx: tokio::sync::Mutex::new(None),
        video_tx: tokio::sync::Mutex::new(None),
    });
    let builder = Bot::builder()
        .with_backend(store)
        .on_qr_code({
            let tx = tx_status.clone();
            move |code, timeout| {
                info!("QR code generated: {} chars, valid for {}s", code.len(), timeout.as_secs());
                let _ = tx.send(ConnectionStatus::QrCode(code.to_string()));
                async {}
            }
        })
        .on_pair_code({
            let tx = tx_status.clone();
            move |code, timeout| {
                info!("Pair code generated: {}, valid for {}s", code, timeout.as_secs());
                let _ = tx.send(ConnectionStatus::PairCode(code.to_string()));
                async {}
            }
        })
        .on_connected({
            let tx = tx_status.clone();
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
                });

                async {}
            }
        })
        .on_logged_out({
            let tx = tx_status.clone();
            move |_| {
                let _ = tx.send(ConnectionStatus::Disconnected);
                warn!("Logged out from WhatsApp");
                async {}
            }
        })
        .on_message({
            let state = state_clone.clone();
            let tx_status = tx_status.clone();
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
            let tx_status = tx_status.clone();
            let call_mgr = call_manager.clone();
            move |event: Arc<Event>, client: Arc<Client>| {
                let state = state.clone();
                let tx_status = tx_status.clone();
                let call_mgr = call_mgr.clone();
                async move {
                    handle_event(event, client, state, tx_status, call_mgr).await;
                }
            }
        });
    
    // Log status transitions for debugging
    let mut rx_status_log = tx_status.subscribe();
    tokio::spawn(async move {
        while rx_status_log.changed().await.is_ok() {
            let status = rx_status_log.borrow().clone();
            info!("Backend connection status changed to: {:?}", status);
        }
    });

    let bot = builder.build().await?;
    let bot_handle = bot.spawn();
    let client = bot_handle.client();
    let bot_handle_arc = Arc::new(tokio::sync::Mutex::new(Some(bot_handle)));
    
    // Spawn command handler
    let client_for_cmds = client.clone();
    let state_for_cmds = state.clone();
    let tx_status_for_cmds = tx_status.clone();
    let store_for_cmds = store_clone.clone();
    let call_mgr_for_cmds = call_manager.clone();
    
    tokio::spawn(async move {
        while let Some(cmd) = rx_command.recv().await {
            if let Err(e) = handle_command(cmd, &client_for_cmds, &state_for_cmds, &tx_status_for_cmds, &store_for_cmds, &call_mgr_for_cmds).await {
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
    let client_for_scheduled = client.clone();
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
                match client_for_scheduled.send_message(to.clone(), wa_msg).await {
                    Ok(res) => {
                        let sender = client_for_scheduled.pn().or(client_for_scheduled.lid()).unwrap_or_else(|| to.clone());
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

    // Check if this is a protocol message (revoke, ephemeral sync, etc.) without edited message
    if let Some(pm) = base_msg.protocol_message.as_option() {
        if !pm.edited_message.is_set() {
            return;
        }
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
    // Check for poll
    if message.poll_creation_message.is_set() {
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

fn is_jid_group(jid: &Jid) -> bool {
    // Check if it's a group JID by looking at the server type
    // Group JIDs end with @g.us
    jid.to_string().ends_with("@g.us")
}

async fn handle_event(
    event: Arc<Event>,
    _client: Arc<Client>,
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
                let mut new_chats = Vec::new();
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

                    new_chats.push((chat_jid.clone(), chat_name, is_group));
                    
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
                    for (jid, name, is_group) in new_chats {
                        let resolved_name = s.resolve_display_name(&jid).unwrap_or(name);
                        s.get_or_create_chat(jid, resolved_name, is_group);
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
            } else {
                let tx = tx_status.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                    let _ = tx.send_if_modified(|status| {
                        if matches!(status, ConnectionStatus::Syncing(_)) {
                            info!("History sync quiescent, setting status to Connected");
                            *status = ConnectionStatus::Connected;
                            true
                        } else {
                            false
                        }
                    });
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
                    _client.clone(),
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
            let client = _client.clone();
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

async fn handle_command(
    cmd: Command,
    client: &Arc<Client>,
    state: &SharedState,
    tx_status: &watch::Sender<ConnectionStatus>,
    store: &SqliteStore,
    call_manager: &Arc<CallManager>,
) -> Result<()> {
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
        
        Command::SendMedia { chat_jid, file_path, caption, reply_to } => {
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

        Command::DownloadMedia { chat_jid, message_id } => {
            let canon_chat = state.read().canonical_jid(&chat_jid);
            let msg_clone = {
                let s = state.read();
                s.get_chat_messages(&canon_chat)
                    .and_then(|msgs| msgs.iter().find(|m| m.id == message_id))
                    .cloned()
            };
            if let Some(msg) = msg_clone {
                if let (Some(direct_path), Some(media_key), Some(file_sha256), Some(media_type)) =
                    (msg.media_direct_path, msg.media_key, msg.media_file_sha256, msg.media_type)
                {
                    let wa_type = match media_type {
                        crate::state::MediaType::Image => whatsapp_rust::download::MediaType::Image,
                        crate::state::MediaType::Video => whatsapp_rust::download::MediaType::Video,
                        crate::state::MediaType::Audio | crate::state::MediaType::Voice => whatsapp_rust::download::MediaType::Audio,
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
                    
                    let client_clone = client.clone();
                    let state_clone = state.clone();
                    let target_path = crate::media::get_local_path(
                        &message_id,
                        msg.media_caption.as_deref(),
                        msg.media_mimetype.as_deref(),
                        media_type,
                    );
                    
                    tokio::spawn(async move {
                        tracing::info!("Starting download for media message {} -> {}", message_id, target_path.display());
                        match client_clone.download_from_params(&params).await {
                            Ok(bytes) => {
                                let _ = std::fs::create_dir_all(crate::media::media_dir());
                                if let Err(e) = tokio::fs::write(&target_path, bytes).await {
                                    tracing::error!("Failed to save media to {}: {}", target_path.display(), e);
                                } else {
                                    tracing::info!("Media successfully saved to {}", target_path.display());
                                    {
                                        let mut s = state_clone.write();
                                        if let Some(msgs) = s.messages.get_mut(&canon_chat) {
                                            if let Some(m) = msgs.iter_mut().find(|m| m.id == message_id) {
                                                m.media_path = Some(target_path.to_string_lossy().to_string());
                                            }
                                        }
                                    }
                                    crate::media::MEDIA_REV.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    let _ = crate::storage::persist_state(&state_clone).await;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to download media for {}: {}", message_id, e);
                            }
                        }
                    });
                }
            }
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
            client.blocking().block(&jid).await?;
        }
        
        Command::UnblockContact { jid } => {
            client.blocking().unblock(&jid).await?;
        }
        
        Command::CreateGroup { subject, participants } => {
            let mut options = GroupCreateOptions::new(subject.clone());
            for p in &participants {
                options = options.with_participant(whatsapp_rust::wacore::iq::groups::GroupParticipantOptions::new(p.clone()));
            }
            let result = client.groups().create_group(options).await?;
            let mut s = state.write();
            let chat = s.get_or_create_chat(result.metadata.id.clone(), subject, true);
            chat.is_group = true;
        }
        
        Command::JoinGroup { invite_code } => {
            // join_via_link is not available, need to use different method
            // For now, no-op
            let _ = invite_code;
        }
        
        Command::LeaveGroup { chat_jid } => {
            client.groups().leave(&chat_jid).await?;
            let mut s = state.write();
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                chat.is_archived = true;
            }
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
        }
        
        Command::DemoteParticipants { chat_jid, participants } => {
            client.groups().demote_participants(&chat_jid, &participants).await?;
        }
        
        Command::GetGroupInviteLink { chat_jid } => {
            let link = client.groups().get_invite_link(&chat_jid, false).await?;
            info!("Group invite link: {}", link);
        }
        
        Command::SetGroupSubject { chat_jid, subject } => {
            let validated = GroupSubject::new(&subject)
                .map_err(|e| anyhow::anyhow!("Invalid group subject: {}", e))?;
            client.groups().set_subject(&chat_jid, validated).await?;
            let mut s = state.write();
            if let Some(chat) = s.chats.get_mut(&chat_jid) {
                chat.name = subject;
            }
        }
        
        Command::SetGroupDescription { chat_jid, description } => {
            let validated = GroupDescription::new(&description)
                .map_err(|e| anyhow::anyhow!("Invalid group description: {}", e))?;
            client.groups().set_description(&chat_jid, Some(validated), PreviousDescription::Resolve).await?;
        }
        
        Command::SetGroupEphemeral { chat_jid, expiration } => {
            client.groups().set_ephemeral(&chat_jid, expiration).await?;
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
                if msg_count < 10 {
                    let client = client.clone();
                    let target = target_jid.clone();
                    tokio::spawn(async move {
                        info!("Requesting on-demand history for {} via PDO", target);
                        if let Err(e) = client.fetch_message_history(&target, &oldest_id, from_me, timestamp_ms, 50).await {
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
            let store = store.clone();
            tokio::spawn(async move {
                info!("Refreshing chats and resetting app state sync versions to force fresh snapshot");
                use whatsapp_rust::wacore::store::traits::AppSyncStore;
                let _ = store.clear_mutation_macs("critical_unblock_low").await;
                let _ = store.set_version("critical_unblock_low", whatsapp_rust::wacore::appstate::hash::HashState::default()).await;
                let _ = store.clear_mutation_macs("regular_low").await;
                let _ = store.set_version("regular_low", whatsapp_rust::wacore::appstate::hash::HashState::default()).await;

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
            info!("Disconnect requested");
        }
        
        Command::Shutdown => {
            info!("Shutdown requested");
        }
        
        _ => {}
    }
    Ok(())
}