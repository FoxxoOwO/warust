//! Message Search engine with multi-criteria filtering

use chrono::Utc;
use whatsapp_rust::wacore_binary::Jid;
use crate::state::{AppState, Message};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchScope {
    #[default]
    CurrentChat,
    AllChats,
}

impl SearchScope {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CurrentChat => "📌 This chat",
            Self::AllChats => "🌐 All chats",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchSender {
    #[default]
    All,
    SentByMe,
    Received,
}

impl SearchSender {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "👥 All senders",
            Self::SentByMe => "↗ Sent by me",
            Self::Received => "↙ Received",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchContentType {
    #[default]
    All,
    TextOnly,
    MediaOnly,
    LinksOnly,
}

impl SearchContentType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "📄 All content",
            Self::TextOnly => "💬 Text only",
            Self::MediaOnly => "📷 Media only",
            Self::LinksOnly => "🔗 Links only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchDateRange {
    #[default]
    AllTime,
    Today,
    Last7Days,
    Last30Days,
}

impl SearchDateRange {
    pub fn label(&self) -> &'static str {
        match self {
            Self::AllTime => "⏱ All time",
            Self::Today => "📅 Today",
            Self::Last7Days => "📅 Last 7 days",
            Self::Last30Days => "📅 Last 30 days",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MessageSearchFilter {
    pub query: String,
    pub scope: SearchScope,
    pub sender: SearchSender,
    pub content_type: SearchContentType,
    pub date_range: SearchDateRange,
}

impl MessageSearchFilter {
    pub fn is_empty(&self) -> bool {
        self.query.trim().is_empty()
            && self.sender == SearchSender::All
            && self.content_type == SearchContentType::All
            && self.date_range == SearchDateRange::AllTime
    }

    pub fn reset_filters(&mut self) {
        self.sender = SearchSender::All;
        self.content_type = SearchContentType::All;
        self.date_range = SearchDateRange::AllTime;
    }
}

#[derive(Debug, Clone)]
pub struct SearchResultItem {
    pub message: Message,
    pub chat_name: String,
    pub sender_name: String,
}

/// Search across stored messages according to active filters
pub fn search_messages(
    state: &AppState,
    filter: &MessageSearchFilter,
) -> Vec<SearchResultItem> {
    if filter.is_empty() {
        return Vec::new();
    }

    let query_lower = filter.query.trim().to_lowercase();
    let now = Utc::now();
    let mut results = Vec::new();

    // Determine target chats to scan
    let target_chats: Vec<&Jid> = match filter.scope {
        SearchScope::CurrentChat => {
            if let Some(ref selected) = state.selected_chat {
                vec![selected]
            } else {
                state.messages.keys().collect()
            }
        }
        SearchScope::AllChats => state.messages.keys().collect(),
    };

    for chat_jid in target_chats {
        if let Some(msgs) = state.messages.get(chat_jid) {
            let chat_name = state.chats.get(chat_jid)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| crate::util::format_jid(chat_jid));

            for msg in msgs {
                // Filter 1: Sender (Sent vs Received)
                match filter.sender {
                    SearchSender::All => {}
                    SearchSender::SentByMe if !msg.from_me => continue,
                    SearchSender::Received if msg.from_me => continue,
                    _ => {}
                }

                // Filter 2: Content Type
                match filter.content_type {
                    SearchContentType::All => {}
                    SearchContentType::TextOnly => {
                        if msg.media_type.is_some() {
                            continue;
                        }
                    }
                    SearchContentType::MediaOnly => {
                        if msg.media_type.is_none() {
                            continue;
                        }
                    }
                    SearchContentType::LinksOnly => {
                        let text_lower = msg.text.to_lowercase();
                        if !text_lower.contains("http://") && !text_lower.contains("https://") {
                            continue;
                        }
                    }
                }

                // Filter 3: Date Range
                match filter.date_range {
                    SearchDateRange::AllTime => {}
                    SearchDateRange::Today => {
                        if msg.timestamp.date_naive() != now.date_naive() {
                            continue;
                        }
                    }
                    SearchDateRange::Last7Days => {
                        if (now - msg.timestamp).num_days() > 7 {
                            continue;
                        }
                    }
                    SearchDateRange::Last30Days => {
                        if (now - msg.timestamp).num_days() > 30 {
                            continue;
                        }
                    }
                }

                // Filter 4: Text Query match
                if !query_lower.is_empty() {
                    let in_text = msg.text.to_lowercase().contains(&query_lower);
                    let in_caption = msg.media_caption.as_deref().map_or(false, |c| c.to_lowercase().contains(&query_lower));
                    if !in_text && !in_caption {
                        continue;
                    }
                }

                let sender_name = if msg.from_me {
                    "You".to_string()
                } else {
                    state.resolve_display_name(&msg.sender_jid)
                        .unwrap_or_else(|| crate::util::format_jid(&msg.sender_jid))
                };

                results.push(SearchResultItem {
                    message: msg.clone(),
                    chat_name: chat_name.clone(),
                    sender_name,
                });
            }
        }
    }

    // Sort newest first
    results.sort_by(|a, b| b.message.timestamp.cmp(&a.message.timestamp));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MessageStatus;

    #[test]
    fn test_search_messages_filter_query_and_sender() {
        let mut state = AppState::new();
        let chat_jid: Jid = "420737526533@s.whatsapp.net".parse().unwrap();
        let my_jid: Jid = "420123456789@s.whatsapp.net".parse().unwrap();

        state.get_or_create_chat(chat_jid.clone(), "Eva".to_string(), false);
        state.selected_chat = Some(chat_jid.clone());

        let msg1 = Message::new_text(
            "1".to_string(),
            chat_jid.clone(),
            chat_jid.clone(),
            "Ahoj, jak se daří v práci?".to_string(),
            Utc::now(),
            false,
            MessageStatus::Read,
            None,
            None,
        );
        let msg2 = Message::new_text(
            "2".to_string(),
            chat_jid.clone(),
            my_jid.clone(),
            "Skvěle! Posílám odkaz https://rust-lang.org".to_string(),
            Utc::now(),
            true,
            MessageStatus::Sent,
            None,
            None,
        );

        state.add_message(msg1);
        state.add_message(msg2);

        // Test 1: Query match
        let mut filter = MessageSearchFilter::default();
        filter.query = "práci".to_string();
        let res = search_messages(&state, &filter);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].message.id, "1");

        // Test 2: Filter sender SentByMe
        filter.query.clear();
        filter.sender = SearchSender::SentByMe;
        let res = search_messages(&state, &filter);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].message.id, "2");

        // Test 3: Filter links only
        filter.sender = SearchSender::All;
        filter.content_type = SearchContentType::LinksOnly;
        let res = search_messages(&state, &filter);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].message.id, "2");
    }
}
