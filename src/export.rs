//! Conversation Export module - TXT, JSON, and HTML formats

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::Result;
use chrono::Utc;
use whatsapp_rust::wacore_binary::Jid;
use crate::state::{Chat, Contact, Message};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Txt,
    Json,
    Html,
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Json => "json",
            Self::Html => "html",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Txt => "Text (.txt)",
            Self::Json => "JSON (.json)",
            Self::Html => "HTML Webpage (.html)",
        }
    }
}

/// Generate plain text WhatsApp export
pub fn export_to_txt(
    chat: &Chat,
    messages: &[Message],
    contacts: &HashMap<Jid, Contact>,
    self_name: &str,
) -> String {
    let mut out = String::new();
    out.push_str("============================================================\n");
    out.push_str(&format!(" WhatsApp Chat Export: {}\n", chat.name));
    out.push_str(&format!(" JID: {}\n", chat.jid));
    out.push_str(&format!(" Exported at: {}\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
    out.push_str(&format!(" Total messages: {}\n", messages.len()));
    out.push_str("============================================================\n\n");

    for msg in messages {
        let sender = if msg.from_me {
            self_name
        } else {
            contacts.get(&msg.sender_jid)
                .and_then(|c| c.name.as_deref().or(c.push_name.as_deref()))
                .unwrap_or(&chat.name)
        };

        let time_str = msg.timestamp.format("%Y-%m-%d %H:%M:%S");
        let content = if let Some(media) = msg.media_type {
            let label = match media {
                crate::state::MediaType::Image => "📷 Image",
                crate::state::MediaType::Video => "📹 Video",
                crate::state::MediaType::Audio => "🎵 Audio",
                crate::state::MediaType::Voice => "🎤 Voice Message",
                crate::state::MediaType::Document => "📄 Document",
                crate::state::MediaType::Sticker => "🎨 Sticker",
            };
            if msg.text.is_empty() {
                label.to_string()
            } else {
                format!("{} {}", label, msg.text)
            }
        } else {
            msg.text.clone()
        };

        out.push_str(&format!("[{}] {}: {}\n", time_str, sender, content));
    }

    out
}

/// Generate JSON export
pub fn export_to_json(
    chat: &Chat,
    messages: &[Message],
    contacts: &HashMap<Jid, Contact>,
    self_name: &str,
) -> Result<String> {
    #[derive(serde::Serialize)]
    struct JsonExportMessage<'a> {
        id: &'a str,
        timestamp: String,
        sender_jid: String,
        sender_name: &'a str,
        from_me: bool,
        text: &'a str,
        media_type: Option<String>,
        status: String,
    }

    #[derive(serde::Serialize)]
    struct JsonExportRoot<'a> {
        chat_jid: String,
        chat_name: &'a str,
        is_group: bool,
        exported_at: String,
        message_count: usize,
        messages: Vec<JsonExportMessage<'a>>,
    }

    let items: Vec<JsonExportMessage> = messages.iter().map(|msg| {
        let sender = if msg.from_me {
            self_name
        } else {
            contacts.get(&msg.sender_jid)
                .and_then(|c| c.name.as_deref().or(c.push_name.as_deref()))
                .unwrap_or(&chat.name)
        };

        JsonExportMessage {
            id: &msg.id,
            timestamp: msg.timestamp.to_rfc3339(),
            sender_jid: msg.sender_jid.to_string(),
            sender_name: sender,
            from_me: msg.from_me,
            text: &msg.text,
            media_type: msg.media_type.map(|m| format!("{:?}", m)),
            status: format!("{:?}", msg.status),
        }
    }).collect();

    let root = JsonExportRoot {
        chat_jid: chat.jid.to_string(),
        chat_name: &chat.name,
        is_group: chat.is_group,
        exported_at: Utc::now().to_rfc3339(),
        message_count: items.len(),
        messages: items,
    };

    Ok(serde_json::to_string_pretty(&root)?)
}

