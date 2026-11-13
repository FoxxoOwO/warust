# Warust

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![egui](https://img.shields.io/badge/egui-0.37-blue.svg?style=flat-square)](https://github.com/emilk/egui)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green.svg?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg?style=flat-square)]()

**Warust** is an ultra-fast, lightweight desktop client for WhatsApp written in pure Rust using **egui / eframe** and **whatsapp-rust**.

Designed as a high-performance alternative to resource-heavy Electron applications, Warust features instant startup, minimal RAM consumption, native hardware acceleration, and WhatsApp Web multi-device protocol support.

> [!WARNING]
> **Experimental Status:** This project is in an early experimental stage and active development. Some features may be incomplete or subject to change. Use at your own discretion. This project is independent and not affiliated with, authorized, maintained, or endorsed by WhatsApp or Meta.

---

## Features

### Messaging & Chats
* **WhatsApp Desktop Look & Feel**: Dark mode theme designed to match the official desktop aesthetic.
* **Instant Responsive UI**: Built with immediate-mode GUI (`egui 0.37`), low latency, and 60+ FPS scrolling.
* **Chat Management**: Pin/unpin chats, mute notifications, archive chats, mark as read/unread, and clear conversations.
* **Contact Resolution**: Merges Phone Number (PN) and LID (Linked Identity) conversations into unified chats with contact names.
* **QR & Pair Code Login**: Log in by scanning the QR code or pair using an 8-character verification code sent to your phone number.

### Voice & Video Calls (VoIP)
* **Native VoIP Support**: Integrates WhatsApp call signaling with MLOW audio codec and WebRTC DataChannels.
* **Incoming Call Overlay**: Ringing popup with caller avatar, name, and Accept / Decline controls.
* **Active Call Window**: Floating window with call duration timer, microphone mute toggle, camera toggle, and hangup button.
* **Video Calling**: Encrypted H.264 video stream negotiation and camera controls.
* **Call Logging**: Automatically records incoming, outgoing, and missed calls in the conversation history.

### Message Reactions
* **Quick Reaction Bar**: Hover over message bubbles to pick from common reaction emojis.
* **Context Menu Support**: Right-click messages to add or remove reactions, reply, or delete.
* **Reaction Summary Pill**: Displays emoji counts and participant breakdown in a detail dialog.

### Rich Media & Attachments
* **Images & Photos**: Embedded previews with asynchronous thumbnail caching and full-resolution view.
* **Video Playback**: In-app player in the chat bubble - the message is decoded by ffmpeg and streamed as frames, with its audio track played alongside, a pause/resume toggle and a scrubbable timeline showing position and total length.
* **Voice Notes & Audio**: In-app playback with waveform progress and seek support. WhatsApp's Opus voice notes are transcoded through ffmpeg (no native Opus build step needed).
* **Videos, Documents & Voice Notes**: File transfer, inline indicators, and caption rendering.
* **Profile Avatars**: Background fetching and caching of user and group profile pictures.

### System Integration & Tray (Windows)
* **System Tray**: Minimize or close to tray icon with restore and exit menu.
* **Windows Autostart**: Optional registry integration to start minimized on login.
* **Native Application Icon**: Embedded Windows executable and taskbar icon.

### Scheduled Messages
* **Send Later**: Schedule messages to be sent automatically at a future date and time.
* **Scheduled Messages Manager**: Review, edit, cancel, or trigger queued messages.

### Search & Data Export
* **Message Search**: Filter by keywords, sender, current chat, or date ranges.
* **Conversation Export**: Export chat history to TXT, styled HTML, or structured JSON.

### Preferences & Settings
* **Taller Tabbed Settings Window**:
  * **Account & Profile**: View JID, push name, connection status, and session management.
  * **Chats & Media**: Enter-to-send settings, auto-download toggles, and formatting options.
  * **Notifications**: Desktop notification and sound toggles.
  * **Privacy**: Read receipts and typing indicator preferences.
  * **Appearance**: Font scaling and color theme customization.
  * **System & Startup**: Tray behavior and Windows autostart configuration.
  * **About & Storage**: Version information, media folder shortcut, and cache management.

---

## Architecture & Tech Stack

Warust utilizes a decoupled client-server architecture inside a single native binary:

```
+--------------------------------------------------------+
|                   eframe / egui GUI                    |
|  (Chat list, Conversation view, Modals, Call overlays) |
+---------------------------+----------------------------+
                            |
               Command / State Channels (mpsc)
                            |
+---------------------------v----------------------------+
|                    Async Backend                       |
|  (Tokio Runtime, SQLite Session Store, VoIP Engine)    |
+---------------------------+----------------------------+
                            |
               WhatsApp Protocol (Noise / Signal)
                            |
+---------------------------v----------------------------+
|                  WhatsApp Web Servers                  |
+--------------------------------------------------------+
```

* **GUI Framework**: [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) & [`egui`](https://github.com/emilk/egui) 0.37 (wgpu backend)
* **WhatsApp Protocol**: [`whatsapp-rust`](https://github.com/jlucaso1/whatsapp-rust) with `signal`, `sqlite-storage`, `tokio-transport`, and `voip-mlow`
* **Concurrency**: [`tokio`](https://tokio.rs/) multi-threaded async runtime
* **State Management**: Arc RwLock SharedState with atomic thread-safe updates

---

## Getting Started

### Prerequisites

* [Rust 1.80+](https://rustup.rs/) (stable toolchain)
* Operating System: Windows 10/11, Linux, or macOS
* [ffmpeg](https://ffmpeg.org/) for audio/video playback (WhatsApp voice notes are Opus, and video needs a decoder; there is no pure-Rust fallback).

  **Release bundles already include `ffmpeg` and `ffprobe` next to the executable** - nothing to install. The app looks for them in this order:

  1. `WARUST_FFMPEG` / `WARUST_FFPROBE` environment variables
  2. a sibling binary in the same directory as `warust` (how the releases ship)
  3. `ffmpeg` / `ffprobe` on `PATH`

  Without ffmpeg the app still runs, but voice notes and videos fall back to the system player.

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

## Usage

1. Launch Warust.
2. If not previously authenticated, scan the **QR Code** using your WhatsApp mobile app (*Linked devices -> Link a device*), or switch to **Pairing Code** to connect using your phone number.
3. Warust will perform an initial background history synchronization (displayed in a floating indicator).
4. Messaging, calling, and scheduling will be active directly from your desktop.

---

## Contributing

Contributions, issues, and feature requests are welcome! Feel free to check the [issues page](https://github.com/FoxxoOwO/warust/issues).

---

## License

This project is licensed under either the [MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE) at your option.
