# Changelog

All notable changes to Warust are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/). Release notes for every version are
also published on the [Releases page](https://github.com/FoxxoOwO/warust/releases).

## [Unreleased]

## [0.3.4] - 2026-11-15

Scrolls smoothly in long chats: the message panel no longer draws, nor copies, the
whole history every frame.

### Changed
- The message list draws only the messages around the viewport. Heights measured
  while drawing are cached per message, so the rows above and below are replaced by
  blank space of the right height — the scrollbar and "stick to bottom" stay honest.
- The panel takes the history out of the app state for the duration of the frame
  and puts it back, instead of cloning it (a clone of 2000 messages with thumbnails
  cost 1.9 ms per frame).
- Jumping to a message from search or the info panel now computes the scroll offset
  from the row model, because the target may not be drawn yet.

### Fixed
- A 2000-message chat spent 4.67 ms per frame just laying out rows nobody could
  see; it now spends 0.08 ms and the cost no longer grows with history length
  (measured by `bench_conversation_frames`).

## [0.3.3] - 2026-11-14

Saves work on this machine, lighter idle rendering, and a log file to attach to a
bug report.

### Fixed
- Long links and document names in the chat info panel pushed the whole row past
  the panel width and squeezed the conversation next to it. Rows now fit the panel
  and the name is truncated instead of stretched.
- The window asked the compositor for a new frame ten times a second even when
  nothing was happening (typing indicator, jump flash, queue drain). Repaints are
  now driven by actual animation state with a one second fallback, so an idle
  window costs almost nothing.
- Revoking an admin message could panic on `client.lid().unwrap()` when the client
  had no linked-device identity; it now tries the original sender, PN, then LID and
  returns a clear error instead of panicking.
- A failed state save was silently dropped at 26 call sites (`let _ =`), which on a
  full disk means a sent message disappears after a restart with no trace. The
  error is logged now, and so is a failed `current_session.txt` write.

### Added
- `%APPDATA%\warust\warust.log`: the app logs to a file next to the session
  database and rotates at 2 MB to `warust.1.log`, so a window that was never
  attached to a console still leaves evidence behind.
- `warust.exe --test-update`: downloads the latest package and unpacks it with the
  system tar into a temporary folder without installing anything, checks that a
  runnable `warust.exe` came out of it, then cleans up. This tests the installer
  path on a machine that has nothing newer to install.

### Changed
- Message list work per frame is smaller: one arrival-window snapshot per frame
  instead of three map lookups per visible bubble, and only the newest 30 messages
  are tracked for fade-in instead of the whole history.
- `docs/QA-PLAN.md` (verification steps, smoke matrix, bug capture protocol) and
  `docs/BUG-PLAN.md` (findings log, fix and verification protocol) are now part of
  the repository.

## [0.3.2] - 2026-11-14

The built-in updater actually installs the update now.

### Fixed
- "Download and install" did nothing: the update script called `tar` from `PATH`,
  which on Windows resolves to the GNU tar shipped with Git. Tar archives only, no
  zip, so extraction failed silently, the app exited and came back as the old
  version. The script now uses `%SystemRoot%\System32\tar.exe` and falls back to
  PowerShell `Expand-Archive`.
- The package is unpacked into a temporary folder first, so a failed extraction
  leaves the previous version in place instead of a half-written install, and why
  it failed is recorded in `%TEMP%\warust-update.log`.
- The update dialog stays open while downloading, shows progress and reports a
  failed download instead of closing as if nothing happened.

## [0.3.1] - 2026-11-14

Updates arrive on their own from this version on, and the interface finally
moves instead of snapping.

### Added — updates
- Automatic update checks against the GitHub releases API: a background check
  every six hours, a manual "Check for updates" button and an update dialog with
  the release notes.
- Self-update: the app downloads the platform package, replaces its own
  executable through a detached script, and restarts.
- Update preferences: background checking and silent auto-install.

### Added — interface
- Animation pass across the main surfaces. Chat list rows cross-fade on hover and
  selection and grow an accent bar; switching chats fades the whole conversation
  in; messages that arrive fade in with a short stagger; the history scrolls
  smoothly; and the jump-to-message highlight now fades in and back out instead of
  blinking off after two seconds.

### Fixed
- Faded accent colours no longer darken towards black while they lose opacity.
- The message history no longer paints over the composer.

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
