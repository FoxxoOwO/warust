//! Conversation View UI - WhatsApp Web/Desktop style

use egui::{Align, Color32, Context, FontId, Frame, Key, Layout, Margin, Rect, RichText, ScrollArea, TextEdit, Ui, Widget};

/// Delay between two new messages of the same batch fading in.
const ARRIVAL_STAGGER: f32 = 0.015;
use crate::state::{AppState, SharedState, Message};
use crate::ui::components::{MessageBubble, Avatar};
use crate::util::format_jid;
use crate::commands::Command;
use whatsapp_rust::wacore_binary::Jid;
use std::borrow::Cow;

/// Conversation panel - main chat area like WhatsApp Web
pub fn show_conversation(ui: &mut Ui, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let mut s = state.write();
    
    if let Some(chat_jid) = s.selected_chat.clone() {
        let canon_jid = s.canonical_jid(&chat_jid);
        let chat = s.chats.get(&canon_jid).or_else(|| s.chats.get(&chat_jid)).cloned();
        // Taken instead of cloned: the state write lock is held for the whole
        // frame, so the history is put back before anything else can look.
        let (messages_key, messages) = s.take_chat_messages(&canon_jid);

        if let Some(chat) = chat {
            show_chat_view(ui, &mut s, &chat, &messages, &canon_jid, backend);
        }
        s.put_chat_messages(messages_key, messages);
    } else {
        // No chat selected - show welcome screen
        show_welcome_screen(ui, &s);
    }
}

fn show_welcome_screen(ui: &mut Ui, _state: &AppState) {
    Frame::new()
        .fill(Color32::from_rgb(0x11, 0x1B, 0x21))
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    // WhatsApp logo
                    ui.label(RichText::new("💬").font(FontId::proportional(64.0)));
                    ui.add_space(24.0);
                    
                    ui.label(RichText::new("Warust").font(FontId::proportional(32.0)).strong().color(Color32::WHITE));
                    ui.add_space(8.0);
                    ui.label(RichText::new("WhatsApp Desktop Client").font(FontId::proportional(16.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    
                    ui.add_space(40.0);
                    
                    ui.label(RichText::new("Select a chat from the list").font(FontId::proportional(16.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    ui.add_space(8.0);
                    ui.label(RichText::new("or start a new conversation").font(FontId::proportional(16.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    
                    ui.add_space(40.0);
                    
                    ui.label(RichText::new("End-to-end encrypted").font(FontId::proportional(13.0)).color(Color32::from_rgb(0x53, 0xB6, 0x9A)));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Your messages and calls are secured").font(FontId::proportional(13.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                });
            });
        });
}

fn show_chat_view(ui: &mut Ui, state: &mut AppState, chat: &crate::state::Chat, messages: &[Message], chat_jid: &Jid, backend: &crate::backend::BackendHandle) {
    let selecting = !state.selected_messages.is_empty();

    // Switching chats used to snap the whole pane into place. A short fade makes
    // the swap read as one movement; the id is the chat, so it plays once.
    let appear = crate::ui::animations::entrance(
        ui.ctx(),
        egui::Id::unique(("chat_view", chat_jid.to_string())),
        crate::ui::animations::MEDIUM,
    );
    if appear < 1.0 {
        ui.set_opacity(appear);
    }

    // The info panel spans the full height on the right, so it has to claim its
    // space before the header and message area do.
    if state.show_chat_info {
        egui::Panel::right("chat_info_panel")
            .resizable(true)
            .default_size(320.0)
            .min_size(260.0)
            .max_size(420.0)
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::chat_list::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .show(ui, |ui| {
                crate::ui::chat_info::show(ui, state, chat, backend);
            });
    }

    // While messages are ticked the header turns into the bulk-action bar and
    // the composer is hidden: sending a draft mid-selection would be surprising.
    if selecting {
        show_selection_bar(ui, state, chat_jid, backend);
    } else {
        show_chat_header(ui, state, chat, backend);
    }

    ui.separator();

    // Above the history, below the header — the same slot WhatsApp uses.
    show_pinned_banner(ui, state, messages, chat_jid, backend);

    // The composer is its own bottom panel: a bottom-up layout with a zero-sized
    // child let the history paint over it, because the child reported no height.
    if !selecting {
        egui::Panel::bottom("composer_bar")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 0)))
            .show(ui, |ui| {
                show_composer(ui, state, chat_jid, backend);
            });
    }

    show_messages_area(ui, state, chat, messages, chat_jid, backend);
}

/// Bulk-action bar shown in place of the chat header while messages are ticked.
fn show_selection_bar(ui: &mut Ui, state: &mut AppState, chat_jid: &Jid, _backend: &crate::backend::BackendHandle) {
    let count = state.selected_messages.len();
    let mut clear = false;
    let mut open_forward = false;

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui.button(RichText::new("✕").font(FontId::proportional(16.0))).clicked() {
            clear = true;
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("Vybráno: {}", count))
                .font(FontId::proportional(15.0))
                .color(Color32::WHITE),
        );

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            let forward_btn = ui.add_enabled(
                count > 0,
                egui::Button::new(
                    RichText::new("↪  Přeposlat")
                        .font(FontId::proportional(13.0))
                        .color(Color32::from_rgb(0x0B, 0x14, 0x18)),
                )
                .fill(crate::theme::ACCENT)
                .corner_radius(egui::CornerRadius::same(6)),
            );
            if forward_btn.clicked() {
                open_forward = true;
            }
        });
    });
    ui.add_space(8.0);

    if clear {
        state.selected_messages.clear();
    } else if open_forward {
        state.forward_dialog = Some(crate::state::ForwardDialogState {
            source_chat: chat_jid.clone(),
            message_ids: state.selected_messages.iter().cloned().collect(),
            search: String::new(),
        });
    }
}

fn show_chat_header(ui: &mut Ui, state: &mut AppState, chat: &crate::state::Chat, backend: &crate::backend::BackendHandle) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        
        // Avatar
        Avatar::new(&chat.name).jid(chat.jid.clone()).size(42.0).ui(ui);
        ui.add_space(12.0);
        
        // Name and status
        ui.vertical(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(&chat.name).font(FontId::proportional(17.0)).strong().color(Color32::WHITE));
            
            if chat.is_currently_typing() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("píše").font(FontId::proportional(12.0)).color(Color32::from_rgb(0x25, 0xD3, 0x66)));
                    crate::ui::animations::bouncing_dots(ui, Color32::from_rgb(0x25, 0xD3, 0x66), 2.5, 3.5);
                    ui.ctx().request_repaint();
                });
            } else {
                let status_text = if chat.is_group {
                    format!("{} participants", chat.participants.len())
                } else if crate::util::is_real_name(&chat.name) {
                    crate::util::format_jid(&chat.jid)
                } else {
                    "".to_string()
                };
                
                if !status_text.is_empty() {
                    ui.label(RichText::new(status_text).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                }
            }
        });
        
        // Right side buttons
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(12.0);
            
            // More menu
            ui.menu_button(RichText::new("⋮").font(FontId::proportional(22.0)), |ui| {
                // Search in this chat
                if ui.button("🔍  Search in this chat").clicked() {
                    state.show_search_dialog = true;
                    state.search_filter.scope = crate::search::SearchScope::CurrentChat;
                    ui.close();
                }

                // Export conversation
                if ui.button("📤  Export conversation").clicked() {
                    state.export_dialog = Some(crate::state::ExportDialogState {
                        chat_jid: chat.jid.clone(),
                        chat_name: chat.name.clone(),
                        format: crate::export::ExportFormat::Txt,
                        status_message: None,
                        exported_path: None,
                    });
                    ui.close();
                }

                // Schedule message
                if ui.button("🕒  Schedule message").clicked() {
                    state.schedule_target_chat = Some(chat.jid.clone());
                    state.schedule_text = state.composer_text.clone();
                    state.show_schedule_dialog = true;
                    ui.close();
                }

                ui.separator();

                if !chat.is_group {
                    if ui.button("✏  Edit contact name").clicked() {
                        state.editing_contact = Some((chat.jid.clone(), chat.name.clone()));
                        ui.close();
                    }
                    ui.separator();
                }
                let is_pinned = chat.is_pinned;
                if ui.button(if is_pinned { "📌 Unpin chat" } else { "📌 Pin chat" }).clicked() {
                    let cmd = if is_pinned {
                        Command::UnpinChat { chat_jid: chat.jid.clone() }
                    } else {
                        Command::PinChat { chat_jid: chat.jid.clone() }
                    };
                    let _ = backend.send_command(cmd);
                    ui.close();
                }
                let is_muted = chat.is_muted();
                // A submenu, like WhatsApp Web: mute is always a choice of
                // duration, not a single toggle.
                ui.menu_button(
                    if is_muted { "🔔  Umlčení" } else { "🔕  Umlčení" },
                    |ui| {
                        if ui.button("Vypnout umlčení").clicked() {
                            let _ = backend.send_command(Command::UnmuteChat {
                                chat_jid: chat.jid.clone(),
                            });
                            ui.close();
                        }
                        for (label, hours) in [("8 hodin", 8i64), ("1 týden", 24 * 7)] {
                            if ui.button(label).clicked() {
                                let until = chrono::Utc::now() + chrono::Duration::hours(hours);
                                let _ = backend.send_command(Command::MuteChat {
                                    chat_jid: chat.jid.clone(),
                                    until: Some(until),
                                });
                                ui.close();
                            }
                        }
                        if ui.button("Vždy").clicked() {
                            let _ = backend.send_command(Command::MuteChat {
                                chat_jid: chat.jid.clone(),
                                until: None,
                            });
                            ui.close();
                        }
                    },
                );
                if ui.button("ℹ  Info o chatu").clicked() {
                    state.show_chat_info = !state.show_chat_info;
                    ui.close();
                }
                if ui.button("🗑 Clear messages").clicked() {
                    let _ = backend.send_command(Command::ClearChat { chat_jid: chat.jid.clone() });
                    ui.close();
                }
            });

            ui.add_space(4.0);

            // Search in chat shortcut
            if ui.add(egui::Button::new(RichText::new("🔍").font(FontId::proportional(18.0)))
                .frame(false)
                .fill(Color32::TRANSPARENT))
                .on_hover_text("Search messages in this chat")
                .clicked() {
                    state.show_search_dialog = true;
                    state.search_filter.scope = crate::search::SearchScope::CurrentChat;
                }
            
            ui.add_space(4.0);

            // Chat info panel toggle
            if ui
                .add(
                    egui::Button::new(RichText::new("ℹ").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(Color32::TRANSPARENT),
                )
                .on_hover_text("Info o chatu")
                .clicked()
            {
                state.show_chat_info = !state.show_chat_info;
            }

            ui.add_space(4.0);

            // Video call
            if ui.add(egui::Button::new(RichText::new("📹").font(FontId::proportional(20.0)))
                .frame(false)
                .fill(Color32::TRANSPARENT))
                .on_hover_text(format!("Videohovor s {}", chat.name))
                .clicked() {
                    let _ = backend.send_command(Command::StartCall {
                        peer_jid: chat.jid.clone(),
                        is_video: true,
                    });
                }
            
            ui.add_space(4.0);
            
            // Voice call
            if ui.add(egui::Button::new(RichText::new("📞").font(FontId::proportional(20.0)))
                .frame(false)
                .fill(Color32::TRANSPARENT))
                .on_hover_text(format!("Hlasový hovor s {}", chat.name))
                .clicked() {
                    let _ = backend.send_command(Command::StartCall {
                        peer_jid: chat.jid.clone(),
                        is_video: false,
                    });
                }
            
            ui.add_space(12.0);
        });
    });
    ui.add_space(8.0);
}

