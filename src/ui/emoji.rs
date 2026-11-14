//! Categorized emoji dataset, search, and picker UI for Warust

use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, vec2};
use crate::state::AppState;

pub struct EmojiCategory {
    pub name: &'static str,
    pub icon: &'static str,
    pub emojis: &'static [&'static str],
}

pub const QUICK_REACTIONS: &[&str] = &[
    "👍", "❤️", "😂", "😮", "😢", "🙏", "🔥", "🎉"
];

pub const POPULAR_EMOJIS: &[&str] = &[
    "👍", "❤️", "😂", "🔥", "🙏", "🎉", "💀", "✨", "👏", "🥰", "😍", "🥺", "🤣", "😭", "💯", "👌"
];

pub const CATEGORIES: &[EmojiCategory] = &[
    EmojiCategory {
        name: "Smajlíci a emoce",
        icon: "😃",
        emojis: &[
            "😀", "😃", "😄", "😁", "😆", "😅", "🤣", "😂", "🙂", "🙃", "😉", "😊",
            "😇", "🥰", "😍", "🤩", "😘", "😗", "😚", "😙", "😋", "😛", "😜", "🤪",
            "😝", "🤑", "🤗", "🤭", "🤫", "🤔", "🤐", "🤨", "😐", "😑", "😶", "😏",
            "😒", "🙄", "😬", "🤥", "😌", "😔", "😪", "🤤", "😴", "😷", "🤒", "🤕",
            "🤢", "🤮", "🤧", "🥵", "🥶", "🥴", "😵", "🤯", "🤠", "🥳", "😎", "🤓",
            "🧐", "😕", "😟", "🙁", "😮", "😯", "😲", "😳", "🥺", "😦", "😧", "💀",
            "☠️", "👻", "💩", "🤡", "🤖", "🎃"
        ],
    },
    EmojiCategory {
        name: "Gesta a tělo",
        icon: "👋",
        emojis: &[
            "👋", "🤚", "🖐️", "✋", "🖖", "👌", "🤌", "🤏", "✌️", "🤞", "🫰", "🤟",
            "🤘", "🤙", "👈", "👉", "👆", "🖕", "👇", "☝️", "👍", "👎", "✊", "👊",
            "🤛", "🤜", "👏", "🙌", "🫶", "👐", "🤲", "🤝", "🙏", "✍️", "💅", "🤳",
            "💪", "🦵", "🦶", "👂", "👀", "👁️", "👅", "👄"
        ],
    },
    EmojiCategory {
        name: "Srdce a emoce",
        icon: "❤️",
        emojis: &[
            "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "🤎", "💔", "❣️", "💕",
            "💞", "💓", "💗", "💖", "💘", "💝", "💟", "💌", "💋", "💯", "💢", "💥",
            "💫", "✨", "🌟", "🔥"
        ],
    },
    EmojiCategory {
        name: "Zvířata a příroda",
        icon: "🐶",
        emojis: &[
            "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🐨", "🐯", "🦁", "🐮",
            "🐷", "🐸", "🐵", "🐔", "🐧", "🐦", "🐤", "🦆", "🦅", "🦉", "🐺", "🐴",
            "🦄", "🐝", "🐛", "🦋", "🐌", "🐞", "🐜", "🦀", "🐙", "🐠", "🐬", "🐳",
            "🐊", "🐢", "🐍", "🌲", "🌳", "🌴", "🌵", "🌿", "🍀", "🍁", "🍂", "🍄"
        ],
    },
    EmojiCategory {
        name: "Jídlo a nápoje",
        icon: "🍕",
        emojis: &[
            "🍏", "🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🍒", "🍑", "🍍",
            "🥥", "🥝", "🍅", "🥑", "🥦", "🌽", "🥕", "🍞", "🥐", "🥖", "🥨", "🧀",
            "🍗", "🥩", "🍔", "🍟", "🍕", "🌭", "🥪", "🌮", "🌯", "🍝", "🍜", "🍲",
            "🍣", "🍱", "🥟", "🍦", "🍧", "🍨", "🍩", "🍪", "🎂", "🍰", "🧁", "🍫",
            "🍬", "🍭", "🍿", "☕", "🍵", "🧃", "🥤", "🧋", "🍺", "🍻", "🥂", "🍷",
            "🥃", "🍸"
        ],
    },
    EmojiCategory {
        name: "Aktivity a sport",
        icon: "⚽",
        emojis: &[
            "⚽", "🏀", "🏈", "⚾", "🎾", "🏐", "🎱", "🏓", "🏸", "🏒", "🥊", "🥋",
            "🛹", "🛼", "⛸️", "🎿", "🏂", "🏋️", "🤼", "🤸", "🎯", "🎳", "🎮", "🎲",
            "♟️", "🏆", "🥇", "🥈", "🥉", "🏅", "🎖️", "🎫", "🎟️", "🎪", "🎨", "🎬",
            "🎤", "🎧", "🎼", "🎹", "🥁", "🎷", "🎺", "🎸", "🎻", "🎭"
        ],
    },
    EmojiCategory {
        name: "Cestování a místa",
        icon: "🚗",
        emojis: &[
            "🚗", "🚕", "🚙", "🚌", "🏎️", "🚓", "🚑", "🚒", "🚐", "🚚", "🚜", "🚲",
            "🛴", "🛵", "🏍️", "🚨", "🚔", "✈️", "🛫", "🛬", "🚀", "🛸", "🚁", "⛵",
            "🚤", "🛳️", "⚓", "🚧", "🚥", "🚦", "🛑", "🗺️", "🗿", "🗽", "🗼", "🏰",
            "🏟️", "🏖️", "🏝️", "🌋", "⛰️", "🏔️", "🏕️", "🏠", "🏢", "🏦", "🏥"
        ],
    },
    EmojiCategory {
        name: "Předměty a symboly",
        icon: "💡",
        emojis: &[
            "⌚", "📱", "📲", "💻", "⌨️", "🖥️", "🖨️", "💾", "📷", "📸", "📹", "🎥",
            "📞", "☎️", "📺", "📻", "🎙️", "⏱️", "⏰", "🕰️", "⏳", "💡", "🔦", "🕯️",
            "💸", "💵", "💰", "💳", "💎", "🔧", "🔨", "🛠️", "🔑", "🗝️", "🚪", "🎁",
            "🎈", "🎉", "🎊", "✉️", "📩", "📦", "🏷️", "📌", "📍", "✂️", "🖊️", "📝",
            "✏️", "🔍", "🔎", "🔒", "🔓", "🔔", "🔕", "📢", "📣"
        ],
    },
];

