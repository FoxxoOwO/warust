//! UI Components - Shared reusable components

use egui::{Color32, FontId, RichText, Sense, Stroke, Ui, Vec2, Widget};

use crate::theme::{bubble, chat_list, status};
use crate::util::{format_timestamp, format_message_time, initials, truncate};

/// Status indicator icon
/// Chat-list preview text for a message, with WhatsApp's media markers.
///
/// The backend already labels uncaptioned media (`"📷 Image"`), so only the
/// plain-caption case needs an icon prefix here.
fn preview_label(preview: &crate::state::MessagePreview) -> String {
    use crate::state::MediaType;

    let text = preview.text.trim();
    if text.starts_with("[Reaction]") {
        return "Reakce".to_string();
    }

    let icon = match preview.media_type {
        Some(MediaType::Image) => "📷",
        Some(MediaType::Video) => "📹",
        Some(MediaType::Audio) | Some(MediaType::Voice) => "🎤",
        Some(MediaType::Document) => "📄",
        Some(MediaType::Sticker) => "🎨",
        None => "",
    };

    if text.is_empty() || text == "[Protocol Message]" {
        return if icon.is_empty() {
            "Zpráva".to_string()
        } else {
            format!("{} Zpráva", icon)
        };
    }

    let already_labelled = ["📷", "📹", "🎤", "🎵", "📄", "🎨"]
        .iter()
        .any(|prefix| text.starts_with(prefix));
    if icon.is_empty() || already_labelled {
        text.to_string()
    } else {
        format!("{} {}", icon, text)
    }
}

pub fn status_icon(ui: &mut Ui, status: crate::state::MessageStatus, size: f32) {
    let (icon, color) = match status {
        crate::state::MessageStatus::Sending => ("⏳", status::SENDING),
        crate::state::MessageStatus::Sent => ("✓", status::SENT),
        crate::state::MessageStatus::Delivered => ("✓✓", status::DELIVERED),
        crate::state::MessageStatus::Read => ("✓✓", status::READ),
        crate::state::MessageStatus::Failed => ("✗", status::FAILED),
    };
    
    ui.label(RichText::new(icon).font(FontId::proportional(size)).color(color));
}

/// Avatar widget
pub struct Avatar {
    pub name: String,
    pub size: f32,
    pub color: Option<Color32>,
    pub jid: Option<whatsapp_rust::wacore_binary::Jid>,
}

impl Avatar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            size: 40.0,
            color: None,
            jid: None,
        }
    }

    /// Show the cached profile picture of this JID (if any) instead of initials.
    pub fn jid(mut self, jid: whatsapp_rust::wacore_binary::Jid) -> Self {
        self.jid = Some(jid);
        self
    }
    
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

impl Widget for Avatar {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let initials = initials(&self.name);
        let color = self.color.unwrap_or_else(|| {
            // Generate consistent color from name
            let hash: u32 = self.name.chars().map(|c| c as u32).sum();
            let hue = (hash % 360) as f32 / 360.0;
            // Convert HSV to RGB
            let h = hue * 6.0;
            let c = 0.8 * 0.6;
            let x = c * (1.0 - (h % 2.0 - 1.0).abs());
            let m = 0.8 - c;
            let (r, g, b) = match h as u32 {
                0 => (c, x, 0.0),
                1 => (x, c, 0.0),
                2 => (0.0, c, x),
                3 => (0.0, x, c),
                4 => (x, 0.0, c),
                _ => (c, 0.0, x),
            };
            Color32::from_rgb(
                ((r + m) * 255.0) as u8,
                ((g + m) * 255.0) as u8,
                ((b + m) * 255.0) as u8,
            )
        });
        
        let desired_size = Vec2::splat(self.size);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());
        
        if ui.is_rect_visible(rect) {
            let tex = self.jid.as_ref().and_then(|j| crate::avatars::texture(ui.ctx(), j));
            if let Some(tex) = tex {
                crate::avatars::paint_circle(ui, &tex, rect);
                return response;
            }
            let painter = ui.painter();
            painter.circle_filled(rect.center(), self.size / 2.0, color);
            
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                initials,
                FontId::proportional(self.size * 0.45),
                Color32::WHITE,
            );
        }
        
        response
    }
}

/// Chat list item widget
pub struct ChatItem<'a> {
    pub chat: &'a crate::state::Chat,
    pub is_selected: bool,
    pub on_click: Option<Box<dyn FnOnce() + 'a>>,
    /// Unsent text for this chat; shown as a draft instead of the last message.
    pub draft: Option<&'a str>,
}

impl<'a> ChatItem<'a> {
    pub fn new(chat: &'a crate::state::Chat, is_selected: bool) -> Self {
        Self {
            chat,
            is_selected,
            on_click: None,
            draft: None,
        }
    }
    
    pub fn draft(mut self, text: Option<&'a str>) -> Self {
        self.draft = text;
        self
    }

    pub fn on_click(mut self, f: impl FnOnce() + 'a) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl<'a> Widget for ChatItem<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let height = 72.0;
        let desired_size = Vec2::new(ui.available_width(), height);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        // Hover and selection cross-fade instead of snapping between three
        // hard-coded fills, and the accent bar grows out of the left edge with
        // the selection so a chat switch reads as one movement.
        let row_id = egui::Id::unique(("chat_row", self.chat.jid.to_string()));
        let hover = crate::ui::animations::hover_t(ui.ctx(), row_id.with("hover"), response.hovered());
        let selected = crate::ui::animations::hover_t(ui.ctx(), row_id.with("selected"), self.is_selected);
        let fill = crate::ui::animations::lerp_color(
            crate::ui::animations::lerp_color(chat_list::BACKGROUND, chat_list::HOVER, hover),
            chat_list::SELECTED,
            selected,
        );
        ui.painter().rect_filled(rect, 0.0, fill);
        if selected > 0.01 {
            let bar = egui::Rect::from_center_size(
                egui::pos2(rect.min.x + 2.0, rect.center().y),
                Vec2::new(3.0, rect.height() * selected),
            );
            ui.painter()
                .rect_filled(bar, 2.0, crate::ui::animations::fade_color(crate::theme::ACCENT, selected));
        }
        
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let padding = 12.0;
            let avatar_size = 48.0;
            let avatar_rect = egui::Rect::from_min_size(
                rect.min + Vec2::new(padding, (height - avatar_size) / 2.0),
                Vec2::splat(avatar_size),
            );
            
            // Avatar
            if let Some(tex) = crate::avatars::texture(ui.ctx(), &self.chat.jid) {
                crate::avatars::paint_circle(ui, &tex, avatar_rect);
            } else {
                painter.circle_filled(avatar_rect.center(), avatar_size / 2.0, 
                    Color32::from_rgb(0x2A, 0x39, 0x42));
                painter.text(
                    avatar_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    initials(&self.chat.name),
                    FontId::proportional(20.0),
                    Color32::WHITE,
                );
            }
            
            // Name and timestamp
            let text_x = avatar_rect.max.x + padding;
            let name_y = rect.min.y + padding;
            let time_y = rect.min.y + padding;
            
            let display_name = if self.chat.name.ends_with("@lid") || self.chat.name.ends_with("@s.whatsapp.net") {
                crate::util::format_jid(&self.chat.jid)
            } else {
                self.chat.name.clone()
            };
            
            painter.text(
                egui::pos2(text_x, name_y),
                egui::Align2::LEFT_TOP,
                &display_name,
                FontId::proportional(15.0),
                Color32::WHITE,
            );
            
            let display_ts = {
                let mut best = None;
                if self.chat.last_activity > chrono::DateTime::<chrono::Utc>::UNIX_EPOCH {
                    best = Some(self.chat.last_activity);
                }
                if let Some(lm) = &self.chat.last_message {
                    best = Some(best.map_or(lm.timestamp, |b| b.max(lm.timestamp)));
                }
                best
            };

            if let Some(ts) = display_ts {
                let time_str = format_timestamp(ts);
                painter.text(
                    egui::pos2(rect.max.x - padding, time_y),
                    egui::Align2::RIGHT_TOP,
                    time_str,
                    FontId::proportional(11.0),
                    Color32::from_rgb(0x86, 0x96, 0xA0),
                );
            }
            
            // Preview, typing animation, and unread badge
            let preview_y = name_y + 22.0;
            if self.chat.is_currently_typing() {
                painter.text(
                    egui::pos2(text_x, preview_y),
                    egui::Align2::LEFT_TOP,
                    "píše",
                    FontId::proportional(13.0),
                    Color32::from_rgb(0x25, 0xD3, 0x66),
                );
                let time = ui.ctx().input(|i| i.time);
                crate::ui::animations::bouncing_dots_painter(
                    &painter,
                    egui::pos2(text_x + 32.0, preview_y + 4.0),
                    time,
                    Color32::from_rgb(0x25, 0xD3, 0x66),
                    2.5,
                    3.5,
                );
                ui.ctx().request_repaint();
            } else if let Some(draft) = self.draft.filter(|d| !d.trim().is_empty()) {
                // Drafts win over the last message, as they do on the phone.
                let preview_text = truncate(&format!("Koncept: {}", draft.trim()), 40);
                painter.text(
                    egui::pos2(text_x, preview_y),
                    egui::Align2::LEFT_TOP,
                    &preview_text,
                    FontId::proportional(13.0),
                    Color32::from_rgb(0x25, 0xD3, 0x66),
                );
            } else if let Some(preview) = &self.chat.last_message {
                let clean_text = preview_label(preview);

                let preview_text = if preview.from_me {
                    format!("You: {}", truncate(&clean_text, 40))
                } else {
                    truncate(&clean_text, 40)
                };
                
                let preview_color = if self.chat.unread_count > 0 {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(0x86, 0x96, 0xA0)
                };
                
                painter.text(
                    egui::pos2(text_x, preview_y),
                    egui::Align2::LEFT_TOP,
                    &preview_text,
                    FontId::proportional(13.0),
                    preview_color,
                );
                
                // Status icon for outgoing
                if preview.from_me {
                    let layout = painter.layout_no_wrap(
                        preview_text.clone(),
                        FontId::proportional(13.0),
                        preview_color,
                    );
                    let icon_x = text_x + layout.size().x + 4.0;
                    let (icon, color) = match preview.status {
                        crate::state::MessageStatus::Sending => ("⏳", status::SENDING),
                        crate::state::MessageStatus::Sent => ("✓", status::SENT),
                        crate::state::MessageStatus::Delivered => ("✓✓", status::DELIVERED),
                        crate::state::MessageStatus::Read => ("✓✓", status::READ),
                        crate::state::MessageStatus::Failed => ("✗", status::FAILED),
                    };
                    painter.text(
                        egui::pos2(icon_x, preview_y),
                        egui::Align2::LEFT_TOP,
                        icon,
                        FontId::proportional(12.0),
                        color,
                    );
                }
            }
            
            // Unread badge
            if self.chat.unread_count > 0 {
                let badge_text = if self.chat.unread_count > 99 { "99+" } else { &self.chat.unread_count.to_string() };
                let badge_width = (badge_text.len() as f32 * 8.0).max(20.0) + 8.0;
                let badge_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.max.x - padding - badge_width, preview_y + 2.0),
                    Vec2::new(badge_width, 20.0),
                );
                