/// One-line summary of a pinned message, used by the banner.
fn pinned_preview(message: &Message) -> String {
    let text = message.text.trim();
    if !text.is_empty() {
        return text.to_string();
    }
    match message.media_type {
        Some(crate::state::MediaType::Image) => "📷 Fotka".to_string(),
        Some(crate::state::MediaType::Video) => "📹 Video".to_string(),
        Some(crate::state::MediaType::Audio) => "🎵 Audio".to_string(),
        Some(crate::state::MediaType::Voice) => "🎤 Hlasová zpráva".to_string(),
        Some(crate::state::MediaType::Document) => "📄 Dokument".to_string(),
        Some(crate::state::MediaType::Sticker) => "🎨 Samolepka".to_string(),
        None => {
            if message.poll.is_some() {
                "📊 Anketa".to_string()
            } else {
                "Zpráva".to_string()
            }
        }
    }
}

/// Banner with the chat's pinned message: click jumps to it, ✕ unpins.
fn show_pinned_banner(
    ui: &mut Ui,
    state: &mut AppState,
    messages: &[Message],
    chat_jid: &Jid,
    backend: &crate::backend::BackendHandle,
) {
    let Some(pin) = state
        .pinned
        .get(chat_jid)
        .filter(|pin| !pin.expired())
        .cloned()
    else {
        return;
    };
    let preview = messages
        .iter()
        .find(|message| message.id == pin.message_id)
        .map(pinned_preview)
        // The pin can outlive the message (deleted locally, not yet resynced).
        .unwrap_or_else(|| "Připnutá zpráva už není k dispozici".to_string());

    let mut jump = false;
    let mut unpin = false;

    egui::Frame::new()
        .fill(Color32::from_rgb(0x1D, 0x28, 0x2E))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("📌").font(FontId::proportional(14.0)));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Připnutá zpráva")
                            .font(FontId::proportional(11.0))
                            .color(crate::theme::ACCENT),
                    );
                    let label = ui.add(
                        egui::Label::new(
                            RichText::new(preview)
                                .font(FontId::proportional(12.0))
                                .color(Color32::WHITE),
                        )
                        .wrap_mode(egui::TextWrapMode::Truncate)
                        .sense(egui::Sense::click()),
                    );
                    if label.clicked() {
                        jump = true;
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("✕").font(FontId::proportional(13.0)),
                            )
                            .frame(false),
                        )
                        .on_hover_text("Odepnout")
                        .clicked()
                    {
                        unpin = true;
                    }
                });
            });
        });
    ui.add_space(2.0);

    if jump {
        state.jump_target = Some(crate::state::JumpTarget::new(pin.message_id.clone()));
    }
    if unpin {
        let _ = backend.send_command(Command::PinMessage {
            chat_jid: chat_jid.clone(),
            message_id: pin.message_id,
            duration_secs: None,
        });
    }
}

/// Height assumed for a row that has not been drawn yet. Refined frame by frame:
/// the first frame of a chat uses it for the whole history, every later frame
/// uses what was measured for the rows it drew.
const ROW_HEIGHT_FALLBACK: f32 = 64.0;
/// Height of the "today/yesterday/date" separator above a row, measured once —
/// it is a fixed one-line label, not text that depends on the message.
const DATE_SEPARATOR_HEIGHT: f32 = 30.0;
/// Rows this far outside the viewport are drawn too, so a fast scroll does not
/// show a gap before the next frame arrives.
const ROW_OVERSCAN: f32 = 700.0;
/// Stop remembering heights at some point; a long session with many chats would
/// otherwise keep a map entry per message ever drawn.
const ROW_HEIGHT_CACHE_LIMIT: usize = 20_000;
/// Diameter of the round "back to the newest message" button.
const JUMP_TO_BOTTOM_SIZE: f32 = 38.0;
/// The button only appears once the user is genuinely reading older messages.
/// Around a line of text of slack keeps it from flickering while a scroll comes
/// to rest a pixel above the bottom.
const JUMP_TO_BOTTOM_SLACK: f32 = 48.0;

/// Whether the "jump to the newest message" button belongs on screen.
///
/// `content_height` is the full height of the history, `viewport_height` the
/// visible part and `offset` how far down the user scrolled. The button is
/// pointless when everything fits, and noise when the tail is still on screen.
fn should_show_jump_to_bottom(content_height: f32, viewport_height: f32, offset: f32) -> bool {
    let scrollable = content_height - viewport_height;
    if scrollable <= JUMP_TO_BOTTOM_SLACK {
        return false;
    }
    scrollable - offset > JUMP_TO_BOTTOM_SLACK
}

/// Which rows a virtualized list has to draw, and how much empty space stands in
/// for the rest.
///
/// The message list used to lay out the entire history every frame: at 2000
/// messages that is ~4 ms per frame for twelve visible bubbles. The window is
/// contiguous, so the rows outside it can be replaced by two spaces.
#[derive(Debug, Clone, Copy, PartialEq)]
struct VisibleRows {
    /// Index of the first drawn message; `messages.len()` when nothing is drawn.
    first: usize,
    /// How many messages are drawn.
    count: usize,
    /// Space replacing the rows above the window.
    before: f32,
    /// Space replacing the rows below the window.
    after: f32,
}

impl VisibleRows {
    fn contains(&self, idx: usize) -> bool {
        idx >= self.first && idx < self.first + self.count
    }
}

/// Row heights for the message list: a cached bubble height plus a date
/// separator decided from the neighbouring message.
///
/// Pure, so the range arithmetic can be tested without an egui context — a
/// scroll area that renders the wrong rows is hard to catch by eye and easy to
/// catch with numbers.
struct RowModel<'a> {
    heights: &'a std::collections::HashMap<String, f32>,
    messages: &'a [Message],
}

