//! Ctrl+K chat switcher — type a name, press Enter, the chat opens.

use egui::{Color32, FontId, RichText};
use whatsapp_rust::wacore_binary::Jid;

use crate::backend::BackendHandle;
use crate::state::{AppState, SharedState};

/// Chats matching `query`, most recent conversation first.
pub fn palette_matches(state: &AppState, query: &str) -> Vec<(Jid, String)> {
    let needle = query.trim().to_lowercase();
    let mut rows: Vec<(Jid, String)> = state
        .chats
        .keys()
        .map(|jid| {
            let name = state
                .resolve_display_name(jid)
                .or_else(|| state.chats.get(jid).map(|chat| chat.name.clone()))
                .unwrap_or_else(|| crate::util::format_jid(jid));
            (jid.clone(), name)
        })
        .filter(|(_, name)| needle.is_empty() || name.to_lowercase().contains(&needle))
        .collect();

    rows.sort_by_key(|(jid, _)| {
        let latest = state
            .chats
            .get(jid)
            .map(|chat| state.get_chat_latest_timestamp(chat))
            .unwrap_or_default();
        std::cmp::Reverse(latest)
    });
    rows.truncate(8);
    rows
}

pub fn show_command_palette(
    ctx: &egui::Context,
    state: &SharedState,
    backend: &BackendHandle,
) {
    let mut s = state.write();
    if !s.show_command_palette {
        return;
    }

    let mut open = true;
    let mut open_chat: Option<Jid> = None;

    egui::Window::new(RichText::new("Přepnout chat").font(FontId::proportional(14.0)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 120.0))
        .default_width(420.0)
        .show(ctx, |ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut s.palette_query)
                    .hint_text("Jméno chatu…  (↑ ↓ výběr, Enter otevřít)")
                    .desired_width(ui.available_width()),
            );
            // Focus once the window appears so typing goes straight in.
            if !response.has_focus() && s.palette_index == 0 && s.palette_query.is_empty() {
                response.request_focus();
            }

            let rows = palette_matches(&s, &s.palette_query.clone());
            if rows.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Nic nenalezeno.")
                        .font(FontId::proportional(12.0))
                        .color(Color32::from_rgb(0x86, 0x96, 0xA0)),
                );
                return;
            }

            let (up, down, enter) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowDown),
                    i.key_pressed(egui::Key::Enter),
                )
            });
            if up {
                s.palette_index = s.palette_index.saturating_sub(1);
            }
            if down {
                s.palette_index = (s.palette_index + 1).min(rows.len() - 1);
            }
            s.palette_index = s.palette_index.min(rows.len() - 1);
            if enter {
                open_chat = Some(rows[s.palette_index].0.clone());
            }

            ui.add_space(6.0);
            ui.separator();
            for (index, (jid, name)) in rows.iter().enumerate() {
                let selected = index == s.palette_index;
                let text = RichText::new(name).font(FontId::proportional(13.0));
                let label = if selected {
                    text.color(Color32::WHITE).strong()
                } else {
                    text.color(Color32::from_rgb(0xD1, 0xD7, 0xDB))
                };
                let response = ui.add(
                    egui::Button::new(label)
                        .fill(if selected {
                            Color32::from_rgb(0x00, 0x5C, 0x4B)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(ui.available_width(), 28.0)),
                );
                if response.clicked() {
                    open_chat = Some(jid.clone());
                }
            }
        });

    if let Some(jid) = open_chat {
        s.show_command_palette = false;
        s.palette_query.clear();
        s.palette_index = 0;
        crate::ui::open_chat(&mut s, &jid, backend);
    }
    if !open {
        s.show_command_palette = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::wacore_binary::Jid;

    fn jid(s: &str) -> Jid {
        s.parse().unwrap()
    }

    fn state_with_chats() -> AppState {
        let mut state = AppState::default();
        for (addr, name) in [
            ("111@s.whatsapp.net", "Anna Nováková"),
            ("222@s.whatsapp.net", "Petr"),
            ("333@g.us", "Rodina"),
        ] {
            let chat_jid = jid(addr);
            state.chats.insert(
                chat_jid.clone(),
                crate::state::Chat {
                    jid: chat_jid.clone(),
                    name: name.to_string(),
                    last_message: None,
                    unread_count: 0,
                    is_group: addr.ends_with("g.us"),
                    is_archived: false,
                    is_pinned: false,
                    mute_until: None,
                    last_activity: chrono::Utc::now(),
                    participants: std::collections::HashSet::new(),
                    is_typing: false,
                    typing_until: None,
                },
            );
            state
                .contacts
                .insert(chat_jid.clone(), crate::state::Contact::new(chat_jid, Some(name.to_string())));
        }
        state
    }

    #[test]
    fn palette_matches_by_name_substring_case_insensitively() {
        let state = state_with_chats();
        let hits = palette_matches(&state, "nov");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, "Anna Nováková");
    }

    #[test]
    fn empty_query_lists_everything_and_caps_at_eight() {
        let state = state_with_chats();
        assert_eq!(palette_matches(&state, "   ").len(), 3);
    }
}
