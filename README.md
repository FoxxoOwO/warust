# ⚡ Warust

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![egui](https://img.shields.io/badge/egui-0.37-blue.svg?style=flat-square)](https://github.com/emilk/egui)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green.svg?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg?style=flat-square)]()

**Warust** is an ultra-fast, lightweight, and modern **WhatsApp Desktop** client built in pure **Rust** using **egui / eframe** and **whatsapp-rust**.

Designed as a high-performance alternative to resource-heavy Electron applications, Warust features instant startup, minimal RAM consumption, native hardware acceleration, and full WhatsApp Web multi-device protocol support.

---

## ✨ Features

### 💬 Messaging & Chats
* **WhatsApp Desktop Look & Feel**: Beautiful dark mode theme matching the official WhatsApp Desktop aesthetic.
* **Instant Responsive UI**: Built with immediate-mode GUI (`egui 0.37`), zero UI lag, buttery smooth 60+ FPS scrolling.
* **Chat Management**: Pin/unpin chats, mute notifications, archive chats, mark as read/unread, and clear conversations.
* **Smart Contact Resolution**: Merges Phone Number (PN) and LID (Linked Identity) conversations seamlessly into canonical chats with real contact names.
* **QR & Pair Code Login**: Log in by scanning the QR code or enter your phone number to pair via an 8-character verification code.

### 📞 Voice & Video Calls (VoIP)
* **Native VoIP Support**: Integrates WhatsApp call signaling with pure-Rust MLOW audio codec and WebRTC DataChannels.
* **Incoming Call Overlay**: Displays animated ringing popup with caller avatar, name, and one-click *Accept* (`📞`) or *Decline* (`❌`) buttons.
* **Active Call Floating Window**: Real-time duration timer (`01:23`), microphone mute toggle (`🎙️` / `🔇`), video camera switch (`📹` / `🚫`), and hangup button (`🔴 Zavěsit`).
* **Chat History Call Logs**: Automatically logs calls and missed calls (`📞 Zmeškaný hovor`) directly into the conversation stream.

### 😊 Complete Emoji Reactions
* **Hover Emoji Quick Bar**: Hover over any message bubble and click `😊` to pick from popular reactions (`👍`, `❤️`, `😂`, `😮`, `😢`, `🙏`, `🔥`, `🎉`).
* **Context Menu Support**: Right-click any message to react, un-react, reply, star, or delete.
* **Interactive Reaction Pill**: Shows emoji summaries and counts under bubbles.
* **Reaction Details Modal**: Click the reaction badge to view who reacted with what emoji, with an option to remove your own reaction.

### 🖼️ Rich Media & Attachments
* **Images & Photos**: Embedded previews with asynchronous thumbnail and full-resolution image caching.
* **Videos, Documents & Voice Notes**: File transfer, inline indicators, and caption rendering.
* **Avatar Synchronization**: Automatic background fetching and caching of user and group profile pictures.

### 🕒 Scheduled Messages
* **Send Later**: Schedule any message to be automatically sent at a future date and time.
* **Scheduled Messages Manager**: Review, edit, cancel, or immediately send queued messages.

### 🔍 Search & Data Export
* **Advanced Full-Text Search**: Filter by keywords, sender, current chat, or date ranges.
* **Conversation Export**: Export chat history to **TXT**, standalone styled **HTML**, or structured **JSON**.

### ⚙️ WhatsApp Settings & Preferences
* **Modern Tabbed Settings**:
  * 👤 **Account & Profile**: View self JID, push name, connection status, and log out.
  * 💬 **Chats & Media**: Enter-to-send settings, auto-download preferences, 24h/12h timestamp toggle.
  * 🔔 **Notifications**: Desktop notification and sound toggles.
  * 🔒 **Privacy**: Read receipts and typing indicator settings.
  * 🎨 **Appearance**: Font scaling and color theme customization.
  * ℹ️ **About & Data**: App statistics and cache cleanup tools.

---

## 🛠️ Architecture & Tech Stack

Warust utilizes a clean decoupled client-server architecture inside a single native binary:

```
┌────────────────────────────────────────────────────────┐
│                   eframe / egui GUI                    │
│  (Chat list, Conversation view, Modals, Call overlays) │
└───────────────────────────┬────────────────────────────┘
                            │
               Command / State Channels (mpsc)
                            │
┌───────────────────────────▼────────────────────────────┐
│                    Async Backend                       │
│  (Tokio Runtime, SQLite Session Store, VoIP Engine)    │
└───────────────────────────┬────────────────────────────┘
                            │
              WhatsApp Protocol (Noise / Signal)
                            │
┌───────────────────────────▼────────────────────────────┐
│                  WhatsApp Web Servers                  │
└────────────────────────────────────────────────────────┘
```

* **GUI Framework**: [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) & [`egui`](https://github.com/emilk/egui) 0.37 (wgpu / glow backend)
* **WhatsApp Protocol**: [`whatsapp-rust`](https://github.com/jlucaso1/whatsapp-rust) with `signal`, `sqlite-storage`, `tokio-transport`, and `voip-mlow`
* **Concurrency**: [`tokio`](https://tokio.rs/) multi-threaded async runtime
* **State Management**: Arc RwLock SharedState with atomic thread-safe updates

---

## 🚀 Getting Started

### Prerequisites

* [Rust 1.80+](https://rustup.rs/) (stable toolchain)
* Operating System: Windows 10/11, Linux, or macOS

### Installation & Build

1. **Clone the repository**:
   ```bash
   git clone https://github.com/FoxxoOwO/warust.git
   cd warust
   ```

2. **Run tests**:
   ```bash
   cargo test
   ```

3. **Build and Run**:
   ```bash
   # Development run
   cargo run

   # Optimized release build
   cargo build --release
   ```
   The compiled executable will be located in `target/release/warust` (or `warust.exe` on Windows).

---

## 📖 Usage

1. Launch Warust.
2. If not previously authenticated, scan the **QR Code** using your WhatsApp mobile app (*Linked devices -> Link a device*), or switch to **Pairing Code** to connect using your phone number.
3. Warust will perform an initial background history synchronization (displayed in a non-intrusive floating toast).
4. Enjoy messaging, calling, and scheduling directly from your desktop!

---

## 🤝 Contributing

Contributions, issues, and feature requests are welcome! Feel free to check the [issues page](https://github.com/FoxxoOwO/warust/issues).

---

## 📜 License

This project is licensed under either the [MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE) at your option.