impl RowModel<'_> {
    fn has_separator(&self, idx: usize) -> bool {
        idx == 0
            || self.messages[idx - 1].timestamp.date_naive() != self.messages[idx].timestamp.date_naive()
    }

    fn height(&self, idx: usize) -> f32 {
        let bubble = self
            .heights
            .get(&self.messages[idx].id)
            .copied()
            .unwrap_or(ROW_HEIGHT_FALLBACK);
        bubble + if self.has_separator(idx) { DATE_SEPARATOR_HEIGHT } else { 0.0 }
    }

    /// Total height of the history above `idx`, i.e. where that row starts.
    fn top_of(&self, idx: usize) -> f32 {
        (0..idx.min(self.messages.len())).map(|i| self.height(i)).sum()
    }

    /// The window of rows to draw, with `top` being where row 0 starts.
    ///
    /// `viewport` has to be in the *same* coordinate system as `top`. The scroll
    /// area hands out its viewport in content coordinates while `ui.cursor()` is
    /// in screen coordinates; mixing the two put the window a whole scroll
    /// offset past the end of the history and the panel stayed empty.
    fn visible(&self, top: f32, viewport: Rect, overscan: f32) -> VisibleRows {
        if self.messages.is_empty() {
            return VisibleRows { first: 0, count: 0, before: 0.0, after: 0.0 };
        }
        let total: f32 = (0..self.messages.len()).map(|i| self.height(i)).sum();
        // A scroll offset remembered from before the heights were measured can
        // point past the end of a history that turned out shorter. Clamping keeps
        // the panel populated instead of blank until the next scroll event.
        let max_from = (total - viewport.height()).max(0.0);
        let from = (viewport.min.y - top - overscan).clamp(0.0, max_from);
        let to = from + viewport.height() + overscan * 2.0;
        let mut y = 0.0f32;
        let mut first = self.messages.len();
        let mut count = 0usize;
        let mut before = 0.0;
        let mut end = 0.0;
        for idx in 0..self.messages.len() {
            let height = self.height(idx);
            if y + height >= from && y <= to {
                if count == 0 {
                    first = idx;
                    before = y;
                }
                count += 1;
                end = y + height;
            }
            y += height;
        }
        VisibleRows {
            first,
            count,
            before,
            // Nothing visible at all: every row is replaced by space.
            after: if count == 0 { y } else { y - end },
        }
    }
}

