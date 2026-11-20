# NotchCat

NotchCat is a small Windows utility that turns the top center of your screen into a drop-down workspace: a task list, a focus timer, a notepad, a reminder scheduler and a calendar strip that appear when you hover the notch or press **Ctrl+Space** (**Alt+N** also works).

It is written in Rust, Tauri 2, React and TypeScript. Everything is stored locally in a SQLite file; an optional account syncs tasks, notes, sessions and reminders across devices.

## Features

- **Notch tray.** Hover the top-center notch pill — or press **Ctrl+Space** / **Alt+N** — to slide open a dark glass workspace; move the cursor away and it tucks back into the top edge with a morphing slide animation.
- **Cards.** Turn individual cards on or off in Settings and drag them into the order you want. Pick any two to sit side by side inside the Tasks card.
- **Tasks.** Today's checklist with a progress pill, an estimated finish time when your list has estimates, inline duration chips (`15m`, `30m`, `45m`, `1h`, `2h`), drag reordering, a Play button on each row that starts a focus timer for that task, and a finished-tasks history view with search and a one-click restore to Today.
- **Remind Me.** Schedule one-off or repeating reminders (daily, weekdays, weekly, monthly) with a date, time and optional note. Due reminders fire a native Windows toast notification plus a chime and show an in-tray banner with Snooze and Done.
- **Focus timer.** Work / short-break / long-break presets with custom minute lengths, auto-advance across rounds, an end-of-round sound and an active-task banner. Completed work rounds are logged as focus sessions.
- **Top-bar pill.** While a timer is running and the tray is closed, a compact countdown pill sits at the top of the screen with pause and stop controls.
- **Notepad.** Plain-text scratchpad that saves as you type (`300ms` debounce).
- **Events.** Shows today's reminders alongside any read-only `.ics` calendar feeds you subscribe to in Settings (Google Calendar, Outlook, iCloud, Fastmail, etc., refreshed every 15 minutes).
- **Week.** Seven-day strip showing scheduled reminders, calendar events, completed tasks and focus minutes per day, with a day detail sheet and quick scheduling for that day.
- **Insights.** Daily and weekly totals for focus time and completed tasks, streak tracking, a configurable daily focus goal, per-task time breakdown and manual focus-session logging.
- **Now Playing** (off by default). Optional card and collapsed album-art chip for Spotify, Apple Music and any Windows System Media Transport Controls (SMTC) player, with play/pause/skip, seekable progress bar, dominant-color tinting and iTunes Search artwork fallback.
- **Cloud sync** (optional, invite only). Sign in with an account on a [notchcat-server](https://github.com/shimijallores/notchcat-server) instance to sync tasks, focus sessions, notes and reminders across devices. Offline edits queue locally and push when you're back online.
- **Fake notch** for Windows displays, with two collapsed styles: a sleek top-edge **Notch** cutout or a floating **Island** pill, plus a preview toggle in Settings.
- **Battery** indicator in the tray header using the Win32 `GetSystemPowerStatus` API.

## Install

1. Download `NotchCat_1.0.7_x64-setup.exe` from the [latest release](https://github.com/shimijallores/notchcat-windows/releases/latest) (or [build it yourself](#build)).
2. Run the setup installer.
3. Because the build isn't code-signed, Windows SmartScreen may show a warning on first launch. Click **More info** → **Run anyway**.

Once running, a small cat icon appears in the Windows system tray and the notch pill sits at the top center of your primary display.

## Requirements

- Windows 10 (1809+) or Windows 11 (64-bit) with the Microsoft Edge WebView2 Runtime
- Rust 1.77+ (`stable-x86_64-pc-windows-msvc`) and Node.js 20+ (only required to build from source)

## Build

```powershell
npm install
npm run tauri dev     # run in development mode
npm run tauri build   # build release binary & NSIS setup installer
```

`npm run tauri build` compiles the Rust backend and React frontend and produces the standalone executable at `src-tauri/target/release/notchcat-windows.exe` along with the NSIS installer at `src-tauri/target/release/bundle/nsis/NotchCat_1.0.7_x64-setup.exe`.

## Using it

| Action | How |
| --- | --- |
| Open the tray | Move the pointer into the notch at the top center of the screen, click the notch (when enabled in Settings), or press **Ctrl+Space** / **Alt+N**. |
| Keep it open | While the pointer is inside the tray or a text field is focused, it stays open. |
| Close it | Move the pointer outside the tray (`180ms` grace period), press **Ctrl+Space** / **Alt+N** again, or click outside. |
| Quit | Right-click the cat icon in the Windows system tray → **Quit NotchCat**, or use the footer button in Settings. |

## Cloud sync

Cloud sync is off until you sign in under **Settings → Account**. Accounts are currently invite-only; the companion backend lives at [notchcat-server](https://github.com/shimijallores/notchcat-server).

To point the app at your own server, set `NOTCHCAT_SYNC_URL` in the environment before launching:

```powershell
$env:NOTCHCAT_SYNC_URL = "https://your-server.example"
.\src-tauri\target\release\notchcat-windows.exe
```

If you don't sign in, NotchCat makes no sync requests and everything stays in the local database.

## Now Playing and privacy

The Now Playing card and the album-art chip beside the notch are **off by default**. When you turn either one on in Settings, NotchCat reads the currently playing track from the Windows `GlobalSystemMediaTransportControlsSessionManager` (SMTC) API so it can show the track title, artist, playback position and album cover and let you play, pause or skip.

When a media player doesn't provide an embedded thumbnail stream over SMTC, NotchCat queries the public Apple iTunes Search API (`https://itunes.apple.com/search`) with the track title and artist to look up album artwork. Leaving both toggles off disables all SMTC polling and artwork lookups.

## Data

All local state lives in a single SQLite database in WAL mode at:

```text
%APPDATA%\NotchCat\notchcat.sqlite
```

| Table | Holds |
| --- | --- |
| `tasks` | Title, completion state, estimate, sort order, timestamps, soft-delete flag |
| `focus_sessions` | Completed focus rounds with duration, linked task and timestamp |
| `notes` | Notepad body (`id = 'main'`) |
| `reminders` | Scheduled reminders, repeat rule, completion state |
| `settings` | Key/value preferences (timers, layout, toggles, `.ics` URLs, sync cursors) |
| `sync_outbox` | Pending mutations queued for cloud sync |

Cloud sync tokens are stored in Windows Credential Manager (`NotchCat` service, with an automatic `.tok` fallback next to the database when unavailable). You can export a timestamped copy of the database at any time from **Settings → Back up database**.

## Project layout

```text
src-tauri/src/
  lib.rs                   Tauri plugin setup, tray menu, command registration, CLI dispatch
  state.rs                 AppState, settings keys, workspace layout, data models
  db.rs                    SQLite wrapper, schema v1–v5 migrations, outbox queue, backup
  platform.rs              WinRT SMTC media bridge, iTunes artwork fallback, ICS parser, Win32 battery, updater
  sync.rs                  SyncEngine: register/login, token refresh, pull/push, 30s loop
  window.rs                Borderless transparent top-center window positioning, cursor dwell polling, hotkeys
  selftest.rs              Self-test suites (--selftest, --selftest-sync, --selftest-ics, --selftest-update)
src/
  App.tsx                  Root notch container, morphing tray open/close transitions, tab & modal routing
  components/
    CollapsedIsland.tsx    Collapsed Notch / Island pill, music chip, focus timer pill
    WorkspaceView.tsx      Card layout, Tasks, Focus timer, Notepad, Events, Now Playing, finished history
    WeekView.tsx           7-day strip, day breakdown, quick reminder scheduling
    InsightsView.tsx       Focus/task totals, streaks, daily goal, manual session sheet
    RemindMeModal.tsx      Reminder scheduler modal and due-reminder banner
    SettingsView.tsx       Account, cards, appearance, timers, calendars, updates, backup
```

## Developer flags

All flags are optional and only used for testing:

| Flag | Effect |
| --- | --- |
| `NOTCHCAT_DB=C:\path\to\db.sqlite` | Use a custom SQLite path instead of `%APPDATA%\NotchCat\notchcat.sqlite`. |
| `--open-on-launch` | Open the tray immediately on launch. |
| `--open-settings` | Open straight into the Settings tab. |
| `--open-remind-me` | Open the Remind Me modal on launch. |
| `--open-history` | Open the finished-tasks sheet on launch. |
| `--open-log-session` | Open the manual focus-session sheet on launch. |
| `--tab workspace\|week\|insights\|settings` | Select the initial tab. |
| `--preview-island` | Force the floating Island preview on launch. |
| `--test-due-banner` | Trigger a sample due-reminder banner for UI testing. |
| `--selftest` | Run the database, state and migration self-test suite and exit. |
| `--selftest-sync` | Run the cloud-sync self-test suite against a local mock server and exit. |
| `--selftest-ics` | Run the `.ics` parser self-test suite and exit. |
| `--selftest-update` | Run the release update-checker self-test suite and exit. |

## Not built yet

- Editing events in external calendars — `.ics` feeds are read-only; only reminders created inside NotchCat can be added or edited.

## License

[MIT](LICENSE). The bundled mascot artwork is a project trademark and isn't covered by the MIT license.
