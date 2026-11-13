# Warust — WhatsApp Desktop Client (Rust + egui)

## 1. Cíle a rozsah (MVP)

| Oblast | Rozsah MVP | Poznámka |
|--------|------------|----------|
| **UI** | Desktop (egui + eframe, nativní okno) | Chat list, conversation view, QR párování, nastavení |
| **Protokol** | `whatsapp-rust` crate (v0.7.0, bez `simd` feature) | QR/pair-code auth, Signal E2E, text 1:1 + groups, delivery/read receipts, history sync |
| **Perzistence** | SQLite (předdefinovaná `SqliteStore`) | Session, klíče, historie zpráv, kontakty |
| **Sítě** | Tokio + WebSocket + ureq HTTP | Automatické reconnect, keepalive, offline queue |
| **Média** | **NENÍ** v MVP (text only) | Později: upload/download, šifrování, náhledy |
| **Volání** | **NENÍ** v MVP | Vyžaduje `voip` feature + Opus/WebRTC |

## 2. Technologický stack

```
┌─────────────────────────────────────────────────────────────────┐
│ warust (bin)                                                    │
├─────────────────────────────────────────────────────────────────┤
│  UI:     egui 0.31 + eframe 0.37 (wgpu backend)                │
│  QR:     qrcode 0.14 (image feature) → egui::Texture           │
│  Async:  tokio 1.54 (rt-multi-thread, macros, time)            │
│  DB:     whatsapp-rust-sqlite-storage (via SqliteStore)        │
│  Core:   whatsapp-rust 0.7.0 (no-default-features,              │
│          features = ["signal","sqlite-storage",                │
│          "tokio-native","tokio-runtime","tokio-transport",     │
│          "ureq-client","whatsapp-rust-sqlite-storage"])        │
└─────────────────────────────────────────────────────────────────┘
```

**Závislosti (Cargo.toml):**
```toml
[package]
name = "warust"
version = "0.1.2"
edition = "2021"

[dependencies]
# WhatsApp core
whatsapp-rust = { version = "0.7.0", default-features = false, features = [
    "signal", "sqlite-storage", "tokio-native", "tokio-runtime",
    "tokio-transport", "ureq-client", "whatsapp-rust-sqlite-storage"
] }

# UI
eframe = { version = "0.37", default-features = false, features = ["wgpu", "persistence"] }
egui = "0.31"
egui_extras = "0.31"
qrcode = { version = "0.14", features = ["image"] }
image = "0.25"

# Async & utils
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time", "sync"] }
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Platform
dirs = "5.0"  # config/data directories
```

> ⚠️ **Důležité**: `whatsapp-rust` default feature `simd` vyžaduje **nightly** Rust. Build na stable vyžaduje `default-features = false` + explicitní feature list výše.

## 3. Architektura aplikace

### 3.1 Vrstvy

```
┌────────────────────────────────────────────┐
│  Presentation (egui)                       │
│  ┌─────────────┐ ┌───────────────────────┐ │
│  │ ChatList    │ │ ConversationView      │ │
│  │ - filters   │ │ - message bubbles     │ │
│  │ - search    │ │ - input composer      │ │
│  └─────────────┘ └───────────────────────┘ │
│  ┌─────────────┐ ┌───────────────────────┐ │
│  │ QRPairing   │ │ Settings / About      │ │
│  └─────────────┘ └───────────────────────┘ │
└────────────────────────────────────────────┘
                    │
                    ▼
┌────────────────────────────────────────────┐
│  App State (Arc<AppState>)                 │
│  - chats: HashMap<Jid, ChatState>          │
│  - messages: HashMap<Jid, Vec<Message>>    │
│  - contacts: HashMap<Jid, Contact>         │
│  - self_jid: Option<Jid>                   │
│  - connection: ConnectionStatus            │
│  - pending_actions: Vec<Action>            │
└────────────────────────────────────────────┘
                    │
         ┌──────────┴──────────┐
         ▼                     ▼
┌──────────────────┐  ┌──────────────────────┐
│  Bot Handle      │  │  Background Tasks    │
│  (BotHandle)     │  │  - history sync      │
│  - client()      │  │  - receipt dispatch  │
│  - shutdown()    │  │  - presence          │
└──────────────────┘  └──────────────────────┘
         │
         ▼
┌────────────────────────────────────────────┐
│  whatsapp-rust (Bot / Client)              │
│  - Event handlers: on_message, on_event    │
│  - Persistence: SqliteStore                │
│  - Transport: Tokio WebSocket              │
└────────────────────────────────────────────┘
```

### 3.2 Event Loop Integration