fn show_messages_area(ui: &mut Ui, state: &mut AppState, chat: &crate::state::Chat, messages: &[Message], chat_jid: &Jid, backend: &crate::backend::BackendHandle) {
    let available_height = ui.available_height();
    
    // Check if we should auto-scroll to bottom. A pending jump owns the scroll
    // position for its first frame instead.
    let should_scroll_to_bottom = state.auto_scroll_to_bottom && state.jump_target.is_none();
    // Shared cells, not plain locals: the bubble callbacks are `move` closures
    // built once per message inside the scroll loop.
    let poll_click = std::rc::Rc::new(std::cell::RefCell::new(None::<(String, String)>));
    let poll_submit = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
    // (message id, duration) — `None` duration means "unpin".
    let pin_action = std::rc::Rc::new(std::cell::RefCell::new(None::<(String, Option<u32>)>));
    // message id whose view-once payload the user asked to open
    let view_once_action = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
    let pinned_id = state
        .pinned
        .get(chat_jid)
        .filter(|pin| !pin.expired())
        .map(|pin| pin.message_id.clone());
    let me_jid = state
        .self_jid
        .as_ref()
        .map(|jid| jid.to_string())
        .unwrap_or_default();
    // Cloned once: `state` is borrowed again inside the bubble callbacks.
    let draft_owner = state.poll_draft.clone();
    let mut selected_reply: Option<usize> = None;
    let mut selected_edit: Option<String> = None;
    let mut selected_toggle: Option<String> = None;
    let selection_active = !state.selected_messages.is_empty();
    let mut message_to_delete = None;
    let mut selected_reaction: Option<(String, String)> = None;
    let mut view_reactions_id: Option<String> = None;
    let message_to_download = std::rc::Rc::new(std::cell::RefCell::new(None));
    let to_dl = message_to_download.clone();
    let open_emoji_picker_id = std::rc::Rc::new(std::cell::RefCell::new(None));
    let to_open_picker = open_emoji_picker_id.clone();
    
    // Bubbles that are new this frame fade in instead of popping; only the tail
    // of the history is tracked, which is where incoming messages land.
    const ARRIVALS_WINDOW: usize = 30;
    let arrivals_id = egui::Id::unique(("conv_arrivals", chat_jid.to_string()));
    // Only the tail is tracked, and only the tail is worth walking: passing the
    // whole history here used to allocate a pointer per message on every frame.
    let tail_start = messages.len().saturating_sub(ARRIVALS_WINDOW);
    crate::ui::animations::track_arrivals(
        ui.ctx(),
        arrivals_id,
        messages[tail_start..].iter().map(|m| m.id.as_str()),
        ARRIVALS_WINDOW,
    );
    // Read the tracked window once for this frame; the bubbles below query it
    // without touching the context again.
    let arrivals = crate::ui::animations::arrivals_snapshot(ui.ctx(), arrivals_id);
    let frame_time = ui.ctx().input(|i| i.time);
    if arrivals.is_fading(
        frame_time,
        crate::ui::animations::MEDIUM,
        ARRIVAL_STAGGER,
    ) {
        ui.ctx().request_repaint();
    }

    // A jump to a message that is not drawn cannot use `scroll_to_rect`: the
    // bubble does not exist yet. Its offset comes from the row model instead, and
    // the next frame draws it in place.
    let jump_offset = state.jump_target.as_ref().and_then(|target| {
        if target.scrolled {
            return None;
        }
        let idx = messages.iter().position(|m| m.id == target.message_id)?;
        let rows = RowModel {
            heights: &state.row_heights,
            messages,
        };
        Some((rows.top_of(idx) - available_height * 0.5).max(0.0))
    });
    if jump_offset.is_some() {
        if let Some(target) = state.jump_target.as_mut() {
            target.scrolled = true;
        }
    }

    let mut scroll_area = ScrollArea::vertical()
        .animated(true)
        .auto_shrink([false, false])
        .stick_to_bottom(should_scroll_to_bottom)
        .max_height(available_height);
    if let Some(offset) = jump_offset {
        scroll_area = scroll_area.vertical_scroll_offset(offset);
    }
    let output = scroll_area.show_viewport(ui, |ui, _viewport| {
            ui.add_space(8.0);
            
            // On-demand message history button (Peer Data Operation to primary phone)
            ui.vertical_centered(|ui| {
                if ui.button(RichText::new("⟳  Load older messages from phone").font(FontId::proportional(12.0))).clicked() {
                    let _ = backend.send_command(Command::RequestHistorySync {
                        chat_jid: Some(chat_jid.clone()),
                        count: 50,
                    });
                }
            });
            
            ui.add_space(12.0);
            
            if messages.is_empty() {
                show_empty_chat(ui, chat, backend);
            } else {
                // Only the rows around the viewport are laid out; the rest is two
                // spacers. Rows are skipped before any text is measured, which is
                // where the whole cost of a long history used to be.
                // The model is borrowed only long enough to pick the window: the
                // loop below writes the measured heights back into the cache.
                // The clip rect, not the scroll area's `viewport`: the cursor below
                // is in screen coordinates and the two have to agree, otherwise a
                // scrolled-down history draws nothing at all.
                let window = {
                    let rows = RowModel {
                        heights: &state.row_heights,
                        messages,
                    };
                    rows.visible(ui.cursor().min.y, ui.clip_rect(), ROW_OVERSCAN)
                };
                ui.add_space(window.before);

                // Cloning the full Message (media bytes included) for every bubble just
                // to support "reply" was wasteful; grab the index instead.
                for (idx, msg) in messages.iter().enumerate() {
                    if !window.contains(idx) {
                        continue;
                    }
                    let row_top = ui.min_rect().max.y;
                    // Show date separator
                    let msg_date = msg.timestamp.date_naive();
                    let show_date = idx == 0
                        || messages[idx - 1].timestamp.date_naive() != msg_date;
                    
                    if show_date {
                        show_date_separator(ui, msg_date);
                    }
                    
                    let is_own = msg.from_me;
                    let show_sender = chat.is_group && !is_own && idx > 0 
                        && messages.get(idx - 1).map(|m| m.sender_jid != msg.sender_jid).unwrap_or(true);
                    
                    let sender_name = if show_sender {
                        state.contacts.get(&msg.sender_jid)
                            .and_then(|c| c.push_name.as_deref())
                            .map(Cow::Borrowed)
                            .or_else(|| {
                                let formatted = format_jid(&msg.sender_jid);
                                Some(Cow::Owned(formatted))
                            })
                    } else {
                        None
                    };
                    
                    let sender_name_str = sender_name.as_deref().unwrap_or("");

                    let jump_here = state
                        .jump_target
                        .as_ref()
                        .is_some_and(|target| target.message_id == msg.id);
                    // The flash fades in and back out on its own, so the tint is
                    // an intensity rather than a flag.
                    let jump_flash = if jump_here {
                        state
                            .jump_target
                            .as_ref()
                            .map_or(0.0, |target| target.intensity())
                    } else {
                        0.0
                    };
                    let arrival = arrivals.alpha(
                        frame_time,
                        &msg.id,
                        crate::ui::animations::MEDIUM,
                        ARRIVAL_STAGGER,
                    );
                    let draft_for_msg = if draft_owner
                        .as_ref()
                        .is_some_and(|(id, _)| id == &msg.id)
                    {
                        draft_owner
                            .as_ref()
                            .map(|(_, names)| names.clone())
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    // One handle pair per bubble: the callbacks are `move`.
                    let poll_click_handle = poll_click.clone();
                    let poll_submit_handle = poll_submit.clone();
                    let pin_handle = pin_action.clone();
                    let view_once_handle = view_once_action.clone();
                    let is_pinned = pinned_id.as_deref() == Some(msg.id.as_str());

                    MessageBubble::new(msg, is_own)
                        .show_sender(sender_name_str)
                        .on_reply(|_id| {
                            selected_reply = Some(idx);
                        })
                        .on_edit(|id| {
                            selected_edit = Some(id.to_string());
                        })
                        .selection_mode(selection_active)
                        .selected(selection_active && state.selected_messages.contains(&msg.id))
                        .on_select(|id| {
                            selected_toggle = Some(id.to_string());
                        })
                        .on_delete(|id| {
                            message_to_delete = Some(id.to_string());
                        })
                        .on_download({
                            let to_dl = to_dl.clone();
                            move |id| {
                                *to_dl.borrow_mut() = Some(id.to_string());
                            }
                        })
                        .on_react(|id, emoji| {
                            selected_reaction = Some((id.to_string(), emoji.to_string()));
                        })
                        .on_view_reactions(|id| {
                            view_reactions_id = Some(id.to_string());
                        })
                        .on_open_emoji_picker({
                            let to_open_picker = to_open_picker.clone();
                            move |id| {
                                *to_open_picker.borrow_mut() = Some(id.to_string());
                            }
                        })
                        .is_pinned(is_pinned)
                        .on_pin({
                            let msg_id = msg.id.clone();
                            move |_id: &str, secs: Option<u32>| {
                                *pin_handle.borrow_mut() = Some((msg_id.clone(), secs));
                            }
                        })
                        .on_open_view_once({
                            let msg_id = msg.id.clone();
                            move || {
                                *view_once_handle.borrow_mut() = Some(msg_id.clone());
                            }
                        })
                        .highlight(jump_flash)
                        .entrance(arrival)
                        .me(me_jid.as_str())
                        .poll_draft(draft_for_msg)
                        .on_poll_option({
                            let msg_id = msg.id.clone();
                            move |option: String| {
                                *poll_click_handle.borrow_mut() = Some((msg_id.clone(), option));
                            }
                        })
                        .on_poll_submit({
                            let msg_id = msg.id.clone();
                            move || {
                                *poll_submit_handle.borrow_mut() = Some(msg_id.clone());
                            }
                        })
                        .ui(ui);

                    // The scroll position was set from the row model before the
                    // list was drawn; the tint only has to keep frames coming
                    // while it fades.
                    if jump_here {
                        ui.ctx().request_repaint();
                    }

                    ui.add_space(2.0);

                    // Remember what this row actually took, so the next frame can
                    // place the rows above and below it without laying them out.
                    let drawn = ui.min_rect().max.y - row_top;
                    let bubble_height =
                        drawn - if show_date { DATE_SEPARATOR_HEIGHT } else { 0.0 };
                    state
                        .row_heights
                        .insert(msg.id.clone(), bubble_height.max(20.0));
                }
                ui.add_space(window.after);
                if state.row_heights.len() > ROW_HEIGHT_CACHE_LIMIT {
                    state.row_heights.clear();
                }
            }
            
            // Bottom padding for stick_to_bottom
            ui.add_space(24.0);

            // A request from the button below the pane: the click lands after the
            // history was laid out, so the actual jump happens on the next frame,
            // when the whole content rect is known.
            if state.scroll_to_bottom_requested {
                state.scroll_to_bottom_requested = false;
                // Asking the scroll area itself (instead of writing an offset)
                // keeps its animation and lets it re-enter sticky mode, so new
                // messages keep following the bottom afterwards.
                ui.scroll_to_rect(
                    Rect::from_min_size(ui.min_rect().left_bottom(), egui::Vec2::ZERO),
                    Some(Align::BOTTOM),
                );
            }
        });

    // Reading older messages: offer the way back to the tail. The rule lives in
    // a pure function so it can be tested without a window.
    let distance_from_viewport_top = output.state.clamped_offset().y;
    if should_show_jump_to_bottom(
        output.content_size.y,
        output.inner_rect.height(),
        distance_from_viewport_top,
    ) {
        let anchor = output.inner_rect.right_bottom();
        let mut clicked = false;
        egui::Area::new(egui::Id::unique(("jump_to_bottom", chat_jid.to_string())))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(anchor.x - 54.0, anchor.y - 54.0))
            .show(ui.ctx(), |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    egui::Vec2::splat(JUMP_TO_BOTTOM_SIZE),
                    egui::Sense::click(),
                );
                let fill = if response.hovered() {
                    Color32::from_rgb(0x06, 0xCF, 0x9C)
                } else {
                    Color32::from_rgb(0x00, 0xA8, 0x84)
                };
                let painter = ui.painter();
                // A dark rim keeps the circle readable over bright photo bubbles.
                painter.circle_filled(
                    rect.center(),
                    JUMP_TO_BOTTOM_SIZE / 2.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 70),
                );
                painter.circle_filled(rect.center(), JUMP_TO_BOTTOM_SIZE / 2.0 - 2.0, fill);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "⬇",
                    FontId::proportional(17.0),
                    Color32::WHITE,
                );
                clicked = response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(crate::tr!("Skočit na nejnovější zprávy"))
                    .clicked();
            });
        if clicked {
            state.scroll_to_bottom_requested = true;
            ui.ctx().request_repaint();
        }
    }
        
    if let Some(id) = selected_toggle {
        state.toggle_selected(&id);
    }

    if let Some(idx) = selected_reply {
        state.replying_to = messages.get(idx).cloned();
    }
    // Entering edit mode loads the original body into the composer and drops any
    // pending reply: an edit rewrites one stanza, it cannot also quote another.
    if let Some(id) = selected_edit {
        if let Some(msg) = messages.iter().find(|m| m.id == id) {
            state.composer_text = msg.text.clone();
            state.replying_to = None;
            state.editing_message = Some(id);
        }
    }
    if let Some(id) = message_to_delete {
        let _ = backend.send_command(Command::DeleteMessageForMe {
            chat_jid: chat_jid.clone(),
            message_id: id,
        });
    }
    if let Some((id, emoji)) = selected_reaction {
        let _ = backend.send_command(Command::SendReaction {
            chat_jid: chat_jid.clone(),
            message_id: id,
            emoji,
        });
    }
    if let Some(id) = view_reactions_id {
        state.reaction_detail_modal = Some((chat_jid.clone(), id));
    }
    if let Some(id) = open_emoji_picker_id.borrow().clone() {
        state.reaction_picker_target = Some(id);
        state.show_emoji_picker = true;
    }
    let to_download = message_to_download.borrow().clone();
    if let Some(id) = to_download {
        let _ = backend.send_command(Command::DownloadMedia {
            chat_jid: chat_jid.clone(),
            message_id: id,
        });
    }

    if let Some(id) = view_once_action.borrow().clone() {
        let _ = backend.send_command(Command::OpenViewOnce {
            chat_jid: chat_jid.clone(),
            message_id: id.clone(),
        });
        state.view_once_open = Some((chat_jid.clone(), id));
    }

    let pin_request = pin_action.borrow().clone();
    if let Some((message_id, secs)) = pin_request {
        let _ = backend.send_command(Command::PinMessage {
            chat_jid: chat_jid.clone(),
            message_id,
            duration_secs: secs,
        });
    }

    let clicked_option = poll_click.borrow().clone();
    let submitted_poll = poll_submit.borrow().clone();
    if let Some((message_id, option)) = clicked_option {
        poll_option_clicked(state, messages, chat_jid, &message_id, &option, backend);
    }
    if let Some(message_id) = submitted_poll {
        poll_submit_clicked(state, messages, chat_jid, &message_id, backend);
    }

    // The highlight is a short flash, not a mode: drop it once it has served
    // its purpose so auto-scroll takes over again.
    if state.jump_target.as_ref().is_some_and(|target| target.expired()) {
        state.jump_target = None;
    }
}

/// Click on one poll option. Single-choice polls replace the vote at once,
/// the way WhatsApp Web does; multi-choice polls only move a tick and wait for
/// the "Hlasovat" button.
fn poll_option_clicked(
    state: &mut AppState,
    messages: &[Message],
    chat_jid: &Jid,
    message_id: &str,
    option: &str,
    backend: &crate::backend::BackendHandle,
) {
    let Some(poll) = messages
        .iter()
        .find(|m| m.id == message_id)
        .and_then(|m| m.poll.clone())
    else {
        return;
    };
    let me = state
        .self_jid
        .as_ref()
        .map(|jid| jid.to_string())
        .unwrap_or_default();

    if !poll.is_multi() {
        // Tapping the ticked option clears the vote, like WhatsApp Web.
        let selection = if poll.my_vote(&me).iter().any(|picked| picked == option) {
            Vec::new()
        } else {
            vec![option.to_string()]
        };
        state.poll_draft = None;
        send_vote(chat_jid, &poll, message_id, selection, backend);
        return;
    }

    let limit = if poll.selectable_count == 0 {
        poll.options.len()
    } else {
        poll.selectable_count as usize
    };
    let draft = state
        .poll_draft
        .get_or_insert_with(|| (message_id.to_string(), Vec::new()));
    if draft.0 != message_id {
        *draft = (message_id.to_string(), Vec::new());
    }
    let picked = &mut draft.1;
    if let Some(pos) = picked.iter().position(|name| name == option) {
        picked.remove(pos);
    } else if picked.len() < limit {
        picked.push(option.to_string());
    }
}