                painter.rect_filled(badge_rect, 10.0, chat_list::UNREAD_BG);
                painter.text(
                    badge_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    badge_text,
                    FontId::proportional(11.0),
                    chat_list::UNREAD_TEXT,
                );
            }
            
            // Muted icon
            if self.chat.is_muted() {
                painter.text(
                    egui::pos2(rect.max.x - padding - 20.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    "🔕",
                    FontId::proportional(16.0),
                    chat_list::MUTED_ICON,
                );
            }
            
            // Pinned icon
            if self.chat.is_pinned {
                let pin_x = if self.chat.unread_count > 0 {
                    rect.max.x - padding - 28.0
                } else {
                    rect.max.x - padding
                };
                painter.text(
                    egui::pos2(pin_x, preview_y + 2.0),
                    egui::Align2::RIGHT_TOP,
                    "📌",
                    FontId::proportional(13.0),
                    chat_list::PINNED_ICON,
                );
            }
        }
        
        response
    }
}

/// Message bubble widget
pub struct MessageBubble<'a> {
    pub message: &'a crate::state::Message,
    pub is_own: bool,
    pub show_sender_name: bool,
    pub sender_name: Option<&'a str>,
    pub on_reply: Option<Box<dyn FnOnce(&'a str) + 'a>>,
    pub on_edit: Option<Box<dyn FnOnce(&'a str) + 'a>>,
    pub on_delete: Option<Box<dyn FnOnce(&'a str) + 'a>>,
    pub on_download: Option<Box<dyn Fn(&'a str) + 'a>>,
    pub on_react: Option<Box<dyn FnOnce(&'a str, &'a str) + 'a>>,
    pub on_view_reactions: Option<Box<dyn FnOnce(&'a str) + 'a>>,
    pub on_open_emoji_picker: Option<Box<dyn FnOnce(&'a str) + 'a>>,
    pub on_select: Option<Box<dyn FnOnce(&'a str) + 'a>>,
    /// Pins the message for the given duration, or unpins when `None`.
    pub on_pin: Option<Box<dyn FnOnce(&'a str, Option<u32>) + 'a>>,
    /// Whether this message currently carries the chat's pin.
    pub is_pinned: bool,
    /// Bulk-action mode: a click ticks the bubble instead of using the media.
    pub selection_mode: bool,
    pub selected: bool,
    /// Briefly tinted and outlined, e.g. after jumping here from search.
    /// A 0..1 intensity, so the tint can fade in and back out.
    pub highlight: f32,
    /// Fade-in progress for a bubble that just arrived; 1.0 means fully there.
    pub entrance: f32,
    /// Fired with the option name when a poll option row is clicked.
    pub on_poll_option: Option<Box<dyn FnOnce(String) + 'a>>,
    /// Fired by the "Hlasovat" button of a multi-choice poll.
    pub on_poll_submit: Option<Box<dyn FnOnce() + 'a>>,
    /// Options ticked in a multi-choice poll but not sent yet.
    pub poll_draft: Vec<String>,
    /// Our own JID (string form), used to mark the options we voted for.
    pub me: Option<&'a str>,
    /// Fired when the view-once tile is clicked and the media must be fetched.
    pub on_open_view_once: Option<Box<dyn FnOnce() + 'a>>,
}

impl<'a> MessageBubble<'a> {
    pub fn new(message: &'a crate::state::Message, is_own: bool) -> Self {
        Self {
            message,
            is_own,
            show_sender_name: false,
            sender_name: None,
            on_reply: None,
            on_edit: None,
            on_delete: None,
            on_download: None,
            on_react: None,
            on_view_reactions: None,
            on_open_emoji_picker: None,
            on_select: None,
            on_pin: None,
            is_pinned: false,
            selection_mode: false,
            selected: false,
            highlight: 0.0,
            entrance: 1.0,
            on_poll_option: None,
            on_poll_submit: None,
            poll_draft: Vec::new(),
            me: None,
            on_open_view_once: None,
        }
    }
    
    pub fn show_sender(mut self, name: &'a str) -> Self {
        self.show_sender_name = true;
        self.sender_name = Some(name);
        self
    }
    
    pub fn on_reply(mut self, f: impl FnOnce(&'a str) + 'a) -> Self {
        self.on_reply = Some(Box::new(f));
        self
    }

    pub fn on_delete(mut self, f: impl FnOnce(&'a str) + 'a) -> Self {
        self.on_delete = Some(Box::new(f));
        self
    }

    pub fn on_edit(mut self, f: impl FnOnce(&'a str) + 'a) -> Self {
        self.on_edit = Some(Box::new(f));
        self
    }

    pub fn on_pin(mut self, f: impl FnOnce(&'a str, Option<u32>) + 'a) -> Self {
        self.on_pin = Some(Box::new(f));
        self
    }

    pub fn is_pinned(mut self, pinned: bool) -> Self {
        self.is_pinned = pinned;
        self
    }

    pub fn on_select(mut self, f: impl FnOnce(&'a str) + 'a) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }

    /// Turn the bubble into a selectable row for the bulk-action bar.
    pub fn selection_mode(mut self, on: bool) -> Self {
        self.selection_mode = on;
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn highlight(mut self, amount: f32) -> Self {
        self.highlight = amount.clamp(0.0, 1.0);
        self
    }

    /// Fade a bubble that just arrived in. Takes an already eased 0..1 and only
    /// touches opacity, so the bubble never moves its neighbours.
    pub fn entrance(mut self, amount: f32) -> Self {
        self.entrance = amount.clamp(0.0, 1.0);
        self
    }

    pub fn on_poll_option(mut self, f: impl FnOnce(String) + 'a) -> Self {
        self.on_poll_option = Some(Box::new(f));
        self
    }

    pub fn on_poll_submit(mut self, f: impl FnOnce() + 'a) -> Self {
        self.on_poll_submit = Some(Box::new(f));
        self
    }

    pub fn poll_draft(mut self, names: Vec<String>) -> Self {
        self.poll_draft = names;
        self
    }

    pub fn me(mut self, jid: &'a str) -> Self {
        self.me = Some(jid);
        self
    }

    pub fn on_open_view_once(mut self, f: impl FnOnce() + 'a) -> Self {
        self.on_open_view_once = Some(Box::new(f));
        self
    }

    pub fn on_download(mut self, f: impl Fn(&'a str) + 'a) -> Self {
        self.on_download = Some(Box::new(f));
        self
    }

    pub fn on_react(mut self, f: impl FnOnce(&'a str, &'a str) + 'a) -> Self {
        self.on_react = Some(Box::new(f));
        self
    }

    pub fn on_view_reactions(mut self, f: impl FnOnce(&'a str) + 'a) -> Self {
        self.on_view_reactions = Some(Box::new(f));
        self
    }

    pub fn on_open_emoji_picker(mut self, f: impl FnOnce(&'a str) + 'a) -> Self {
        self.on_open_emoji_picker = Some(Box::new(f));
        self
    }
}

/// Tile standing in for a view-once attachment: no thumbnail and no size, so
/// nothing about the payload leaks before it is opened.
fn paint_view_once_tile<'a>(
    ctx: &egui::Context,
    painter: &egui::Painter,
    rect: egui::Rect,
    mt: crate::state::MediaType,
    message: &crate::state::Message,
    on_open: &mut Option<Box<dyn FnOnce() + 'a>>,
) {
    painter.rect_filled(rect, 8.0, Color32::from_rgb(0x1E, 0x2B, 0x32));

    let badge_center = rect.center() + Vec2::new(0.0, -12.0);
    painter.circle_stroke(badge_center, 12.0, Stroke::new(2.0, crate::theme::ACCENT));
    painter.text(
        badge_center,
        egui::Align2::CENTER_CENTER,
        "1",
        FontId::proportional(13.0),
        crate::theme::ACCENT,
    );

    let kind = match mt {
        crate::state::MediaType::Image | crate::state::MediaType::Sticker => "Fotka",
        crate::state::MediaType::Video => "Video",
        _ => "Médium",
    };
    let label = if message.view_once_opened {
        "Jednorázové médium bylo zobrazeno".to_string()
    } else {
        format!("{kind} (zobrazit jednou)")
    };
    painter.text(
        rect.center() + Vec2::new(0.0, 16.0),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        Color32::from_rgb(0xA9, 0xB6, 0xBD),
    );

    if message.view_once_opened {
        return;
    }

    // Hit-tested against the pointer instead of `ui.interact`: the bubble paints
    // through a painter borrowed from `ui`, so `ui` cannot be borrowed mutably here.
    let hovered = ctx
        .pointer_hover_pos()
        .map_or(false, |pos| rect.contains(pos));
    if hovered {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        if ctx.input(|i| i.pointer.primary_clicked()) {
            if let Some(f) = on_open.take() {
                f();
            }
        }
    }
}

impl<'a> Widget for MessageBubble<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let max_bubble_width = (ui.available_width() * 0.72).max(180.0);
        let bubble_padding = Vec2::new(12.0, 8.0);
        let text_color = if self.is_own { bubble::OUTGOING_TEXT } else { bubble::INCOMING_TEXT };
        let text_wrap_width = (max_bubble_width - bubble_padding.x * 2.0).max(60.0);
        
        let media_type = self.message.media_type;
        let mut media_size = Vec2::ZERO;
        let mut cached_texture: Option<(egui::TextureHandle, [f32; 2])> = None;

        if let Some(mt) = media_type {
            if self.message.view_once {
                // Opaque tile: neither the bytes nor a thumbnail are shown
                // until the recipient asks for them.
                media_size = Vec2::new(
                    220.0_f32.min(max_bubble_width - bubble_padding.x * 2.0),
                    96.0,
                );
            } else {
            match mt {
                crate::state::MediaType::Image | crate::state::MediaType::Sticker => {
                    if let Some((tex, dim)) = crate::media::image_texture(ui.ctx(), self.message) {
                        let max_w = (max_bubble_width - bubble_padding.x * 2.0).min(260.0);
                        let aspect = if dim[1] > 0.0 { dim[0] / dim[1] } else { 1.33 };
                        let mut w = max_w;
                        let mut h = w / aspect;
                        if h > 260.0 {
                            h = 260.0;
                            w = h * aspect;
                        }
                        media_size = Vec2::new(w.max(120.0), h.max(80.0));
                        cached_texture = Some((tex, dim));
                    } else {
                        media_size = Vec2::new(220.0_f32.min(max_bubble_width - bubble_padding.x * 2.0), 130.0);
                    }
                }
                crate::state::MediaType::Video => {
                    if let Some((tex, dim)) = crate::media::image_texture(ui.ctx(), self.message) {
                        let max_w = (max_bubble_width - bubble_padding.x * 2.0).min(260.0);
                        let aspect = if dim[1] > 0.0 { dim[0] / dim[1] } else { 1.33 };
                        let mut w = max_w;
                        let mut h = w / aspect;
                        if h > 200.0 {
                            h = 200.0;
                            w = h * aspect;
                        }
                        media_size = Vec2::new(w.max(120.0), h.max(80.0));
                        cached_texture = Some((tex, dim));
                    } else {
                        media_size = Vec2::new(230.0_f32.min(max_bubble_width - bubble_padding.x * 2.0), 120.0);
                    }
                }
                crate::state::MediaType::Audio | crate::state::MediaType::Voice => {
                    media_size = Vec2::new(220.0_f32.min(max_bubble_width - bubble_padding.x * 2.0), 40.0);
                }
                crate::state::MediaType::Document => {
                    media_size = Vec2::new(240.0_f32.min(max_bubble_width - bubble_padding.x * 2.0), 52.0);
                }
            }
            }
        }

        let display_text = if let Some(ref cap) = self.message.media_caption {
            cap.clone()
        } else if media_type.is_some() {
            if self.message.text.starts_with("📷") || self.message.text.starts_with("📹") 
                || self.message.text.starts_with("📄") || self.message.text.starts_with("🎤")
                || self.message.text.starts_with("🎵") || self.message.text.starts_with("🎨") {
                String::new()
            } else {
                self.message.text.clone()
            }
        } else {
            self.message.text.clone()
        };

        let is_emoji_only = if media_type.is_none() && !display_text.trim().is_empty() {
            crate::state::count_emoji_only(&display_text)
        } else {
            None
        };

        let (font_size, is_big_emoji) = match is_emoji_only {
            Some(1) => (44.0, true),
            Some(2) => (30.0, true),
            Some(3) => (24.0, true),
            _ => (14.0, false),
        };

        let has_text = !display_text.trim().is_empty();
        let galley = if has_text {
            Some(ui.painter().layout_job(mention_layout_job(
                display_text.as_str(),
                crate::theme::message_font(font_size),
                text_color,
                text_wrap_width,
            )))
        } else {
            None
        };
        
        let sender_height = if self.show_sender_name && !self.is_own && self.sender_name.is_some() {
            18.0
        } else {
            0.0
        };
        
        let reply_height = if self.message.reply_to.is_some() {
            36.0
        } else {
            0.0
        };

        let media_height = if media_size.y > 0.0 { media_size.y + 6.0 } else { 0.0 };
        let text_height = galley.as_ref().map_or(0.0, |g| g.size().y + 4.0);
        let time_height = 16.0;

        // Polls grow a tally block under the question: one row per option plus
        // a header line and, for multi-choice polls, a submit button.
        let poll_row_count = self.message.poll.as_ref().map_or(0, |p| p.options.len());
        let poll_is_multi = self.message.poll.as_ref().is_some_and(|p| p.is_multi());
        let poll_height = if poll_row_count == 0 {
            0.0
        } else {
            22.0 + poll_row_count as f32 * 28.0 + if poll_is_multi { 28.0 } else { 4.0 }
        };

        let content_width = {
            let tw = galley.as_ref().map_or(0.0, |g| g.size().x);
            // Tally bars need room for an option name next to its count.
            let floor = if poll_row_count > 0 { 240.0 } else { 110.0 };
            (tw.max(media_size.x) + bubble_padding.x * 2.0).max(floor)
        };
        let bubble_width = content_width.min(max_bubble_width);
        let bubble_height = bubble_padding.y * 2.0 + sender_height + reply_height + media_height + text_height + time_height + poll_height + 4.0;
        
        let reaction_text = if !self.message.reactions.is_empty() {
            let mut emojis: Vec<String> = self.message.reactions.iter().map(|r| r.emoji.clone()).collect();
            emojis.dedup();
            if self.message.reactions.len() > 1 {
                format!("{} {}", emojis.join(""), self.message.reactions.len())
            } else {
                emojis.join("")
            }
        } else {
            String::new()
        };
        let has_reactions = !reaction_text.is_empty();

        let row_height = bubble_height + if has_reactions { 14.0 } else { 4.0 };
        let row_size = Vec2::new(ui.available_width(), row_height);
        let (row_rect, response) = ui.allocate_exact_size(row_size, Sense::click());

        // A bubble that just arrived fades in where it will stay; the row was
        // already measured, so nothing under it shifts while it appears.
        if self.entrance < 1.0 {
            ui.set_opacity(self.entrance);
        }
        
        let mut audio_clicked = false;
        let mut video_scrub_used = false;
        let mut on_poll_option = self.on_poll_option;
        let mut on_poll_submit = self.on_poll_submit;
        let mut on_open_view_once = self.on_open_view_once;

        let bubble_rect = if self.is_own {
            egui::Rect::from_min_size(
                egui::pos2(row_rect.max.x - bubble_width - 8.0, row_rect.min.y + 2.0),
                Vec2::new(bubble_width, bubble_height),
            )
        } else {
            egui::Rect::from_min_size(
                egui::pos2(row_rect.min.x + 8.0, row_rect.min.y + 2.0),
                Vec2::new(bubble_width, bubble_height),
            )
        };
        
        if ui.is_rect_visible(row_rect) {
            let painter = ui.painter();
            
            // Bubble background
            let bubble_color = if is_big_emoji {
                Color32::TRANSPARENT
            } else if self.is_own {
                bubble::OUTGOING_BG
            } else {
                bubble::INCOMING_BG
            };
            if !is_big_emoji {
                painter.rect_filled(bubble_rect, 12.0, bubble_color);
                
                // Tail (triangle)
                let tail_size = 12.0;
                if self.is_own {
                    let tail_points = [
                        bubble_rect.right_top() + Vec2::new(-tail_size, 0.0),
                        bubble_rect.right_top() + Vec2::new(4.0, 0.0),
                        bubble_rect.right_top() + Vec2::new(0.0, tail_size),
                    ];
                    painter.add(egui::Shape::convex_polygon(tail_points.to_vec(), bubble_color, Stroke::NONE));
                } else {
                    let tail_points = [
                        bubble_rect.left_top() + Vec2::new(tail_size, 0.0),
                        bubble_rect.left_top() + Vec2::new(-4.0, 0.0),
                        bubble_rect.left_top() + Vec2::new(0.0, tail_size),
                    ];
                    painter.add(egui::Shape::convex_polygon(tail_points.to_vec(), bubble_color, Stroke::NONE));
                }
            }
            
            // Sender name (for groups)
            let mut y_offset = bubble_padding.y;
            if self.show_sender_name && !self.is_own {
                if let Some(name) = self.sender_name {
                    painter.text(
                        bubble_rect.min + Vec2::new(bubble_padding.x, y_offset),
                        egui::Align2::LEFT_TOP,
                        name,
                        FontId::proportional(11.0),
                        bubble::REPLY_NAME,
                    );
                    y_offset += sender_height;
                }
            }
            
            // Reply preview
            if let Some(_reply_to) = &self.message.reply_to {
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        bubble_rect.min + Vec2::new(bubble_padding.x, y_offset),
                        Vec2::new(bubble_rect.width() - bubble_padding.x * 2.0, 30.0),
                    ),
                    6.0,
                    bubble::REPLY_BG,
                );
                painter.text(
                    bubble_rect.min + Vec2::new(bubble_padding.x + 8.0, y_offset + 6.0),
                    egui::Align2::LEFT_TOP,
                    "↳ Replying to message",
                    FontId::proportional(11.0),
                    bubble::REPLY_TEXT,
                );
                y_offset += reply_height;
            }
            
            // Media rendering
            if let Some(mt) = media_type {
                let media_rect = egui::Rect::from_min_size(
                    bubble_rect.min + Vec2::new(bubble_padding.x, y_offset),
                    media_size,
                );

                if self.message.view_once {
                    paint_view_once_tile(ui.ctx(), painter, media_rect, mt, self.message, &mut on_open_view_once);
                } else {
                let is_downloaded = self.message.media_path.as_ref().map_or(false, |p| std::path::Path::new(p).exists());
                let file_len = self.message.media_file_length.unwrap_or(0);

                match mt {
                    crate::state::MediaType::Image | crate::state::MediaType::Sticker => {
                        if let Some((ref tex, _)) = cached_texture {
                            egui::Image::new(egui::load::SizedTexture::new(tex.id(), media_size))
                                .corner_radius(8.0)
                                .paint_at(ui, media_rect);

                            if !is_downloaded {
                                painter.circle_filled(media_rect.center(), 22.0, Color32::from_black_alpha(160));
                                painter.text(
                                    media_rect.center() + Vec2::new(0.0, if file_len > 0 { -4.0 } else { 0.0 }),
                                    egui::Align2::CENTER_CENTER,
                                    "⬇",
                                    FontId::proportional(16.0),
                                    Color32::WHITE,
                                );
                                if file_len > 0 {
                                    painter.text(
                                        media_rect.center() + Vec2::new(0.0, 11.0),
                                        egui::Align2::CENTER_CENTER,
                                        crate::media::format_file_size(file_len),
                                        FontId::proportional(9.0),
                                        Color32::from_rgb(0xDF, 0xE5, 0xE7),
                                    );
                                }
                            }
                        } else {
                            painter.rect_filled(media_rect, 8.0, Color32::from_black_alpha(50));
                            painter.text(
                                media_rect.center() + Vec2::new(0.0, -8.0),
                                egui::Align2::CENTER_CENTER,
                                "📷 Photo",
                                FontId::proportional(14.0),
                                text_color,
                            );
                            if !is_downloaded {
                                let label = if file_len > 0 {
                                    format!("⬇ Stáhnout ({})", crate::media::format_file_size(file_len))
                                } else {
                                    "⬇ Stáhnout".to_string()
                                };
                                painter.text(
                                    media_rect.center() + Vec2::new(0.0, 12.0),
                                    egui::Align2::CENTER_CENTER,
                                    label,
                                    FontId::proportional(11.0),
                                    Color32::from_rgb(0x00, 0xA8, 0x84),
                                );
                            }
                        }
                    }
                    crate::state::MediaType::Video => {
                        let live_frame = if crate::video::is_open_for(&self.message.id) {
                            crate::video::texture(ui.ctx(), &self.message.id)
                        } else {
                            None
                        };
                        if let Some(live) = live_frame {
                            // Decoded frames stream in at source fps; keep asking for repaints
                            // while the player is running, then normal frames take over.
                            egui::Image::new(egui::load::SizedTexture::new(live.id(), media_size))
                                .corner_radius(8.0)
                                .paint_at(ui, media_rect);
                            ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
                            if crate::video::is_paused() {
                                painter.circle_filled(media_rect.center(), 22.0, Color32::from_black_alpha(150));
                                painter.text(
                                    media_rect.center() + Vec2::new(1.5, 0.0),
                                    egui::Align2::CENTER_CENTER,
                                    "\u{25B6}",
                                    FontId::proportional(16.0),
                                    Color32::WHITE,
                                );
                            }
                            if media_rect.height() > 60.0 {
                                video_scrub_used |= video_timeline(ui, &self.message.id, media_rect);
                            }
                        } else if let Some((ref tex, _)) = cached_texture {
                            egui::Image::new(egui::load::SizedTexture::new(tex.id(), media_size))
                                .corner_radius(8.0)
                                .paint_at(ui, media_rect);
                        } else {
                            painter.rect_filled(media_rect, 8.0, Color32::from_black_alpha(60));
                        }
                        painter.circle_filled(media_rect.center(), 22.0, Color32::from_black_alpha(170));
                        painter.text(
                            media_rect.center() + Vec2::new(if is_downloaded { 1.5 } else { 0.0 }, if file_len > 0 && !is_downloaded { -4.0 } else { 0.0 }),
                            egui::Align2::CENTER_CENTER,
                            if is_downloaded { "▶" } else { "⬇" },
                            FontId::proportional(16.0),
                            Color32::WHITE,
                        );
                        if !is_downloaded && file_len > 0 {
                            painter.text(
                                media_rect.center() + Vec2::new(0.0, 11.0),
                                egui::Align2::CENTER_CENTER,
                                crate::media::format_file_size(file_len),
                                FontId::proportional(9.0),
                                Color32::from_rgb(0xDF, 0xE5, 0xE7),
                            );
                        }
                    }
                    crate::state::MediaType::Document => {
                        painter.rect_filled(media_rect, 8.0, Color32::from_black_alpha(45));
                        
                        let icon_rect = egui::Rect::from_min_size(
                            media_rect.min + Vec2::new(8.0, 8.0),
                            Vec2::new(34.0, 34.0),
                        );
                        painter.rect_filled(icon_rect, 6.0, Color32::from_rgb(0x7F, 0x66, 0xFF));
                        painter.text(
                            icon_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "📄",
                            FontId::proportional(18.0),
                            Color32::WHITE,
                        );
                        
                        let doc_name = self.message.media_caption.as_deref().unwrap_or(&self.message.text);
                        let truncated_name = crate::util::truncate(doc_name, 24);
                        painter.text(
                            media_rect.min + Vec2::new(50.0, 10.0),
                            egui::Align2::LEFT_TOP,
                            truncated_name,
                            FontId::proportional(13.0),
                            text_color,
                        );
                        
                        let status_sub = if is_downloaded {
                            if file_len > 0 {
                                format!("{} • ✓ Otevřít", crate::media::format_file_size(file_len))
                            } else {
                                "✓ Otevřít".to_string()
                            }
                        } else if file_len > 0 {
                            format!("{} • ⬇ Stáhnout", crate::media::format_file_size(file_len))
                        } else {
                            "⬇ Stáhnout".to_string()
                        };
                        painter.text(
                            media_rect.min + Vec2::new(50.0, 28.0),
                            egui::Align2::LEFT_TOP,
                            status_sub,
                            FontId::proportional(11.0),
                            if is_downloaded { Color32::from_rgb(0x25, 0xD3, 0x66) } else { Color32::from_rgb(0x86, 0x96, 0xA0) },
                        );
                    }
                    crate::state::MediaType::Audio | crate::state::MediaType::Voice => {
                        painter.rect_filled(media_rect, 8.0, Color32::from_black_alpha(45));

                        let is_current = is_downloaded && crate::audio::is_playing(&self.message.id);
                        let mid_y = media_rect.min.y + media_rect.height() / 2.0;
                        let btn_center = egui::pos2(media_rect.min.x + 22.0, mid_y);
                        painter.circle_filled(btn_center, 15.0, crate::theme::ACCENT);
                        painter.text(
                            btn_center,
                            egui::Align2::CENTER_CENTER,
                            if !is_downloaded {
                                "⬇"
                            } else if is_current {
                                "❚❚"
                            } else {
                                "▶"
                            },
                            FontId::proportional(if is_current { 10.0 } else { 13.0 }),
                            Color32::WHITE,
                        );

                        // Waveform bars double as the progress indicator while playing.
                        let track_left = media_rect.min.x + 46.0;
                        let track_right = media_rect.max.x - 52.0;
                        let frac = if is_current {
                            match crate::audio::duration() {
                                Some(total) if total.as_secs_f32() > 0.0 => {
                                    (crate::audio::position().as_secs_f32() / total.as_secs_f32())
                                        .clamp(0.0, 1.0)
                                }
                                _ => 0.0,
                            }
                        } else {
                            0.0
                        };
                        let bars = 26;
                        for i in 0..bars {
                            let t = i as f32 / (bars - 1) as f32;
                            let x = track_left + (track_right - track_left) * t;
                            // Deterministic pseudo-waveform so bubbles look consistent.
                            let seed = ((i as f32 * 12.9898 + 78.233).sin() * 43758.5453).fract().abs();
                            let h = 3.0 + 9.0 * seed;
                            let played = is_current && t <= frac;
                            painter.line_segment(
                                [
                                    egui::pos2(x, mid_y - h / 2.0),
                                    egui::pos2(x, mid_y + h / 2.0),
                                ],
                                Stroke::new(
                                    2.0,
                                    if !is_downloaded {
                                        Color32::from_rgb(0x4a, 0x5c, 0x66)
                                    } else if played {
                                        crate::theme::ACCENT
                                    } else {
                                        Color32::from_rgb(0x86, 0x96, 0xA0)
                                    },
                                ),
                            );
                        }

                        let sub = if !is_downloaded {
                            if file_len > 0 {
                                crate::media::format_file_size(file_len)
                            } else {
                                "Hlasová zpráva".to_string()
                            }
                        } else if is_current {
                            match crate::audio::duration() {
                                Some(total) => format!(
                                    "{} / {}",
                                    crate::audio::format_duration(crate::audio::position()),
                                    crate::audio::format_duration(total)
                                ),
                                None => crate::audio::format_duration(crate::audio::position()),
                            }
                        } else {
                            "Přehrát".to_string()
                        };
                        painter.text(
                            media_rect.max + Vec2::new(-8.0, -media_rect.height() / 2.0),
                            egui::Align2::RIGHT_CENTER,
                            sub,
                            FontId::proportional(10.0),
                            Color32::from_rgb(0x86, 0x96, 0xA0),
                        );

                        // Clickable play/stop target over the round button.
                        let hit = egui::Rect::from_center_size(
                            btn_center,
                            Vec2::splat(34.0),
                        );
                        let play_resp = ui.interact(
                            hit,
                            ui.scope_id().with(("bubble-audio", self.message.id.as_str())),
                            Sense::click(),
                        );
                        if play_resp.clicked() {
                            audio_clicked = true;
                        }
                        if play_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        if is_current {
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_millis(60));
                        }
                    }
                }

                }

                y_offset += media_height;
            }

            // Message text (caption)
            if let Some(ref g) = galley {
                painter.galley(
                    bubble_rect.min + Vec2::new(bubble_padding.x, y_offset),
                    g.clone(),
                    text_color,
                );
                y_offset += g.size().y + 4.0;
            }

            // Poll: one tally bar per option, click to vote.
            if let Some(poll) = &self.message.poll {
                let left = bubble_rect.min.x + bubble_padding.x;
                let inner_width = bubble_rect.width() - bubble_padding.x * 2.0;
                let tallies = poll.tallies();
                let total_votes: usize = tallies.iter().map(|(_, count)| *count).sum();
                let my_vote = self.me.map(|me| poll.my_vote(me)).unwrap_or_default();

                painter.text(
                    egui::pos2(left, y_offset),
                    egui::Align2::LEFT_TOP,
                    if poll.is_quiz { "🧠 Kvíz" } else { "📊 Anketa" },
                    FontId::proportional(10.0),
                    bubble::REPLY_NAME,
                );
                painter.text(
                    egui::pos2(bubble_rect.max.x - bubble_padding.x, y_offset),
                    egui::Align2::RIGHT_TOP,
                    format!(
                        "{} {}",
                        total_votes,
                        if total_votes == 1 { "hlas" } else { "hlasů" }
                    ),
                    FontId::proportional(10.0),
                    Color32::from_rgb(0x86, 0x96, 0xA0),
                );
                y_offset += 22.0;

                for (name, count) in &tallies {
                    let row = egui::Rect::from_min_size(
                        egui::pos2(left, y_offset),
                        Vec2::new(inner_width, 24.0),
                    );
                    let ticked = my_vote.iter().any(|picked| picked == name)
                        || self.poll_draft.iter().any(|picked| picked == name);
                    let resp = ui.interact(
                        row,
                        ui.scope_id()
                            .with(("poll-opt", self.message.id.as_str(), name.as_str())),
                        Sense::click(),
                    );
                    painter.rect_filled(
                        row,
                        6.0,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 14),
                    );
                    if total_votes > 0 && *count > 0 {
                        let share = (*count as f32 / total_votes as f32).min(1.0);
                        painter.rect_filled(
                            egui::Rect::from_min_size(
                                row.min,
                                Vec2::new(row.width() * share, row.height()),
                            ),
                            6.0,
                            if ticked {
                                Color32::from_rgba_unmultiplied(0x00, 0xA8, 0x84, 110)
                            } else {
                                Color32::from_rgba_unmultiplied(255, 255, 255, 30)
                            },
                        );
                    }
                    if ticked {
                        painter.rect_stroke(
                            row,
                            6.0,
                            egui::Stroke::new(1.5, crate::theme::ACCENT),
                            egui::StrokeKind::Inside,
                        );
                    }
                    painter.text(
                        egui::pos2(row.min.x + 8.0, row.center().y),
                        egui::Align2::LEFT_CENTER,
                        name,
                        FontId::proportional(12.0),
                        text_color,
                    );
                    if *count > 0 {
                        painter.text(
                            egui::pos2(row.max.x - 8.0, row.center().y),
                            egui::Align2::RIGHT_CENTER,
                            count.to_string(),
                            FontId::proportional(11.0),
                            Color32::from_rgb(0x86, 0x96, 0xA0),
                        );
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        if let Some(f) = on_poll_option.take() {
                            f(name.clone());
                        }
                    }
                    y_offset += 28.0;
                }

                if poll.is_multi() {
                    let button = egui::Rect::from_min_size(
                        egui::pos2(left, y_offset),
                        Vec2::new(inner_width, 24.0),
                    );
                    let resp = ui.interact(
                        button,
                        ui.scope_id().with(("poll-submit", self.message.id.as_str())),
                        Sense::click(),
                    );
                    painter.rect_filled(
                        button,
                        6.0,
                        if resp.hovered() {
                            Color32::from_rgb(0x06, 0xC0, 0x8A)
                        } else {
                            Color32::from_rgb(0x00, 0x8A, 0x6E)
                        },
                    );
                    painter.text(
                        button.center(),
                        egui::Align2::CENTER_CENTER,
                        "Hlasovat",
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                    if resp.clicked() {
                        if let Some(f) = on_poll_submit.take() {
                            f();
                        }
                    }
                }
            }
            
            // Timestamp and status
            let time_str = format_message_time(self.message.timestamp);
            let status_text = if self.is_own {
                match self.message.status {
                    crate::state::MessageStatus::Sending => " ⏳".to_string(),
                    crate::state::MessageStatus::Sent => " ✓".to_string(),
                    crate::state::MessageStatus::Delivered => " ✓✓".to_string(),
                    crate::state::MessageStatus::Read => " ✓✓".to_string(),
                    crate::state::MessageStatus::Failed => " ✗".to_string(),
                }
            } else {
                String::new()
            };
            
            let time_color = if self.is_own { 
                Color32::from_rgba_unmultiplied(255, 255, 255, 180) 
            } else { 
                Color32::from_rgb(0x86, 0x96, 0xA0) 
            };
            
            painter.text(
                egui::pos2(bubble_rect.max.x - bubble_padding.x, bubble_rect.max.y - bubble_padding.y),
                egui::Align2::RIGHT_BOTTOM,
                format!(
                    "{}{}{}",
                    if self.message.edited { "upraveno · " } else { "" },
                    time_str,
                    status_text
                ),
                FontId::proportional(10.0),
                time_color,
            );

            // Reaction pill badge
            if has_reactions {
                let pill_width = (reaction_text.chars().count() as f32 * 10.0).max(24.0) + 12.0;
                let pill_height = 20.0;
                let pill_pos = if self.is_own {
                    egui::pos2(bubble_rect.max.x - pill_width - 10.0, bubble_rect.max.y - 8.0)
                } else {
                    egui::pos2(bubble_rect.min.x + 10.0, bubble_rect.max.y - 8.0)
                };
                let pill_rect = egui::Rect::from_min_size(pill_pos, Vec2::new(pill_width, pill_height));

                let pill_id = ui.scope_id().with(("pill_reaction", &self.message.id));
                let pill_resp = ui.interact(pill_rect, pill_id, Sense::click());

                let bg_color = if pill_resp.hovered() { Color32::from_rgb(0x2A, 0x39, 0x42) } else { Color32::from_rgb(0x20, 0x2C, 0x33) };
                painter.rect_filled(pill_rect, 10.0, bg_color);
                painter.text(
                    pill_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &reaction_text,
                    crate::theme::message_font(11.0),
                    Color32::WHITE,
                );

                if pill_resp.hovered() {
                    let mut tip = String::from("Reakce:\n");
                    for r in &self.message.reactions {
                        let sender_str = crate::util::format_jid(&r.sender);
                        tip.push_str(&format!("{}  {}\n", r.emoji, sender_str));
                    }
                    tip.push_str("\n(Kliknutím zobrazíte detail)");
                    pill_resp.clone().on_hover_text(tip.trim());
                }

                if pill_resp.clicked() {
                    if let Some(f) = self.on_view_reactions {
                        f(&self.message.id);
                    }
                }
            }
        }
        
        let mut on_react = self.on_react;
        let mut on_open_emoji_picker = self.on_open_emoji_picker;
        
        // Quick reaction button on hover
        let smile_center = if self.is_own {
            bubble_rect.left_center() + Vec2::new(-16.0, 0.0)
        } else {
            bubble_rect.right_center() + Vec2::new(16.0, 0.0)
        };
        let smile_rect = egui::Rect::from_center_size(smile_center, Vec2::splat(24.0));
        let smile_id = ui.scope_id().with(("smile_btn", &self.message.id));
        let popup_id = smile_id.with("popup");
        let is_popup_open = egui::Popup::is_id_open(ui.ctx(), popup_id);

        let hover_check_rect = bubble_rect.union(smile_rect).expand(8.0);
        let is_hovered = ui.rect_contains_pointer(hover_check_rect);

        if is_hovered || is_popup_open {
            let smile_resp = ui.interact(smile_rect, smile_id, Sense::click());
            let bg_alpha = if smile_resp.hovered() || is_popup_open { 180 } else { 120 };
            ui.painter().circle_filled(smile_center, 12.0, Color32::from_black_alpha(bg_alpha));
            ui.painter().text(smile_center, egui::Align2::CENTER_CENTER, "😊", FontId::proportional(12.5), Color32::WHITE);
            
            egui::Popup::menu(&smile_resp).show(|ui| {
                ui.label(RichText::new("Reagovat na zprávu").font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                ui.horizontal(|ui| {
                    for emoji in ["👍", "❤️", "😂", "😮", "😢", "🙏", "🔥", "🎉"] {
                        let has_this_reaction = self.message.reactions.iter().any(|r| r.emoji == emoji);
                        let btn = egui::Button::new(RichText::new(emoji).font(crate::theme::message_font(18.0)))
                            .fill(if has_this_reaction { Color32::from_rgb(0x00, 0x5C, 0x4B) } else { Color32::TRANSPARENT })
                            .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(btn).on_hover_text(if has_this_reaction { "Odebrat reakci" } else { "Reagovat" }).clicked() {
                            if let Some(f) = on_react.take() {
                                f(&self.message.id, if has_this_reaction { "" } else { emoji });
                            }
                            ui.close();
                        }
                    }

                    let plus_btn = egui::Button::new(RichText::new("➕").font(FontId::proportional(14.0)))
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(egui::CornerRadius::same(6));
                    if ui.add(plus_btn).on_hover_text("Další emoji...").clicked() {
                        if let Some(f) = on_open_emoji_picker.take() {
                            f(&self.message.id);
                        }
                        ui.close();
                    }
                });
            });
        }
        
        // Selection mode swallows the click, so tapping a bubble ticks it
        // instead of opening, playing or downloading the attachment.
        if self.selection_mode || self.highlight > 0.0 {
            if self.highlight > 0.0 || self.selected {
                let painter = ui.painter();
                // A ticked message keeps the steady tint it always had; the jump
                // flash rides on top of it and lets go as it fades.
                let tint = if self.selected { 1.0 } else { self.highlight };
                painter.rect_filled(
                    bubble_rect,
                    egui::CornerRadius::same(9),
                    crate::ui::animations::fade_color(
                        Color32::from_rgb(0x00, 0xA8, 0x84),
                        tint * 0.14,
                    ),
                );
                // The outline expands slightly while it fades out, which reads
                // as the flash letting go instead of blinking off.
                let spread = if self.selected {
                    1.0
                } else {
                    1.0 + 3.0 * (1.0 - self.highlight)
                };
                painter.rect_stroke(
                    bubble_rect.expand(spread),
                    egui::CornerRadius::same(10),
                    egui::Stroke::new(2.0, crate::ui::animations::fade_color(crate::theme::ACCENT, tint)),
                    egui::StrokeKind::Outside,
                );
            }
            if response.clicked() {
                if let Some(f) = self.on_select {
                    f(&self.message.id);
                }
            }
            return response;
        }

        // View-once attachments open through the viewer modal only.
        if self.message.view_once {
            return response;
        }

        // Handle click on media
        let is_audio_bubble = matches!(
            self.message.media_type,
            Some(crate::state::MediaType::Audio) | Some(crate::state::MediaType::Voice)
        );
        let is_video_bubble = matches!(self.message.media_type, Some(crate::state::MediaType::Video));
        if (response.clicked() && !video_scrub_used) || audio_clicked {
            match self.message.media_path.as_ref() {
                Some(path_str) => {
                    let p = std::path::PathBuf::from(path_str);
                    if !p.exists() {
                        if let Some(ref f) = self.on_download {
                            f(&self.message.id);
                        }
                    } else if is_audio_bubble {
                        if let Err(e) = crate::audio::toggle(&self.message.id, &p) {
                            crate::media::report_playback_error(ui.ctx(), e);
                        }
                        ui.ctx().request_repaint();
                    } else if is_video_bubble {
                        // Play in-app; falls back to the OS player only if ffmpeg is absent.
                        if let Err(e) = crate::video::toggle(&self.message.id, &p) {
                            crate::media::report_playback_error(ui.ctx(), e);
                            crate::media::open_file(&p);
                        }
                        ui.ctx().request_repaint();
                    } else {
                        crate::media::open_file(&p);
                    }
                }
                None => {
                    if self.message.media_type.is_some() {
                        if let Some(ref f) = self.on_download {
                            f(&self.message.id);
                        }
                    }
                }
            }
        }
        
        // Context menu for reply, react, open, download, etc.
        response.context_menu(|ui| {
            // Reaction picker row
            ui.label(RichText::new("Reagovat").font(FontId::proportional(11.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
            ui.horizontal(|ui| {
                for emoji in ["👍", "❤️", "😂", "😮", "😢", "🙏", "🔥", "🎉"] {
                    let has_this_reaction = self.message.reactions.iter().any(|r| r.emoji == emoji);
                    let btn = egui::Button::new(RichText::new(emoji).font(crate::theme::message_font(18.0)))
                        .fill(if has_this_reaction { Color32::from_rgb(0x00, 0x5C, 0x4B) } else { Color32::TRANSPARENT })
                        .corner_radius(egui::CornerRadius::same(6));
                    if ui.add(btn).on_hover_text(if has_this_reaction { "Odebrat reakci" } else { "Reagovat" }).clicked() {
                        if let Some(f) = on_react.take() {
                            f(&self.message.id, if has_this_reaction { "" } else { emoji });
                        }
                        ui.close();
                    }
                }

                let plus_btn = egui::Button::new(RichText::new("➕").font(FontId::proportional(14.0)))
                    .fill(Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(6));
                if ui.add(plus_btn).on_hover_text("Další emoji...").clicked() {
                    if let Some(f) = on_open_emoji_picker.take() {
                        f(&self.message.id);
                    }
                    ui.close();
                }
            });
            ui.separator();
            if let Some(ref path_str) = self.message.media_path {
                let p = std::path::PathBuf::from(path_str);
                if p.exists() {
                    if ui.button("Otevřít").clicked() {
                        crate::media::open_file(&p);
                        ui.close();
                    }
                    if ui.button("Zobrazit ve složce").clicked() {
                        crate::media::show_in_folder(&p);
                        ui.close();
                    }
                }
            } else if self.message.media_type.is_some() {
                if ui.button("Stáhnout").clicked() {
                    if let Some(ref f) = self.on_download {
                        f(&self.message.id);
                    }
                    ui.close();
                }
            }
            if ui.button("Reply").clicked() {
                if let Some(f) = self.on_reply {
                    f(&self.message.id);
                }
                ui.close();
            }
            let mut on_select = self.on_select;
            if on_select.is_some() && ui.button("Vybrat").clicked() {
                if let Some(f) = on_select.take() {
                    f(&self.message.id);
                }
                ui.close();
            }
            // Only plain text can be rewritten in place; media captions and
            // system-ish bubbles have no editable body.
            if self.is_own && self.message.media_type.is_none() && !self.message.text.is_empty() {
                let mut on_edit = self.on_edit;
                if ui.button("Upravit").clicked() {
                    if let Some(f) = on_edit.take() {
                        f(&self.message.id);
                    }
                    ui.close();
                }
            }
            let mut on_pin = self.on_pin;
            if on_pin.is_some() {
                ui.separator();
                if self.is_pinned {
                    if ui.button("📌 Odepnout").clicked() {
                        if let Some(f) = on_pin.take() {
                            f(&self.message.id, None);
                        }
                        ui.close();
                    }
                } else {
                    ui.label(
                        RichText::new("Připnout pro všechny")
                            .font(FontId::proportional(11.0))
                            .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                    );
                    for (label, secs) in [
                        ("📌 Na 24 hodin", 86_400u32),
                        ("📌 Na 7 dní", 604_800),
                        ("📌 Na 30 dní", 2_592_000),
                    ] {
                        if ui.button(label).clicked() {
                            if let Some(f) = on_pin.take() {
                                f(&self.message.id, Some(secs));
                            }
                            ui.close();
                        }
                    }
                }
            }
            if ui.button("Copy").clicked() {
                ui.copy_text(self.message.text.clone());
                ui.close();
            }
            if self.is_own {
                let mut on_del = self.on_delete;
                if ui.button("Delete for me").clicked() {
                    if let Some(f) = on_del.take() {
                        f(&self.message.id);
                    }
                    ui.close();
                }
                if ui.button("Delete for everyone").clicked() {
                    if let Some(f) = on_del.take() {
                        f(&self.message.id);
                    }
                    ui.close();
                }
            }
        });
        
        response
    }
}

/// Composer placeholder for compatibility
pub struct Composer;

impl Composer {
    pub fn new(_text: String) -> Self {
        Self
    }
}

/// QR code display widget
pub struct QrDisplay {
    pub code: String,
    pub size: f32,
}

impl QrDisplay {
    pub fn new(code: String) -> Self {
        Self { code, size: 256.0 }
    }
    
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl Widget for QrDisplay {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        // Generate QR code image
        let qr = qrcode::QrCode::new(self.code.as_bytes()).ok();
        let texture_id = if let Some(qr) = qr {
            let image = qr.render::<image::Luma<u8>>()
                .min_dimensions(self.size as u32, self.size as u32)
                .build();
            
            let pixels: Vec<Color32> = image.pixels()
                .map(|p| Color32::from_gray(p.0[0]))
                .collect();
            
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                &pixels.iter().flat_map(|c| c.to_array()).collect::<Vec<_>>(),
            );
            
            Some(ui.ctx().load_texture("qr_code", color_image, Default::default()))
        } else {
            None
        };
        
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(self.size), Sense::hover());
        
        if let Some(texture) = texture_id {
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        
        response
    }
}

/// Overlay timeline (play/pause, progress, time, scrubbing) drawn on top of the
/// video player that is currently open in a bubble.
///
/// Returns true when the pointer was used here, so the enclosing bubble does not
/// also interpret the same click as play/pause.
fn video_timeline(ui: &egui::Ui, message_id: &str, media_rect: egui::Rect) -> bool {
    const BAR_HEIGHT: f32 = 26.0;

    let bar = egui::Rect::from_min_max(
        egui::pos2(media_rect.left(), media_rect.bottom() - BAR_HEIGHT),
        media_rect.max,
    );
    let painter = ui.painter();
    painter.rect_filled(
        bar,
        egui::CornerRadius { nw: 0, ne: 0, sw: 8, se: 8 },
        Color32::from_black_alpha(150),
    );

    let paused = crate::video::is_paused();
    let position = crate::video::position();
    let total = crate::video::duration();
    let progress = crate::video::progress();

    // Play / pause.
    let button = egui::Rect::from_center_size(
        egui::pos2(bar.left() + 15.0, bar.center().y),
        Vec2::splat(22.0),
    );
    let button_resp = ui.interact(
        button,
        ui.scope_id().with((message_id, "video-toggle")),
        egui::Sense::click(),
    );
    if button_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if button_resp.clicked() {
        if paused {
            crate::video::resume();
        } else {
            crate::video::pause();
        }
    }
    painter.text(
        button.center(),
        egui::Align2::CENTER_CENTER,
        if paused { "\u{25B6}" } else { "\u{23F8}" },
        FontId::proportional(13.0),
        Color32::WHITE,
    );

    // Elapsed / total, right aligned.
    let label = format!(
        "{} / {}",
        crate::audio::format_duration(position),
        total
            .map(crate::audio::format_duration)
            .unwrap_or_else(|| "--:--".to_string()),
    );
    let galley = painter.layout_no_wrap(
        label,
        FontId::proportional(9.5),
        Color32::from_rgb(0xDF, 0xE5, 0xE7),
    );
    let label_width = galley.size().x;
    let label_left = bar.right() - 8.0 - label_width;
    painter.galley(
        egui::pos2(label_left, bar.center().y - galley.size().y / 2.0),
        galley,
        Color32::WHITE,
    );

    // Seek track.
    let track = egui::Rect::from_min_max(
        egui::pos2(button.right() + 9.0, bar.center().y - 2.0),
        egui::pos2(label_left - 10.0, bar.center().y + 2.0),
    );
    if track.width() < 24.0 {
        return button_resp.clicked();
    }

    painter.rect_filled(track, 2.0, Color32::from_white_alpha(55));
    let filled = egui::Rect::from_min_max(
        track.min,
        egui::pos2(track.left() + track.width() * progress, track.max.y),
    );
    painter.rect_filled(filled, 2.0, crate::theme::ACCENT);
    painter.circle_filled(
        egui::pos2(track.left() + track.width() * progress, track.center().y),
        4.5,
        Color32::WHITE,
    );

    let hit = track.expand2(Vec2::new(0.0, 8.0));
    let resp = ui.interact(
        hit,
        ui.scope_id().with((message_id, "video-track")),
        egui::Sense::click_and_drag(),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let mut used = button_resp.clicked();
    if (resp.clicked() || resp.dragged()) && total.is_some() {
        if let (Some(pointer), Some(len)) = (resp.interact_pointer_pos(), total) {
            let fraction = ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0);
            crate::video::seek(len.mul_f32(fraction));
            used = true;
        }
    }
    if used {
        ui.ctx().request_repaint();
    }
    used
}

#[cfg(test)]
mod timeline_tests {
    use super::*;

    fn render(width: f32, height: f32, message_id: &str) -> bool {
        let ctx = egui::Context::default();
        let mut used = false;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let rect = egui::Rect::from_min_size(
                egui::pos2(10.0, 10.0),
                egui::vec2(width, height),
            );
            used = video_timeline(ui, message_id, rect);
        });
        // Nothing is uploading textures in a headless test.
        output.textures_delta.clear();
        used
    }

    /// The timeline runs on every frame while a video is open, so it must not
    /// panic or claim input when nothing is playing / nothing is hovered.
    #[test]
    fn timeline_renders_without_an_open_player() {
        assert!(!render(220.0, 160.0, "no-such-message"));
    }

    /// Very narrow bubbles cannot fit a track; it must degrade to just the button.
    #[test]
    fn timeline_degrades_on_narrow_bubbles() {
        assert!(!render(120.0, 80.0, "narrow"));
    }
}

#[cfg(test)]
mod bubble_tests {
    use super::*;
    use crate::state::{Message, MessageStatus};

    fn message(body: &str, from_me: bool) -> Message {
        let jid = whatsapp_rust::wacore_binary::Jid::pn("420777888999");
        Message::new_text(
            "msg-1".into(),
            jid.clone(),
            jid,
            body.to_string(),
            chrono::Utc::now(),
            from_me,
            MessageStatus::Sent,
            None,
            None,
        )
    }

    fn render(message: &Message, is_own: bool) {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.set_max_width(360.0);
            MessageBubble::new(message, is_own).ui(ui);
        });
        output.textures_delta.clear();
    }

    /// The edit marker is painted into the already-measured meta row, so a long
    /// body must still lay out without panicking.
    #[test]
    fn edited_bubble_renders() {
        let mut msg = message("opravený text", true);
        msg.edited = true;
        render(&msg, true);
    }

    #[test]
    fn unedited_bubble_renders() {
        render(&message("plain", false), false);
    }

    /// Selection mode intercepts the click and paints an outline; it must lay
    /// out in both states without touching the media path.
    #[test]
    fn selection_highlight_renders() {
        let ctx = egui::Context::default();
        for selected in [true, false] {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                ui.set_max_width(360.0);
                MessageBubble::new(&message("tick me", false), false)
                    .selection_mode(true)
                    .selected(selected)
                    .ui(ui);
            });
            output.textures_delta.clear();
        }
    }

    /// The jump flash and the arrival fade are intensities, so partial values
    /// have to lay out as happily as 0.0 and 1.0 do.
    #[test]
    fn animated_bubble_states_render() {
        let ctx = egui::Context::default();
        for step in [0.0, 0.35, 1.0] {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                ui.set_max_width(360.0);
                MessageBubble::new(&message("flash me", false), false)
                    .highlight(step)
                    .entrance(step)
                    .ui(ui);
            });
            output.textures_delta.clear();
        }
    }

}

#[cfg(test)]
mod preview_tests {
    use super::preview_label;
    use crate::state::{MediaType, MessagePreview, MessageStatus};
    use chrono::Utc;

    fn preview(text: &str, media_type: Option<MediaType>) -> MessagePreview {
        MessagePreview {
            text: text.to_string(),
            timestamp: Utc::now(),
            from_me: false,
            status: MessageStatus::Sent,
            media_type,
        }
    }

    #[test]
    fn plain_text_is_untouched() {
        assert_eq!(preview_label(&preview("ahoj", None)), "ahoj");
    }

    #[test]
    fn empty_text_without_media_is_a_generic_message() {
        assert_eq!(preview_label(&preview("", None)), "Zpráva");
        assert_eq!(
            preview_label(&preview("[Protocol Message]", None)),
            "Zpráva"
        );
    }

    #[test]
    fn empty_text_with_media_names_the_type() {
        assert_eq!(
            preview_label(&preview("", Some(MediaType::Image))),
            "📷 Zpráva"
        );
        assert_eq!(
            preview_label(&preview("[Protocol Message]", Some(MediaType::Voice))),
            "🎤 Zpráva"
        );
    }

    #[test]
    fn media_caption_gets_one_icon_prefix() {
        assert_eq!(
            preview_label(&preview("ahoj", Some(MediaType::Image))),
            "📷 ahoj"
        );
    }

    #[test]
    fn already_labelled_media_is_not_prefixed_twice() {
        // The backend turns caption-less media into "📷 Image" and friends.
        assert_eq!(
            preview_label(&preview("📷 Image", Some(MediaType::Image))),
            "📷 Image"
        );
        assert_eq!(
            preview_label(&preview("🎤 Voice Message", Some(MediaType::Voice))),
            "🎤 Voice Message"
        );
    }

    #[test]
    fn reactions_are_named_not_rendered_as_payload() {
        assert_eq!(
            preview_label(&preview("[Reaction] 👍", None)),
            "Reakce"
        );
    }
}

/// Lays out message text, tinting `@jméno` tokens the way WhatsApp does.
///
/// The token is only a display cue here — what actually goes on the wire is
/// `ContextInfo.mentioned_jid`, built when the message is sent.
fn mention_layout_job(
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
    wrap_width: f32,
) -> egui::text::LayoutJob {
    let plain = egui::TextFormat {
        font_id: font.clone(),
        color,
        ..Default::default()
    };
    let accent = egui::TextFormat {
        font_id: font.clone(),
        color: crate::theme::ACCENT,
        ..Default::default()
    };

    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;

    let mut rest = text;
    while let Some(at) = rest.find('@') {
        if at > 0 {
            job.append(&rest[..at], 0.0, plain.clone());
        }
        let after = &rest[at + 1..];
        let len = after.find(|c: char| c.is_whitespace()).unwrap_or(after.len());
        if len == 0 {
            job.append("@", 0.0, plain.clone());
            rest = after;
            continue;
        }
        job.append(&format!("@{}", &after[..len]), 0.0, accent.clone());
        rest = &after[len..];
    }
    if !rest.is_empty() {
        job.append(rest, 0.0, plain);
    }
    job
}
