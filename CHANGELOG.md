# Changelog

All notable changes to Warust are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/). Release notes for every version are
also published on the [Releases page](https://github.com/FoxxoOwO/warust/releases).

## [Unreleased]

## [0.2.0] - 2026-11-14

A large feature jump since 0.1.2: the whole November 2026 development wave.

### Added — messaging
- Edit sent messages in place.
- Multi-select and forwarding, including forwarding to several chats at once.
- Pinned messages: a banner above the conversation, with 24 h, 7 day and 30 day
  durations, mirrored from server pin updates.
- View-once media: photos and videos open exactly once and are deleted locally
  afterwards; a "view once" toggle while sending.
- Per-chat drafts, preserved across chat switches and restarts.
- Media gallery in the chat info panel, split into photos and videos, documents
  and links.
- Jump to a message from the search results, with a short highlight.

### Added — groups
- Group creation, member management, admin promotion and removal.
- Joining groups by invite link, including groups that require approval.
- Admin-only toggles for posting and for editing group information.
- `@mentions` with member autocomplete, highlighted names in bubbles and a
  "Mention" prefix in desktop notifications.
- Contact sharing as vCard and sticker sending (WebP).

### Added — voice and media
- Voice notes with a waveform, recorded and converted to Opus with the bundled
  ffmpeg.
- Pasting images from the clipboard: Ctrl+V turns a screenshot into an
  attachment.
- Attachment previews, thumbnails and a local media cache.

### Added — UI and controls
- Polls: creation, voting, live tallies and quiz mode.
- Archived chats rendered as their own collapsible section.
- Jump to the next unread chat with Alt+Down.
- Keyboard shortcuts: Ctrl+K chat switcher, Ctrl+F message search, Esc closes the
  topmost window.
- Chat info panel, mute durations and disappearing messages.

### Added — maintenance
- Storage tab with media folder statistics and cleanup of files no message
  references any more.

### Changed
- Application state is split into a core file plus one message file per chat,
  which makes startup and writes considerably cheaper.

## [0.1.2] - 2026-11-13

- Desktop notifications, system tray behaviour and background startup.
- Message scheduling, chat export and global message search.

Earlier versions: see the [Releases page](https://github.com/FoxxoOwO/warust/releases).