/// Commit the ticks of a multi-choice poll.
fn poll_submit_clicked(
    state: &mut AppState,
    messages: &[Message],
    chat_jid: &Jid,
    message_id: &str,
    backend: &crate::backend::BackendHandle,
) {
    let Some(poll) = messages
        .iter()
        .find(|m| m.id == message_id)
        .and_then(|m| m.poll.clone())
    else {
        return;
    };
    let Some((owner, selection)) = state.poll_draft.take() else {
        return;
    };
    if owner != message_id || selection.is_empty() {
        return;
    }
    send_vote(chat_jid, &poll, message_id, selection, backend);
}

/// Hand a vote to the backend. Polls received without a secret cannot be voted
/// on at all: the encryption key is derived from it.
fn send_vote(
    chat_jid: &Jid,
    poll: &crate::state::PollData,
    message_id: &str,
    selection: Vec<String>,
    backend: &crate::backend::BackendHandle,
) {
    if poll.secret.is_empty() {
        return;
    }
    let _ = backend.send_command(Command::VotePoll {
        chat_jid: chat_jid.clone(),
        message_id: message_id.to_string(),
        poll_creator: poll.creator.clone(),
        message_secret: poll.secret.clone(),
        option_names: selection,
    });
}

fn show_date_separator(ui: &mut Ui, date: chrono::NaiveDate) {
    let today = chrono::Utc::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);
    
    let label = if date == today {
        "Today".to_string()
    } else if date == yesterday {
        "Yesterday".to_string()
    } else {
        date.format("%A, %B %d, %Y").to_string()
    };
    
    ui.vertical_centered(|ui| {
        Frame::new()
            .fill(Color32::from_rgba_premultiplied(0, 0, 0, 80))
            .inner_margin(Margin::symmetric(16, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.label(RichText::new(label).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
            });
    });
    ui.add_space(12.0);
}

fn show_empty_chat(ui: &mut Ui, chat: &crate::state::Chat, backend: &crate::backend::BackendHandle) {
    ui.centered_and_justified(|ui| {
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("💬").font(FontId::proportional(48.0)));
            ui.add_space(16.0);
            ui.label(RichText::new(format!("Start messaging {}", chat.name)).font(FontId::proportional(18.0)).strong().color(Color32::WHITE));
            ui.add_space(8.0);
            ui.label(RichText::new("Messages and calls are end-to-end encrypted").font(FontId::proportional(13.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
            ui.add_space(4.0);
            ui.label(RichText::new("No one outside of this chat can read or listen to them").font(FontId::proportional(13.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
            ui.add_space(16.0);
            if ui.add(
                egui::Button::new(RichText::new("⟳  Načíst zprávy z telefonu").font(FontId::proportional(13.0)).color(Color32::WHITE))
                    .fill(Color32::from_rgb(0x00, 0xA8, 0x84))
                    .corner_radius(egui::CornerRadius::same(6))
            ).clicked() {
                let _ = backend.send_command(Command::RequestHistorySync {
                    chat_jid: Some(chat.jid.clone()),
                    count: 50,
                });
            }
        });
    });
}

/// Recording bar shown in place of the composer while a voice note is captured.
fn show_recorder_bar(
    ui: &mut Ui,
    state: &mut AppState,
    chat_jid: &Jid,
    backend: &crate::backend::BackendHandle,
) {
    let elapsed = std::time::Duration::from_millis(crate::recorder::elapsed_ms());
    Frame::new()
        .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Blink the dot twice a second so it reads as "live".
                let on = (elapsed.as_millis() / 500) % 2 == 0;
                let dot = if on {
                    Color32::from_rgb(0xE6, 0x4A, 0x4A)
                } else {
                    Color32::from_rgb(0x7A, 0x2B, 0x2B)
                };
                ui.label(RichText::new("●").font(FontId::proportional(16.0)).color(dot));
                ui.label(
                    RichText::new(crate::audio::format_duration(elapsed))
                        .font(FontId::proportional(15.0))
                        .color(Color32::WHITE),
                );
                ui.label(
                    RichText::new("Nahrávání…")
                        .font(FontId::proportional(12.0))
                        .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let send = ui.add(
                        egui::Button::new(
                            RichText::new("➤")
                                .font(FontId::proportional(22.0))
                                .color(Color32::WHITE),
                        )
                        .frame(false)
                        .fill(Color32::from_rgb(0x00, 0x80, 0x69)),
                    );
                    if send.on_hover_text("Odeslat hlasovou zprávu").clicked() {
                        match crate::recorder::finish() {
                            Ok(recording) => {
                                state.recorder_error = None;
                                let _ = backend.send_command(Command::SendVoiceNote {
                                    chat_jid: chat_jid.clone(),
                                    file_path: recording.path,
                                    duration_seconds: (recording.duration_ms / 1000).max(1) as u32,
                                    waveform: recording.waveform,
                                });
                            }
                            Err(message) => state.recorder_error = Some(message),
                        }
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("✕").font(FontId::proportional(18.0)),
                            )
                            .frame(false),
                        )
                        .on_hover_text("Zahodit nahrávku")
                        .clicked()
                    {
                        crate::recorder::cancel();
                    }
                });
            });
        });
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(200));
}