`whatsapp-rust` běží v **vlastním Tokio tasku** (`bot.spawn()` → `BotHandle`). UI thread (egui) komunikuje přes:
- **Shared state** (`Arc<RwLock<AppState>>`) — čtení chatů, zpráv, kontaktů
- **MPSC channel** — příkazy z UI do backendu (send_text, mark_read, archive_chat, …)
- **Tokio watch channel** — connection status, QR kód, auth state

```
UI Thread (egui)                    Backend Task (Tokio)
┌─────────────────────┐             ┌─────────────────────┐
│ AppState (RwLock)   │◄───────────►│ Bot event handlers  │
│  chats, messages    │   shared    │  on_message → push  │
│  contacts           │   state     │  to AppState        │
│                     │             │                     │
│ tx_command (mpsc)   │────────────►│ rx_command → exec   │
│  SendText {to, txt} │   commands  │  client.send_text() │
│  MarkRead {ids}     │             │                     │
│                     │             │ tx_status (watch)   │
│ rx_status (watch)   │◄────────────│  ConnectionStatus   │
│  Connected/QR/Error │   status    │  QRCode / PairCode  │
└─────────────────────┘             └─────────────────────┘
```

### 3.3 Datový model (AppState)

```rust
// src/state.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use whatsapp_rust::wacore_binary::Jid;
use whatsapp_rust::waproto::whatsapp as wa;

#[derive(Debug, Clone)]
pub struct Chat {
    pub jid: Jid,
    pub name: String,              // push_name / group subject / formatted PN
    pub last_message: Option<MessagePreview>,
    pub unread_count: u32,
    pub is_group: bool,
    pub is_archived: bool,
    pub is_pinned: bool,
    pub mute_until: Option<DateTime<Utc>>,
    pub last_activity: DateTime<Utc>,  // for sorting
}

#[derive(Debug, Clone)]
pub struct MessagePreview {
    pub text: String,              // truncated
    pub timestamp: DateTime<Utc>,
    pub from_me: bool,
    pub status: MessageStatus,     // Sent / Delivered / Read / Failed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    Sending,
    Sent,         // server ACK (stanza-id)
    Delivered,    // delivery receipt
    Read,         // read receipt
    Failed,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub id: String,                // message_id (stanza id)
    pub chat_jid: Jid,
    pub sender_jid: Jid,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub from_me: bool,
    pub status: MessageStatus,
    pub reply_to: Option<String>,  // quoted message id
    pub ephemeral_expiration: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Contact {
    pub jid: Jid,
    pub push_name: Option<String>,
    pub verified_name: Option<String>,
    pub profile_pic_url: Option<String>,
    pub is_business: bool,
    pub is_on_whatsapp: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    QrCode(String),        // QR data URI or raw code
    PairCode(String),      // 8-char code
    Connected,
    Syncing(f32),          // history sync progress 0.0–1.0
    Error(String),
}

#[derive(Default)]
pub struct AppState {
    pub chats: HashMap<Jid, Chat>,
    pub messages: HashMap<Jid, Vec<Message>>,   // per-chat message history
    pub contacts: HashMap<Jid, Contact>,
    pub self_jid: Option<Jid>,
    pub connection: ConnectionStatus,
    pub selected_chat: Option<Jid>,
    pub search_query: String,
}
```

## 4. Klíčové use-case a flow

### 4.1 První spuštění — QR párování

```
1. App start → AppState.connection = Connecting
2. Bot::builder()
     .with_backend(SqliteStore::new("sqlite:warust.db").await?)
     .on_qr_code(|code, _| tx_status.send(ConnectionStatus::QrCode(code)).ok())
     .on_connected(|_| tx_status.send(ConnectionStatus::Connected).ok())
     .on_event(|event, _| match event {       // history sync
         Event::HistorySync(h) => {
             let p = h.progress().unwrap_or(0) as f32 / 100.0;
             tx_status.send(ConnectionStatus::Syncing(p)).ok();
             // process conversations → populate AppState.chats/messages
         }
         Event::Messages(batch) => {          // live messages
             for msg in batch.iter() {
                 push_to_appstate(msg);
             }
         }
         Event::Receipt(r) => update_receipts(r),
         _ => {}
     })
     .build().await?
3. bot.spawn() → BotHandle
4. UI zobrazí QR kód (qrcode crate → egui::Texture)
5. Uživatel naskenuje → on_connected → Connected → history sync → Chats loaded
```

### 4.2 Odeslání zprávy (text)

```
UI: User types + Enter
    │
    ▼
tx_command.send(Command::SendText { to: jid, text })
    │
    ▼
Backend task: rx_command.recv()
    │
    ▼
client.send_text(&jid, &text).await?
    │
    ▼
SendResult { message_id, to } → update local message status = Sent
    │
    ▼ (async)
Event::Receipt(Delivered) → status = Delivered
Event::Receipt(Read)      → status = Read
```