/// Simple keyword match for emoji searching (supports Czech & English keywords)
pub fn search_emojis(query: &str) -> Vec<&'static str> {
    let q = query.to_lowercase().trim().to_string();
    if q.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();

    // Check specific keywords
    if q.contains("srdc") || q.contains("heart") || q.contains("love") || q.contains("lask") {
        matches.extend_from_slice(&["❤️", "💖", "🥰", "😍", "💕", "💞", "💓", "💗", "💘", "💔"]);
    }
    if q.contains("smej") || q.contains("smich") || q.contains("laugh") || q.contains("lol") || q.contains("vtip") {
        matches.extend_from_slice(&["😂", "🤣", "😆", "😅", "😄", "😃", "😀"]);
    }
    if q.contains("palec") || q.contains("thumb") || q.contains("ok") || q.contains("dobre") || q.contains("super") {
        matches.extend_from_slice(&["👍", "👎", "👌", "✌️", "👏", "🙌", "🤝"]);
    }
    if q.contains("ohen") || q.contains("fire") || q.contains("plamen") || q.contains("hot") {
        matches.extend_from_slice(&["🔥", "💥", "✨", "⚡"]);
    }
    if q.contains("lebk") || q.contains("skull") || q.contains("smrt") || q.contains("dead") {
        matches.extend_from_slice(&["💀", "☠️", "👻"]);
    }
    if q.contains("plac") || q.contains("cry") || q.contains("smut") || q.contains("sad") || q.contains("slza") {
        matches.extend_from_slice(&["😢", "😭", "🥺", "😔", "😪"]);
    }
    if q.contains("modl") || q.contains("pray") || q.contains("pros") || q.contains("dek") || q.contains("thanks") {
        matches.extend_from_slice(&["🙏", "🙇", "✨"]);
    }
    if q.contains("part") || q.contains("oslav") || q.contains("slav") || q.contains("darek") || q.contains("gift") {
        matches.extend_from_slice(&["🎉", "🥳", "🎊", "🎁", "🎈"]);
    }
    if q.contains("pes") || q.contains("dog") || q.contains("kock") || q.contains("cat") || q.contains("zvire") {
        matches.extend_from_slice(&["🐶", "🐱", "🦊", "🐻", "🦁", "🐵", "🐸"]);
    }
    if q.contains("piv") || q.contains("beer") || q.contains("vino") || q.contains("wine") || q.contains("chlast") {
        matches.extend_from_slice(&["🍺", "🍻", "🥂", "🍷", "🥃", "🍸"]);
    }
    if q.contains("jidlo") || q.contains("food") || q.contains("pizza") || q.contains("burger") {
        matches.extend_from_slice(&["🍕", "🍔", "🍟", "🌭", "🥪", "🌮", "🍖", "🍗"]);
    }
    if q.contains("kafe") || q.contains("coffee") || q.contains("caj") || q.contains("tea") {
        matches.extend_from_slice(&["☕", "🍵", "🧋", "🥤"]);
    }
    if q.contains("auto") || q.contains("car") || q.contains("let") || q.contains("plane") {
        matches.extend_from_slice(&["🚗", "🏎️", "🚕", "🚌", "✈️", "🚀", "🏍️"]);
    }
    if q.contains("hvezd") || q.contains("star") || q.contains("svit") || q.contains("noc") {
        matches.extend_from_slice(&["⭐", "🌟", "✨", "💫", "🌙"]);
    }
    if q.contains("peniz") || q.contains("money") || q.contains("cash") || q.contains("bohat") {
        matches.extend_from_slice(&["💰", "💸", "💵", "💳", "🤑"]);
    }

    // Also check if any category name matches
    for cat in CATEGORIES {
        if cat.name.to_lowercase().contains(&q) {
            matches.extend_from_slice(cat.emojis);
        }
    }

    matches.dedup();
    matches
}

