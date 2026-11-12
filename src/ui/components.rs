//! UI Components - Shared reusable components

use egui::{Color32, FontId, RichText, Sense, Stroke, Ui, Vec2, Widget};

use crate::theme::{bubble, chat_list, status};
use crate::util::{format_timestamp, format_message_time, initials, truncate};

/// Status indicator icon
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
}

impl<'a> ChatItem<'a> {
    pub fn new(chat: &'a crate::state::Chat, is_selected: bool) -> Self {
        Self {
            chat,
            is_selected,
            on_click: None,
        }
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
        
        if self.is_selected {
            ui.painter().rect_filled(rect, 0.0, chat_list::SELECTED);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, chat_list::HOVER);
        } else {
            ui.painter().rect_filled(rect, 0.0, chat_list::BACKGROUND);
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
            
            let display_ts = self.chat.last_message.as_ref().map(|m| m.timestamp)
                .or_else(|| {
                    if self.chat.last_activity > chrono::DateTime::<chrono::Utc>::UNIX_EPOCH {
                        Some(self.chat.last_activity)
                    } else {
                        None
                    }
                });

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
            } else if let Some(preview) = &self.chat.last_message {
                let clean_text = if preview.text == "[Protocol Message]" || preview.text.is_empty() {
                    "Zpráva".to_string()
                } else if preview.text.starts_with("[Reaction]") {
                    "Reakce".to_string()
                } else {
                    preview.text.clone()
                };

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
    pub on_delete: Option<Box<dyn FnOnce(&'a str) + 'a>>,
    pub on_download: Option<Box<dyn Fn(&'a str) + 'a>>,
    pub on_react: Option<Box<dyn FnOnce(&'a str, &'a str) + 'a>>,
    pub on_view_reactions: Option<Box<dyn FnOnce(&'a str) + 'a>>,
    pub on_open_emoji_picker: Option<Box<dyn FnOnce(&'a str) + 'a>>,
}

impl<'a> MessageBubble<'a> {
    pub fn new(message: &'a crate::state::Message, is_own: bool) -> Self {
        Self {
            message,
            is_own,
            show_sender_name: false,
            sender_name: None,
            on_reply: None,
            on_delete: None,
            on_download: None,
            on_react: None,
            on_view_reactions: None,
            on_open_emoji_picker: None,
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
            Some(ui.painter().layout(
                display_text,
                FontId::proportional(font_size),
                text_color,
                text_wrap_width,
            ))
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

        let content_width = {
            let tw = galley.as_ref().map_or(0.0, |g| g.size().x);
            (tw.max(media_size.x) + bubble_padding.x * 2.0).max(110.0)
        };
        let bubble_width = content_width.min(max_bubble_width);
        let bubble_height = bubble_padding.y * 2.0 + sender_height + reply_height + media_height + text_height + time_height + 4.0;
        
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
                        if let Some((ref tex, _)) = cached_texture {
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
                        
                        let btn_center = media_rect.min + Vec2::new(22.0, media_rect.height() / 2.0);
                        painter.circle_filled(btn_center, 15.0, Color32::from_rgb(0x00, 0xA8, 0x84));
                        painter.text(
                            btn_center + Vec2::new(if is_downloaded { 1.0 } else { 0.0 }, 0.0),
                            egui::Align2::CENTER_CENTER,
                            if is_downloaded { "▶" } else { "⬇" },
                            FontId::proportional(13.0),
                            Color32::WHITE,
                        );
                        
                        let line_start = media_rect.min + Vec2::new(46.0, media_rect.height() / 2.0);
                        let line_end = media_rect.max + Vec2::new(-60.0, -media_rect.height() / 2.0);
                        painter.line_segment([line_start, line_end], Stroke::new(2.5, Color32::from_rgb(0x00, 0xA8, 0x84)));
                        
                        let sub = if is_downloaded {
                            "Přehrát".to_string()
                        } else if file_len > 0 {
                            crate::media::format_file_size(file_len)
                        } else {
                            "Hlasová zpráva".to_string()
                        };
                        painter.text(
                            media_rect.max + Vec2::new(-8.0, -media_rect.height() / 2.0),
                            egui::Align2::RIGHT_CENTER,
                            sub,
                            FontId::proportional(10.0),
                            Color32::from_rgb(0x86, 0x96, 0xA0),
                        );
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
                format!("{}{}", time_str, status_text),
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
                    FontId::proportional(11.0),
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
                        let btn = egui::Button::new(RichText::new(emoji).font(FontId::proportional(18.0)))
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
        
        // Handle click on media
        if response.clicked() {
            if let Some(ref path_str) = self.message.media_path {
                let p = std::path::PathBuf::from(path_str);
                if p.exists() {
                    crate::media::open_file(&p);
                } else if let Some(ref f) = self.on_download {
                    f(&self.message.id);
                }
            } else if self.message.media_type.is_some() {
                if let Some(ref f) = self.on_download {
                    f(&self.message.id);
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
                    let btn = egui::Button::new(RichText::new(emoji).font(FontId::proportional(18.0)))
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