fn show_composer(ui: &mut Ui, state: &mut AppState, chat_jid: &Jid, backend: &crate::backend::BackendHandle) {
    ui.add_space(4.0);

    // Pending scheduled messages for this chat indicator banner
    let pending_count = state.scheduled_messages.iter()
        .filter(|m| m.chat_jid == *chat_jid && m.status == crate::state::ScheduledStatus::Pending)
        .count();
    if pending_count > 0 {
        Frame::new()
            .fill(Color32::from_rgba_premultiplied(0, 168, 132, 25))
            .inner_margin(Margin::symmetric(14, 4))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("🕒 {} scheduled message(s) for this chat", pending_count))
                        .font(FontId::proportional(11.0)).color(Color32::from_rgb(0x25, 0xD3, 0x66)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("View").font(FontId::proportional(11.0))).clicked() {
                            state.show_scheduled_manager = true;
                        }
                    });
                });
            });
        ui.add_space(2.0);
    }
    
    // Edit banner: an active edit owns the composer until it is sent or cancelled.
    let mut cancel_edit = false;
    if state.editing_message.is_some() {
        Frame::new()
            .fill(Color32::from_rgb(0x1A, 0x26, 0x2C))
            .inner_margin(Margin::symmetric(14, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("✎").font(FontId::proportional(14.0)).color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                    ui.label(RichText::new("Upravit zprávu").font(FontId::proportional(12.0)).strong().color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            cancel_edit = true;
                        }
                    });
                });
            });
    }
    if cancel_edit {
        state.cancel_edit();
    }

    // Replying preview bar if replying to a message
    let mut cancel_reply = false;
    if let Some(reply_msg) = &state.replying_to {
        let snippet = crate::util::truncate(&reply_msg.text, 50);
        let sender_name = if reply_msg.from_me {
            "You".to_string()
        } else {
            state.contacts.get(&reply_msg.sender_jid)
                .and_then(|c| c.push_name.clone())
                .unwrap_or_else(|| crate::util::format_jid(&reply_msg.sender_jid))
        };
        
        Frame::new()
            .fill(Color32::from_rgb(0x1A, 0x26, 0x2C))
            .inner_margin(Margin::symmetric(14, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("↳").font(FontId::proportional(14.0)).color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(sender_name).font(FontId::proportional(11.0)).strong().color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                        ui.label(RichText::new(snippet).font(FontId::proportional(12.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            cancel_reply = true;
                        }
                    });
                });
            });
    }
    if cancel_reply {
        state.replying_to = None;
    }
    
    // Check for drag-and-dropped files
    let dropped_path = ui.input(|i| {
        i.raw.dropped_files.first()
            .map(|f| f.path().to_path_buf())
            .filter(|p| !p.as_os_str().is_empty())
    });
    if let Some(path) = dropped_path {
        state.pending_attachment = Some(path);
    }

    // Attachment preview banner if a file is attached
    let mut cancel_attachment = false;
    if let Some(path) = &state.pending_attachment {
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("Attachment");
        let file_size_str = std::fs::metadata(path)
            .map(|m| crate::media::format_file_size(m.len()))
            .unwrap_or_default();
        
        Frame::new()
            .fill(Color32::from_rgb(0x1A, 0x26, 0x2C))
            .inner_margin(Margin::symmetric(14, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📎").font(FontId::proportional(14.0)).color(Color32::from_rgb(0x00, 0xA8, 0x84)));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(file_name).font(FontId::proportional(12.0)).strong().color(Color32::WHITE));
                        if !file_size_str.is_empty() {
                            ui.label(RichText::new(file_size_str).font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            cancel_attachment = true;
                        }
                    });
                });
            });
        ui.add_space(2.0);
    }
    if cancel_attachment {
        state.pending_attachment = None;
    }

    if let Some(message) = state.recorder_error.clone() {
        Frame::new()
            .fill(Color32::from_rgb(0x3A, 0x1E, 0x22))
            .inner_margin(Margin::symmetric(14, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("⚠ Mikrofon: {message}"))
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(0xF2, 0x8B, 0x82)),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            state.recorder_error = None;
                        }
                    });
                });
            });
        ui.add_space(2.0);
    }

    if let Some(message) = state.clipboard_error.clone() {
        Frame::new()
            .fill(Color32::from_rgb(0x3A, 0x1E, 0x22))
            .inner_margin(Margin::symmetric(14, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("⚠ Schránka: {message}"))
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(0xF2, 0x8B, 0x82)),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            state.clipboard_error = None;
                        }
                    });
                });
            });
        ui.add_space(2.0);
    }

    // While a take is running the composer row is replaced by the recorder bar,
    // so a stray Enter cannot send text halfway through a voice note.
    if crate::recorder::is_recording() {
        show_recorder_bar(ui, state, chat_jid, backend);
        return;
    }

    // @zmínky: while the user types a name after `@`, offer matching members.
    let mention_people_list = mention_people(state, chat_jid);
    let suggestions: Vec<(Jid, String)> =
        crate::state::mention_suggestions(&state.composer_text, &mention_people_list)
            .into_iter()
            .cloned()
            .collect();
    if !suggestions.is_empty() {
        let mut picked: Option<String> = None;
        Frame::new()
            .fill(Color32::from_rgb(0x1A, 0x26, 0x2C))
            .inner_margin(Margin::symmetric(12, 6))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("@")
                            .font(FontId::proportional(12.0))
                            .color(crate::theme::ACCENT),
                    );
                    for (_, name) in &suggestions {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(name.clone())
                                        .font(FontId::proportional(12.0))
                                        .color(Color32::WHITE),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            picked = Some(name.clone());
                        }
                    }
                });
            });
        ui.add_space(2.0);
        if let Some(name) = picked {
            crate::state::apply_mention(&mut state.composer_text, &name);
        }
    }

    Frame::new()
        .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Attach file button
                let attach_btn = ui.add(
                    egui::Button::new(RichText::new("📎").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(Color32::TRANSPARENT)
                ).on_hover_text("Připojit soubor, foto nebo video");

                if attach_btn.clicked() {
                    if let Some(path) = crate::media::pick_file() {
                        state.pending_attachment = Some(path);
                        state.send_view_once = false;
                    }
                }

                // View-once toggle only makes sense once something is attached.
                if state.pending_attachment.is_some() {
                    let vo_btn = ui.add(
                        egui::Button::new(
                            RichText::new("1")
                                .font(FontId::proportional(15.0))
                                .color(if state.send_view_once { Color32::BLACK } else { crate::theme::ACCENT })
                        )
                        .frame(true)
                        .fill(if state.send_view_once { crate::theme::ACCENT } else { Color32::TRANSPARENT })
                    ).on_hover_text(if state.send_view_once {
                        "Zobrazit jednou: příjemce uvidí médium pouze jednou"
                    } else {
                        "Odeslat jako zobrazení jednou"
                    });
                    if vo_btn.clicked() {
                        state.send_view_once = !state.send_view_once;
                    }
                }

                // Poll button
                let poll_btn = ui.add(
                    egui::Button::new(RichText::new("📊").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(Color32::TRANSPARENT)
                ).on_hover_text("Vytvořit anketu");

                if poll_btn.clicked() {
                    state.new_poll = Some(crate::state::NewPollState::new(chat_jid.clone()));
                }

                // Sticker picker button
                let sticker_btn = ui.add(
                    egui::Button::new(RichText::new("🎨").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(Color32::TRANSPARENT)
                ).on_hover_text("Poslat samolepku");

                if sticker_btn.clicked() {
                    state.show_sticker_picker = !state.show_sticker_picker;
                }

                // Emoji picker button
                let emoji_btn = ui.add(
                    egui::Button::new(RichText::new("😊").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(if state.show_emoji_picker && state.reaction_picker_target.is_none() { Color32::from_rgb(0x2A, 0x39, 0x42) } else { Color32::TRANSPARENT })
                ).on_hover_text("Vybrat emoji");

                if emoji_btn.clicked() {
                    state.show_emoji_picker = !state.show_emoji_picker;
                    state.reaction_picker_target = None;
                }

                // Text input - takes most space
                let hint = if state.editing_message.is_some() {
                    "Upravit zprávu..."
                } else if state.pending_attachment.is_some() {
                    "Přidat popisek..."
                } else {
                    "Napište zprávu..."
                };
                let input_width = (ui.available_width() - 115.0).max(50.0);
                let response = ui.add(
                    TextEdit::multiline(&mut state.composer_text)
                        .hint_text(hint)
                        .desired_rows(1)
                        .desired_width(input_width)
                        .font(FontId::proportional(15.0))
                        .margin(egui::vec2(12.0, 10.0))
                );
                
                // Ctrl+V with a picture on the clipboard becomes an attachment;
                // plain text still pastes normally because we only look for images.
                if response.has_focus()
                    && ui.input(|i| i.modifiers.command && i.key_pressed(Key::V))
                {
                    match crate::clipboard::save_clipboard_image() {
                        Ok(Some(path)) => {
                            state.pending_attachment = Some(path);
                            state.clipboard_error = None;
                        }
                        Ok(None) => {}
                        Err(err) => state.clipboard_error = Some(err),
                    }
                }

                // Handle Enter to send, Shift+Enter for newline
                let enter_pressed = response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);
                let can_send = state.pending_attachment.is_some() || !state.composer_text.trim().is_empty();

                // Schedule message button
                let schedule_btn = ui.add(
                    egui::Button::new(RichText::new("🕒").font(FontId::proportional(18.0)))
                        .frame(false)
                        .fill(Color32::TRANSPARENT)
                ).on_hover_text("Naplánovat odeslání zprávy");

                if schedule_btn.clicked() {
                    state.schedule_target_chat = Some(chat_jid.clone());
                    state.schedule_text = state.composer_text.clone();
                    state.show_schedule_dialog = true;
                }
                
                // WhatsApp swaps the send arrow for a microphone while the box
                // is empty, so recording never needs a second control.
                let send_btn = if can_send {
                    ui.add(
                        egui::Button::new(RichText::new("➤").font(FontId::proportional(22.0)).color(Color32::WHITE))
                            .frame(false)
                            .fill(Color32::from_rgb(0x00, 0x80, 0x69))
                    ).on_hover_text("Send")
                } else {
                    ui.add(
                        egui::Button::new(RichText::new("🎤").font(FontId::proportional(20.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)))
                            .frame(false)
                            .fill(Color32::TRANSPARENT)
                    ).on_hover_text("Nahrát hlasovou zprávu")
                };

                if !can_send && send_btn.clicked() {
                    match crate::recorder::start() {
                        Ok(()) => ui.ctx().request_repaint(),
                        Err(message) => state.recorder_error = Some(message),
                    }
                }
                
                if (enter_pressed || send_btn.clicked()) && can_send {
                    let text = state.composer_text.trim().to_string();

                    // An active edit rewrites the original stanza instead of
                    // sending a second one, so no reply/attachment is involved.
                    if let Some(message_id) = state.editing_message.clone() {
                        if !text.is_empty() {
                            let _ = backend.send_command(Command::EditText {
                                to: chat_jid.clone(),
                                message_id,
                                text,
                            });
                        }
                        state.editing_message = None;
                        state.composer_text.clear();
                        return;
                    }

                    let reply_to = state.replying_to.take().map(|m| m.id);
                    
                    if let Some(file_path) = state.pending_attachment.take() {
                        let caption = if text.is_empty() { None } else { Some(text) };
                        let view_once = state.send_view_once;
                        state.send_view_once = false;
                        let _ = backend.send_command(Command::SendMedia {
                            chat_jid: chat_jid.clone(),
                            file_path,
                            caption,
                            reply_to,
                            view_once,
                        });
                    } else {
                        let people = mention_people(state, chat_jid);
                        let mentioned_jid: Vec<String> =
                            crate::state::collect_mentions(&text, &people)
                                .into_iter()
                                .map(|jid| jid.to_string())
                                .collect();
                        let _ = backend.send_command(Command::SendText {
                            to: chat_jid.clone(),
                            text,
                            reply_to,
                            mentioned_jid,
                        });
                    }
                    state.composer_text.clear();
                    // The text is gone, so must be its remembered draft.
                    state.drafts.remove(chat_jid);
                }
            });
        });
    
    // Emoji Picker Window / Popup
    if state.show_emoji_picker {
        let is_reaction = state.reaction_picker_target.is_some();
        let title = if is_reaction { "❤️  Vybrat reakci" } else { "😊  Emoji" };
        let mut open = true;
        let mut picked_emoji = None;

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::vec2(340.0, 360.0))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -80.0))
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                crate::ui::emoji::show_emoji_picker(ui, state, |emoji| {
                    picked_emoji = Some(emoji.to_string());
                });
            });

        if !open {
            state.show_emoji_picker = false;
            state.reaction_picker_target = None;
        }

        if let Some(emoji) = picked_emoji {
            if let Some(target_msg_id) = state.reaction_picker_target.take() {
                let _ = backend.send_command(Command::SendReaction {
                    chat_jid: chat_jid.clone(),
                    message_id: target_msg_id,
                    emoji,
                });
            } else {
                state.composer_text.push_str(&emoji);
            }
            state.show_emoji_picker = false;
        }
    }

    ui.add_space(6.0);
}