### 4.3 Příjem zprávy

```
Server → WebSocket → whatsapp-rust decrypt → Event::Messages(batch)
    │
    ▼
on_message handler:
    - extract text, sender, timestamp
    - push Message to AppState.messages[chat_jid]
    - update Chat.last_message, unread_count, last_activity
    - if chat != selected_chat: play notification sound
    - client.mark_as_read(&chat_jid, sender_opt, &[msg_id]).await?  // auto read receipt
```

### 4.4 History Sync (initial bootstrap)

```
Event::HistorySync(lazy_sync)
    │
    ▼
lazy_sync.stream() → HistorySyncStream
    │
    ├─ next_conversation() → Conversation { id, messages[] }
    │       for msg in messages:
    │           decrypt if needed (already done by library)
    │           push to AppState
    │
    └─ remainder() → pushnames, mappings, etc.
```

> `LazyHistorySync::stream()` držel paměť nízkou — vždy jen jeden conversation v RAM.

## 5. UI specifikace (egui)

### 5.1 Layout (třípanelový na široké obrazovce)

```
┌────────────────────────────────────────────────────────────────┐
│  Header: [Warust] [Search...] [☰ Menu]                         │
├──────────┬──────────────────────────┬──────────────────────────┤
│          │                          │                          │
│ ChatList │   ConversationView       │  (Optional) Details      │
│          │                          │  - Contact info          │
│  [📌]    │  ┌────────────────────┐  │  - Media gallery         │
│  Chat 1  │  │ msg bubble (them)  │  │  - Search in chat        │
│  Chat 2  │  │ msg bubble (me)    │  │                          │
│  Chat 3  │  │ ...                │  │                          │
│  ...     │  ├────────────────────┤  │                          │
│          │  │ [📎] Type msg...  │  │                          │
│          │  │ [Send]             │  │                          │
│          │  └────────────────────┘  │                          │
└──────────┴──────────────────────────┴──────────────────────────┘
```

**Responzivita:**
- < 900px: jen ChatList + Conversation (přepínání `selected_chat`)
- ≥ 900px: třípanelový

### 5.2 Komponenty

| Komponenta | Popis | Kľúčové funkce |
|------------|-------|----------------|
| `ChatList` | Scrollable list, virtuální scroll pro 1000+ chatů | `ui.allocate_ui_at_rect`, `egui_extras::Strip` |
| `ChatItem` | Jeden řádek: avatar, name, preview, timestamp, unread badge | `SelectableLabel`, `Image::new(texture)` |
| `ConversationView` | Messages + composer | `ScrollArea::vertical().stick_to_bottom(true)` |
| `MessageBubble` | In/out styl, čas, status ikona (✓/✓✓/👁) | `Frame::none().fill(color).rounding(12)` |
| `Composer` | Multiline `TextEdit`, Enter=send, Shift+Enter=newline | `TextEdit::multiline().desired_rows(1..4)` |
| `QrPairingScreen` | Full-screen QR + instrukce + pair-code fallback | `CentralPanel`, `Image::from_texture` |
| `SettingsDialog` | Modal: account, notifications, privacy, about | `Window::new("Settings").modal(true)` |

### 5.3 Téma a barvy

```rust
// src/theme.rs
pub fn setup_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    
    // WhatsApp-like green accent
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0x00, 0x80, 0x69);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(0xDC, 0xF8, 0xC6);
    style.visuals.selection.stroke = egui::Stroke::NONE;
    
    // Dark mode default
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = egui::Color32::from_rgb(0x11, 0x1B, 0x21);
    style.visuals.window_fill = egui::Color32::from_rgb(0x1F, 0x2C, 0x34);
    
    ctx.set_style(style);
    
    // Fonty
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "noto".into(),
        egui::FontData::from_static(include_bytes!("../assets/NotoSans-Regular.ttf"))
    );
    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
        .insert(0, "noto".into());
    ctx.set_fonts(fonts);
}
```

## 6. Persistence & Data Flow

### 6.1 Co ukládá `SqliteStore` (whatsapp-rust managed)

- Session keys (Noise, Signal)
- Device identity (PN, LID, push_name)
- App State sync patches (archived, pinned, muted chats)
- Message secrets (pro decryption history)
- Pre-key bundles

### 6.2 Co ukládá naše aplikace (AppState → disk)

```
warust.db (naše SQLite, oddělené od whatsapp-rust)
├── chats (jid, name, last_msg_id, unread, archived, pinned, mute_until)
├── messages (id, chat_jid, sender_jid, text, ts, from_me, status, reply_to)
├── contacts (jid, push_name, verified_name, pic_url, is_business)
└── settings (key, value)  // theme, notifications, auto_download_media
```