/// Generate interactive dark-mode HTML export
pub fn export_to_html(
    chat: &Chat,
    messages: &[Message],
    contacts: &HashMap<Jid, Contact>,
    self_name: &str,
) -> String {
    let mut msgs_html = String::new();
    let mut last_date: Option<chrono::NaiveDate> = None;

    for msg in messages {
        let date = msg.timestamp.date_naive();
        if last_date != Some(date) {
            msgs_html.push_str(&format!(
                r#"<div class="date-divider"><span>{}</span></div>"#,
                date.format("%A, %B %d, %Y")
            ));
            last_date = Some(date);
        }

        let is_me = msg.from_me;
        let sender = if is_me {
            self_name
        } else {
            contacts.get(&msg.sender_jid)
                .and_then(|c| c.name.as_deref().or(c.push_name.as_deref()))
                .unwrap_or(&chat.name)
        };

        let row_class = if is_me { "row out" } else { "row in" };
        let bubble_class = if is_me { "bubble out" } else { "bubble in" };
        let time_str = msg.timestamp.format("%H:%M");

        let escaped_text = html_escape(&msg.text);
        let media_badge = if let Some(media) = msg.media_type {
            let icon = match media {
                crate::state::MediaType::Image => "📷 Photo",
                crate::state::MediaType::Video => "📹 Video",
                crate::state::MediaType::Audio => "🎵 Audio",
                crate::state::MediaType::Voice => "🎤 Voice",
                crate::state::MediaType::Document => "📄 Document",
                crate::state::MediaType::Sticker => "🎨 Sticker",
            };
            format!(r#"<div class="media-badge">{}</div>"#, icon)
        } else {
            String::new()
        };

        let sender_header = if !is_me && chat.is_group {
            format!(r#"<div class="sender-name">{}</div>"#, html_escape(sender))
        } else {
            String::new()
        };

        msgs_html.push_str(&format!(
            r#"<div class="{row_class}">
  <div class="{bubble_class}">
    {sender_header}
    {media_badge}
    <div class="text">{escaped_text}</div>
    <div class="meta"><span class="time">{time_str}</span></div>
  </div>
</div>"#
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Chat Export: {}</title>
  <style>
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      background-color: #0b141a;
      color: #e9edef;
      display: flex;
      justify-content: center;
      min-height: 100vh;
      padding: 20px;
    }}
    .chat-wrapper {{
      width: 100%;
      max-width: 800px;
      background: #111b21;
      border-radius: 12px;
      overflow: hidden;
      box-shadow: 0 4px 20px rgba(0,0,0,0.5);
      display: flex;
      flex-direction: column;
    }}
    .chat-header {{
      background: #202c33;
      padding: 16px 20px;
      display: flex;
      justify-content: space-between;
      align-items: center;
      border-bottom: 1px solid #2a3942;
    }}
    .chat-title {{ font-size: 1.15rem; font-weight: 600; color: #e9edef; }}
    .chat-subtitle {{ font-size: 0.8rem; color: #8696a0; margin-top: 2px; }}
    .badge {{ background: #00a884; color: #111b21; font-weight: 600; font-size: 0.75rem; padding: 3px 8px; border-radius: 10px; }}
    .chat-messages {{
      padding: 20px;
      display: flex;
      flex-direction: column;
      gap: 6px;
      background-image: radial-gradient(#1f2c34 1px, transparent 1px);
      background-size: 20px 20px;
    }}
    .date-divider {{
      text-align: center;
      margin: 14px 0 8px;
    }}
    .date-divider span {{
      background: #182229;
      color: #8696a0;
      font-size: 0.75rem;
      padding: 5px 12px;
      border-radius: 8px;
    }}
    .row {{ display: flex; width: 100%; }}
    .row.out {{ justify-content: flex-end; }}
    .row.in {{ justify-content: flex-start; }}
    .bubble {{
      max-width: 75%;
      padding: 8px 12px;
      border-radius: 8px;
      position: relative;
      word-break: break-word;
      font-size: 0.92rem;
      line-height: 1.4;
    }}
    .bubble.in {{ background: #202c33; color: #e9edef; border-top-left-radius: 2px; }}
    .bubble.out {{ background: #005c4b; color: #e9edef; border-top-right-radius: 2px; }}
    .sender-name {{ font-size: 0.78rem; font-weight: bold; color: #53bdeb; margin-bottom: 3px; }}
    .media-badge {{ font-size: 0.8rem; background: rgba(0,0,0,0.25); padding: 3px 8px; border-radius: 6px; display: inline-block; margin-bottom: 5px; }}
    .text {{ white-space: pre-wrap; }}
    .meta {{ display: flex; justify-content: flex-end; align-items: center; gap: 4px; margin-top: 4px; }}
    .time {{ font-size: 0.68rem; color: #8696a0; }}
    .chat-footer {{
      background: #202c33;
      padding: 12px 20px;
      text-align: center;
      font-size: 0.75rem;
      color: #8696a0;
      border-top: 1px solid #2a3942;
    }}
  </style>
</head>
<body>
  <div class="chat-wrapper">
    <div class="chat-header">
      <div>
        <div class="chat-title">{}</div>
        <div class="chat-subtitle">Exported from Warust WhatsApp Client</div>
      </div>
      <div class="badge">{} messages</div>
    </div>
    <div class="chat-messages">
      {}
    </div>
    <div class="chat-footer">
      Generated on {} • End-to-end encrypted backup
    </div>
  </div>
</body>
</html>"#,
        html_escape(&chat.name),
        html_escape(&chat.name),
        messages.len(),
        msgs_html,
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Get default directory for exports
pub fn default_export_dir() -> PathBuf {
    let base = dirs::document_dir()
        .or_else(dirs::download_dir)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    base.join("Warust_Exports")
}

/// Sanitize filename for operating system
pub fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = sanitized.trim().trim_matches('.');
    if trimmed.is_empty() {
        "chat_export".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Save export to file and return final path
pub fn save_export(
    chat: &Chat,
    messages: &[Message],
    contacts: &HashMap<Jid, Contact>,
    self_name: &str,
    format: ExportFormat,
    custom_dir: Option<&Path>,
) -> Result<PathBuf> {
    let dir = custom_dir.map(PathBuf::from).unwrap_or_else(default_export_dir);
    std::fs::create_dir_all(&dir)?;

    let date_str = Utc::now().format("%Y%m%d_%H%M%S");
    let safe_name = sanitize_filename(&chat.name);
    let filename = format!("{}_{}.{}", safe_name, date_str, format.extension());
    let file_path = dir.join(filename);

    let content = match format {
        ExportFormat::Txt => export_to_txt(chat, messages, contacts, self_name),
        ExportFormat::Json => export_to_json(chat, messages, contacts, self_name)?,
        ExportFormat::Html => export_to_html(chat, messages, contacts, self_name),
    };

    std::fs::write(&file_path, content)?;
    Ok(file_path)
}

/// Open folder or file in Windows Explorer
pub fn open_in_file_manager(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        if path.is_file() {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,\"{}\"", path.display()))
                .spawn();
        } else {
            let _ = std::process::Command::new("explorer")
                .arg(path)
                .spawn();
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        let _ = std::process::Command::new("xdg-open")
            .arg(target)
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use crate::state::MessageStatus;

    fn sample_chat_and_messages() -> (Chat, Vec<Message>, HashMap<Jid, Contact>) {
        let chat_jid: Jid = "420737526533@s.whatsapp.net".parse().unwrap();
        let my_jid: Jid = "420123456789@s.whatsapp.net".parse().unwrap();

        let chat = Chat {
            jid: chat_jid.clone(),
            name: "Jan Novák".to_string(),
            last_message: None,
            unread_count: 0,
            is_group: false,
            is_archived: false,
            is_pinned: false,
            mute_until: None,
            last_activity: Utc::now(),
            participants: HashSet::new(),
        };

        let mut contacts = HashMap::new();
        contacts.insert(chat_jid.clone(), Contact::new(chat_jid.clone(), Some("Jan Novák".to_string())));

        let msgs = vec![
            Message::new_text(
                "msg1".to_string(),
                chat_jid.clone(),
                chat_jid.clone(),
                "Ahoj! Jak se máš?".to_string(),
                Utc::now(),
                false,
                MessageStatus::Read,
                None,
                None,
            ),
            Message::new_text(
                "msg2".to_string(),
                chat_jid.clone(),
                my_jid.clone(),
                "Ahoj, všechno v pořádku, díky!".to_string(),
                Utc::now(),
                true,
                MessageStatus::Sent,
                None,
                None,
            ),
        ];

        (chat, msgs, contacts)
    }

    #[test]
    fn test_export_txt() {
        let (chat, msgs, contacts) = sample_chat_and_messages();
        let txt = export_to_txt(&chat, &msgs, &contacts, "Me");
        assert!(txt.contains("WhatsApp Chat Export: Jan Novák"));
        assert!(txt.contains("Ahoj! Jak se máš?"));
        assert!(txt.contains("Ahoj, všechno v pořádku, díky!"));
    }

    #[test]
    fn test_export_json() {
        let (chat, msgs, contacts) = sample_chat_and_messages();
        let json_str = export_to_json(&chat, &msgs, &contacts, "Me").unwrap();
        assert!(json_str.contains("\"chat_name\": \"Jan Novák\""));
        assert!(json_str.contains("\"message_count\": 2"));
    }

    #[test]
    fn test_export_html() {
        let (chat, msgs, contacts) = sample_chat_and_messages();
        let html = export_to_html(&chat, &msgs, &contacts, "Me");
        assert!(html.contains("Jan Novák"));
        assert!(html.contains("Ahoj! Jak se máš?"));
        assert!(html.contains("bubble in"));
        assert!(html.contains("bubble out"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Jan / Novák : Chat?"), "Jan _ Novák _ Chat_");
        assert_eq!(sanitize_filename(""), "chat_export");
    }
}