/// Modal dialog showing details of who reacted to a message
pub fn show_reaction_detail_modal(ctx: &Context, state: &SharedState, backend: &crate::backend::BackendHandle) {
    let (chat_jid, msg_id, reactions) = {
        let s = state.read();
        if let Some((ref chat_jid, ref msg_id)) = s.reaction_detail_modal {
            let canon = s.canonical_jid(chat_jid);
            if let Some(msgs) = s.get_chat_messages(&canon) {
                if let Some(msg) = msgs.iter().find(|m| &m.id == msg_id) {
                    (chat_jid.clone(), msg_id.clone(), msg.reactions.clone())
                } else {
                    return;
                }
            } else {
                return;
            }
        } else {
            return;
        }
    };

    let mut is_open = true;
    let mut should_close = false;
    let mut reaction_to_remove = false;

    egui::Window::new("❤️  Reakce na zprávu")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::vec2(360.0, 300.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut is_open)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            if reactions.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new("Tato zpráva zatím nemá žádné reakce.").color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                });
            } else {
                let self_jid = state.read().self_jid.clone();
                
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Celkem reakcí: {}", reactions.len())).font(FontId::proportional(13.0)).strong());
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for r in &reactions {
                        let is_mine = self_jid.as_ref().map_or(false, |sj| sj.user == r.sender.user);
                        let name = {
                            let s = state.read();
                            s.resolve_display_name(&r.sender).unwrap_or_else(|| {
                                if is_mine { "Vy".to_string() } else { crate::util::format_jid(&r.sender) }
                            })
                        };
                        let name_display = if is_mine && name != "Vy" {
                            format!("{} (Vy)", name)
                        } else {
                            name
                        };

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&r.emoji).font(crate::theme::message_font(22.0)));
                            ui.add_space(8.0);
                            ui.label(RichText::new(name_display).font(FontId::proportional(14.0)).color(Color32::from_rgb(0xE9, 0xED, 0xEF)));
                            if is_mine {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button(RichText::new("Odebrat").color(Color32::from_rgb(0xFF, 0x6B, 0x6B))).clicked() {
                                        reaction_to_remove = true;
                                    }
                                });
                            }
                        });
                        ui.add_space(6.0);
                    }
                });
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Zavřít").clicked() {
                    should_close = true;
                }
            });
        });

    if reaction_to_remove {
        let _ = backend.send_command(Command::SendReaction {
            chat_jid: chat_jid.clone(),
            message_id: msg_id.clone(),
            emoji: String::new(),
        });
        should_close = true;
    }

    if should_close || !is_open {
        state.write().reaction_detail_modal = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn base(id: &str, text: &str) -> Message {
        let jid: Jid = "123@s.whatsapp.net".parse().unwrap();
        Message::new_text(
            id.to_string(),
            jid.clone(),
            jid,
            text.to_string(),
            chrono::Utc::now(),
            false,
            crate::state::MessageStatus::Delivered,
            None,
            None,
        )
    }

    #[test]
    fn pinned_preview_prefers_text_over_media_kind() {
        let mut msg = base("a", "  nazdar  ");
        msg.media_type = Some(crate::state::MediaType::Image);
        assert_eq!(pinned_preview(&msg), "nazdar");
    }

    fn history(count: usize, height: Option<f32>) -> (Vec<Message>, std::collections::HashMap<String, f32>) {
        let messages: Vec<Message> = (0..count)
            .map(|i| {
                // One message per day keeps the date separators out of the way
                // unless a test wants them.
                let mut msg = base(&format!("m{i}"), "text");
                msg.timestamp =
                    chrono::Utc::now() - chrono::Duration::days((count - i) as i64);
                msg
            })
            .collect();
        let heights = match height {
            Some(h) => messages
                .iter()
                .map(|m| (m.id.clone(), h))
                .collect::<std::collections::HashMap<_, _>>(),
            None => std::collections::HashMap::new(),
        };
        (messages, heights)
    }

    fn viewport(top: f32, height: f32) -> Rect {
        Rect::from_min_size(egui::pos2(0.0, top), egui::vec2(600.0, height))
    }

    /// The whole point of the change: a long history must not be laid out. The
    /// window is measured in rows, so this fails loudly if the range arithmetic
    /// ever starts returning the full history again.
    #[test]
    fn a_long_history_only_draws_the_rows_around_the_viewport() {
        let (messages, heights) = history(2000, Some(60.0));
        let model = RowModel {
            heights: &heights,
            messages: &messages,
        };
        for scroll_top in [0.0, 30_000.0, 2000.0 * 60.0 - 700.0] {
            let window = model.visible(0.0, viewport(scroll_top, 700.0), 0.0);
            assert!(
                window.count <= 13,
                "při scrollu {scroll_top} se kreslí {} řádků",
                window.count
            );
        }
    }

    /// Spacers have to add up to the same height as the rows they replace,
    /// otherwise the scrollbar lies and `stick_to_bottom` lands somewhere else.
    #[test]
    fn spacers_keep_the_content_height() {
        let (messages, heights) = history(500, Some(57.5));
        let model = RowModel {
            heights: &heights,
            messages: &messages,
        };
        let window = model.visible(0.0, viewport(2_000.0, 700.0), ROW_OVERSCAN);
        let drawn: f32 = (window.first..window.first + window.count)
            .map(|idx| model.height(idx))
            .sum();
        let total: f32 = (0..messages.len()).map(|idx| model.height(idx)).sum();
        assert_eq!(window.before + drawn + window.after, total);
        assert!(window.count > 0 && window.count < messages.len());
    }

    /// A scroll offset pointing past the end of the history must not blank the
    /// panel: this is what a shrink after measuring looks like.
    #[test]
    fn a_stale_scroll_offset_still_draws_rows() {
        let (messages, heights) = history(60, Some(60.0));
        let model = RowModel {
            heights: &heights,
            messages: &messages,
        };
        let window = model.visible(60.0, viewport(500_000.0, 700.0), ROW_OVERSCAN);
        assert!(window.count > 0, "panel zůstal prázdný");
        assert_eq!(window.after, 0.0, "pod oknem už nic není");
    }

    /// Rows nobody has drawn yet use the fallback, so the first frame of a chat
    /// still shows something instead of an empty scroll area.
    #[test]
    fn undrawn_rows_use_the_fallback_height() {
        let (messages, heights) = history(40, None);
        let model = RowModel {
            heights: &heights,
            messages: &messages,
        };
        assert_eq!(model.height(7), ROW_HEIGHT_FALLBACK + DATE_SEPARATOR_HEIGHT);
        assert_eq!(
            model.top_of(10),
            10.0 * (ROW_HEIGHT_FALLBACK + DATE_SEPARATOR_HEIGHT)
        );
    }

    /// A short history is drawn whole — virtualization must not hide the few
    /// messages a new chat actually has.
    #[test]
    fn a_short_history_is_drawn_whole() {
        let (messages, heights) = history(5, Some(60.0));
        let model = RowModel {
            heights: &heights,
            messages: &messages,
        };
        let window = model.visible(0.0, viewport(0.0, 700.0), 0.0);
        assert_eq!(window.first, 0);
        assert_eq!(window.count, 5);
        assert_eq!(window.before, 0.0);
        assert_eq!(window.after, 0.0);
    }

    /// Date separators are part of a row's height, and only the first message of
    /// a day has one — a row model that forgets this shifts every row below it.
    #[test]
    fn only_the_first_message_of_a_day_carries_a_separator() {
        let (mut messages, heights) = history(6, Some(60.0));
        for msg in messages.iter_mut() {
            msg.timestamp = chrono::Utc::now();
        }
        // A second day starts at message 3 and keeps going to the end; going
        // back to an earlier date would legitimately start another separator.
        let tomorrow = chrono::Utc::now() + chrono::Duration::days(1);
        for msg in messages.iter_mut().skip(3) {
            msg.timestamp = tomorrow;
        }
        let model = RowModel {
            heights: &heights,
            messages: &messages,
        };
        assert!(model.has_separator(0), "první zpráva má datum");
        assert!(!model.has_separator(1));
        assert!(model.has_separator(3), "nový den má datum");
        assert!(!model.has_separator(4));
        assert!(!model.has_separator(5));
        assert_eq!(model.height(3) - model.height(4), DATE_SEPARATOR_HEIGHT);
    }

    /// Headless cena jednoho snímku panelu zpráv podle délky historie.
    ///
    /// Není to test, ale měření — proto `#[ignore]`, aby nezatěžovalo CI:
    /// `cargo test --release bench_conversation_frames -- --ignored --nocapture`.
    /// Bez čísla se nedá poznat, jestli vykreslování celé historie stojí za
    /// opravu, ani jestli oprava něco přinesla.
    #[test]
    #[ignore = "měření, ne test"]
    fn bench_conversation_frames() {
        for count in [100usize, 500, 2000] {
            println!(
                "{count:>5} zpráv: {:.2} ms/snímek, klon s náhledy {:.2} ms, bez náhledů {:.2} ms",
                frame_cost_ms(count),
                clone_cost_ms(count, true),
                clone_cost_ms(count, false)
            );
        }
    }

    /// Cena samotného `cloned()` celé historie, který si panel dělá každý snímek.
    fn clone_cost_ms(count: usize, with_thumbnails: bool) -> f64 {
        let jid: Jid = "123@s.whatsapp.net".parse().unwrap();
        let mut state = AppState::default();
        let msgs: Vec<Message> = (0..count)
            .map(|i| {
                let mut msg = base(&format!("m{i}"), "zpráva s dost dlouhým textem");
                if with_thumbnails {
                    msg.media_jpeg_thumbnail = Some(vec![0u8; 2048]);
                }
                msg
            })
            .collect();
        state.messages.insert(jid.clone(), msgs);
        let start = std::time::Instant::now();
        let rounds = 20;
        for _ in 0..rounds {
            let cloned = state.get_chat_messages(&jid).cloned().unwrap_or_default();
            std::hint::black_box(&cloned);
        }
        start.elapsed().as_secs_f64() * 1000.0 / rounds as f64
    }

    /// The panel must actually draw something, and it has to be the rows the
    /// user is looking at: a window offset by the scroll position draws nothing
    /// at all, and a window that always starts at zero draws the wrong end of
    /// the history.
    #[test]
    fn the_panel_draws_the_visible_bubbles() {
        use crate::backend::BackendHandle;
        use crate::state::Chat;
        use std::sync::Arc;

        let jid: Jid = "123@s.whatsapp.net".parse().unwrap();
        let state: SharedState = Arc::new(parking_lot::RwLock::new(AppState::default()));
        {
            let mut s = state.write();
            s.chats.insert(
                jid.clone(),
                Chat {
                    jid: jid.clone(),
                    name: "Měření".to_string(),
                    last_message: None,
                    unread_count: 0,
                    is_group: false,
                    is_archived: false,
                    is_pinned: false,
                    mute_until: None,
                    last_activity: chrono::Utc::now(),
                    participants: Default::default(),
                    is_typing: false,
                    typing_until: None,
                },
            );
            s.messages.insert(
                jid.clone(),
                (0..200)
                    .map(|i| base(&format!("m{i}"), "zpráva s dost dlouhým textem, aby se musela lámat do řádků"))
                    .collect(),
            );
            s.selected_chat = Some(jid.clone());
        }
        let backend = BackendHandle {
            tx_command: tokio::sync::mpsc::unbounded_channel().0,
            rx_status: tokio::sync::watch::channel(crate::state::ConnectionStatus::Disconnected).1,
            state: state.clone(),
            bot_handle: Arc::new(tokio::sync::Mutex::new(None)),
        };

        let ctx = Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0)));
        for _ in 0..3 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                show_conversation(ui, &state, &backend);
            });
            // Glyph atlases are freed again at the end of the pass; dropping the
            // delta keeps the shared TextureManager from complaining.
            output.textures_delta.clear();
        }

        let heights: Vec<f32> = state.read().row_heights.values().copied().collect();
        assert!(!heights.is_empty(), "panel nenakreslil ani jednu bublinu");
        let drawn: f32 = heights.iter().sum();
        assert!(
            drawn >= 300.0,
            "nakreslené bubliny mají dohromady jen {drawn} px, viewport má 700 px"
        );
        assert!(
            heights.len() < 200,
            "kreslí se {} z 200 zpráv — virtualizace nefunguje",
            heights.len()
        );
        // Historie je odscrollovaná na konec, takže musí být nakreslená poslední
        // zpráva. Bez toho se panel jeví prázdný, i když se něco nakreslilo.
        assert!(
            state.read().row_heights.contains_key("m199"),
            "poslední zpráva se nenakreslila — okno je mimo viditelnou část"
        );
    }

    fn frame_cost_ms(count: usize) -> f64 {
        use crate::backend::BackendHandle;
        use crate::state::Chat;
        use std::sync::Arc;

        let jid: Jid = "123@s.whatsapp.net".parse().unwrap();
        let state: SharedState = Arc::new(parking_lot::RwLock::new(AppState::default()));
        {
            let mut s = state.write();
            s.chats.insert(
                jid.clone(),
                Chat {
                    jid: jid.clone(),
                    name: "Měření".to_string(),
                    last_message: None,
                    unread_count: 0,
                    is_group: false,
                    is_archived: false,
                    is_pinned: false,
                    mute_until: None,
                    last_activity: chrono::Utc::now(),
                    participants: Default::default(),
                    is_typing: false,
                    typing_until: None,
                },
            );
            s.messages.insert(
                jid.clone(),
                (0..count)
                    .map(|i| {
                        let mut msg = base(
                            &format!("m{i}"),
                            "zpráva s dost dlouhým textem, aby se musela lámat do řádků",
                        );
                        // A thumbnail makes the clone cost visible, which is part
                        // of what a frame pays for a long history.
                        msg.media_jpeg_thumbnail = Some(vec![0u8; 2048]);
                        msg
                    })
                    .collect(),
            );
            s.selected_chat = Some(jid.clone());
        }

        let (tx_command, _rx_command) = tokio::sync::mpsc::unbounded_channel();
        let (_tx_status, rx_status) =
            tokio::sync::watch::channel(crate::state::ConnectionStatus::Disconnected);
        let backend = BackendHandle {
            tx_command,
            rx_status,
            state: state.clone(),
            bot_handle: Arc::new(tokio::sync::Mutex::new(None)),
        };

        let ctx = egui::Context::default();
        let screen = egui::vec2(900.0, 700.0);
        let frames = 10usize;
        let warmup = 2usize;
        let mut total = 0.0;
        for frame in 0..frames {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen));
            let start = std::time::Instant::now();
            let mut output = ctx.run_ui(input, |ui| {
                ui.set_max_size(screen);
                show_conversation(ui, &state, &backend);
            });
            output.textures_delta.clear();
            if frame >= warmup {
                total += start.elapsed().as_secs_f64() * 1000.0;
            }
        }
        total / (frames - warmup) as f64
    }

    #[test]
    fn jump_to_bottom_hides_while_the_tail_is_on_screen() {
        // A short chat that fits: nothing to jump to.
        assert!(!should_show_jump_to_bottom(300.0, 700.0, 0.0));
        // Scrolled to the very bottom of a long history.
        assert!(!should_show_jump_to_bottom(20_000.0, 700.0, 19_300.0));
        // Just a few pixels above the bottom: the button would only flicker.
        assert!(!should_show_jump_to_bottom(20_000.0, 700.0, 19_280.0));
    }

    #[test]
    fn jump_to_bottom_shows_when_the_user_is_reading_older_messages() {
        assert!(should_show_jump_to_bottom(20_000.0, 700.0, 0.0));
        assert!(should_show_jump_to_bottom(20_000.0, 700.0, 10_000.0));
        // One screen above the bottom is still "in the past".
        assert!(should_show_jump_to_bottom(20_000.0, 700.0, 18_000.0));
        // After the jump the offset equals the scrollable range again.
        assert!(!should_show_jump_to_bottom(20_000.0, 700.0, 19_300.0));
    }

    #[test]
    fn pinned_preview_describes_media_without_caption() {
        let mut msg = base("b", "");
        msg.media_type = Some(crate::state::MediaType::Voice);
        assert_eq!(pinned_preview(&msg), "🎤 Hlasová zpráva");
        msg.media_type = None;
        assert_eq!(pinned_preview(&msg), "Zpráva");
    }
}

/// Group members as `(jid, jméno)` pairs — the pool both for `@` suggestions
/// and for turning a finished body back into `mentioned_jid`.
fn mention_people(state: &AppState, chat_jid: &Jid) -> Vec<(Jid, String)> {
    let Some(chat) = state.chats.get(chat_jid) else {
        return Vec::new();
    };
    if !chat.is_group {
        return Vec::new();
    }
    let me = state.self_jid.clone();
    chat.participants
        .iter()
        .filter(|jid| me.as_ref() != Some(*jid))
        .filter_map(|jid| state.resolve_display_name(jid).map(|name| (jid.clone(), name)))
        .collect()
}