> **Důvod oddělení**: `whatsapp-rust` store je opaque, nechceme měnit jeho schéma. Naše DB je pro UI rychlé dotazy (seřazení chatů, fulltext search, unread counts).

### 6.3 Sync strategie

| Směr | Mechanizmus |
|------|-------------|
| Server → App | HistorySync event (bootstrap) + live Messages events |
| App → Server | `send_text`, `mark_as_read`, `archive_chat`, `pin_chat` (IQ) |
| UI ↔ Disk | Periodický flush (5s) + okamžitý flush u klíčových změn (send, read) |

## 7. Modulární struktura projektu

```
warust/
├── Cargo.toml
├── assets/
│   └── NotoSans-Regular.ttf
├── src/
│   ├── main.rs                 # entry, eframe::run_native, logging init
│   ├── app.rs                  # struct WarustApp { state, tx_cmd, rx_status, handle }
│   ├── state.rs                # AppState, Chat, Message, Contact, ConnectionStatus
│   ├── theme.rs                # setup_theme, colors, fonts
│   ├── commands.rs             # enum Command { SendText, MarkRead, Archive, ... }
│   ├── backend.rs              # BackendTask: spawn_bot, handle_commands, event_loop
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── chat_list.rs        # ChatList, ChatItem
│   │   ├── conversation.rs     # ConversationView, MessageBubble, Composer
│   │   ├── qr_pairing.rs       # QrPairingScreen
│   │   ├── settings.rs         # SettingsDialog
│   │   └── components.rs       # shared: Avatar, StatusIcon, Timestamp
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── schema.sql          # CREATE TABLE statements
│   │   └── dao.rs              # ChatDao, MessageDao, ContactDao, SettingsDao
│   └── util/
│       ├── jid.rs              # Jid formatting, display names
│       └── time.rs             # human-readable timestamps
└── tests/
    └── integration_test.rs     # smoke test: build + run headless
```

## 8. Spuštění a build

### 8.1 Development

```bash
cd warust
cargo run                    # debug build
cargo run --release          # release build (fast UI)
RUST_LOG=warust=debug,whatsapp_rust=info cargo run  # verbose logs
```

### 8.2 Release (Windows)

```bash
cargo build --release
# výstup: target/release/warust.exe (~15-25 MB)
# Zahrne: egui/wgpu shader cache, fonty, qrcode generátor
```

### 8.3 CI (GitHub Actions)

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --release
      - run: cargo test
```

## 9. Rizika a mitigace

| Riziko | Dopad | Mitigace |
|--------|-------|----------|
| **WhatsApp ban** (neoficiální klient) | Účet zablokován | Disclaimer v README; uživatel používá na vlastní riziko; nedávat do store |
| **Protokol změna** (server-side) | Klient přestane fungovat | Sledovat `whatsapp-rust` releases; pinned revision v Cargo.lock |
| **History sync velikost** (MB-GB) | OOM / pomalý start | `LazyHistorySync::stream()` + `spawn_blocking` pro decode |
| **egui/wgpu na starém HW** | Černé okno / crash | Fallback na `glow` feature; test na CI |
| **SQLite locking** (dva procesy) | `database is locked` | Jeden proces na DB; `SqliteStoreConfig::pool_size = 1` |

## 10. Další kroky (po MVP)

1. **Média**: `client.upload_media` + `client.download_media` + thumbnail generator
2. **Volání**: `voip` feature + Opus + WebRTC (webrtc-dtls, webrtc-sctp)
3. **Reakce/odpovědi/editace**: `send_reaction`, `reply_quoting`, `edit_message`
4. **Hledání**: FTS5 v naší SQLite + `client.search_messages` (USync)
5. **Export/Backup**: SQL dump + media archive
6. **System Tray**: `tray-icon` crate, notifikace přes `notify-rust`
7. **Auto-update**: `tauri-updater` nebo custom GitHub releases check

## 11. Otevřené otázky pro implementaci

> Tyto body vyřešíme při psaní kódu, ne v návrhu.

- [ ] **JID display**: Jak formátovat `15551234567@s.whatsapp.net` → `+1 555 123 4567`? (`libphonenumber` crate?)
- [ ] **Avatar cache**: Stahovat profile pictures do `egui::Texture` cache s TTL?
- [ ] **Notifikace**: Native OS notifikace (`notify-rust`) vs egui toast?
- [ ] **Keyboard shortcuts**: `Ctrl+N` nový chat, `Ctrl+F` hledání, `Esc` zavřít dialog?
- [ ] **Migrace DB**: Verze schématu + migrace při upgrade aplikace?