/// Renders the categorized Emoji Picker popup
pub fn show_emoji_picker(
    ui: &mut egui::Ui,
    state: &mut AppState,
    mut on_select: impl FnMut(&str),
) {
    let frame = egui::Frame::new()
        .fill(Color32::from_rgb(0x1F, 0x2C, 0x34))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x2A, 0x39, 0x42)))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(10));

    frame.show(ui, |ui| {
        ui.set_width(320.0);
        ui.set_height(340.0);

        // Search bar at the top
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").font(FontId::proportional(14.0)).color(Color32::from_rgb(0x86, 0x96, 0xA0)));
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut state.emoji_search)
                    .hint_text("Hledat emoji (srdce, oheň, pivo...)")
                    .desired_width(ui.available_width() - 28.0)
                    .font(FontId::proportional(13.0))
            );
            if !state.emoji_search.is_empty() {
                if ui.small_button("✕").clicked() {
                    state.emoji_search.clear();
                }
            }
            if search_resp.changed() {
                ui.ctx().request_repaint();
            }
        });

        ui.add_space(4.0);

        // Category Tab Icons (if not searching)
        if state.emoji_search.trim().is_empty() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (idx, cat) in CATEGORIES.iter().enumerate() {
                    let is_active = state.emoji_active_category == idx;
                    let tab_btn = egui::Button::new(
                        RichText::new(cat.icon).font(FontId::proportional(16.0))
                    )
                    .fill(if is_active { Color32::from_rgb(0x00, 0x80, 0x69) } else { Color32::TRANSPARENT })
                    .corner_radius(CornerRadius::same(6))
                    .min_size(vec2(32.0, 28.0));

                    if ui.add(tab_btn).on_hover_text(cat.name).clicked() {
                        state.emoji_active_category = idx;
                    }
                }
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
        }

        // Emoji display list
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(260.0)
            .show(ui, |ui| {
                let emojis_to_show: Vec<&'static str> = if !state.emoji_search.trim().is_empty() {
                    search_emojis(&state.emoji_search)
                } else {
                    let mut list = Vec::new();
                    // Add popular on top of first category
                    if state.emoji_active_category == 0 {
                        list.extend_from_slice(POPULAR_EMOJIS);
                    }
                    if let Some(cat) = CATEGORIES.get(state.emoji_active_category) {
                        list.extend_from_slice(cat.emojis);
                    }
                    list.dedup();
                    list
                };

                if emojis_to_show.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(RichText::new("Nenalezena žádná emoji").color(Color32::from_rgb(0x86, 0x96, 0xA0)));
                    });
                } else {
                    let item_size = vec2(34.0, 34.0);

                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                        for &emoji in &emojis_to_show {
                            let btn = egui::Button::new(
                                RichText::new(emoji).font(crate::theme::message_font(20.0))
                            )
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(CornerRadius::same(6))
                            .min_size(item_size);

                            let resp = ui.add(btn);
                            if resp.clicked() {
                                on_select(emoji);
                            }
                            // Smooth hover effect
                            if resp.hovered() {
                                ui.ctx().request_repaint();
                            }
                        }
                    });
                }
            });
    });
}
