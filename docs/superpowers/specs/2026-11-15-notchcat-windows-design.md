# NotchCat for Windows: Design and Plan

Status: draft for review. Classification: architectural (new client).

## Goal

A faithful port of the macOS NotchCat app to Windows, rewritten in Rust and Tauri 2. Same cards, same look, same behavior, same sync account. The notch is replaced by a floating black island at the top center of the screen.

Parity means everything the Mac app does today. Voice input, MCP/AI-agent integration and Apple Reminders sync are not built on the Mac either, so they are out of scope here too.

## Decisions

| Area | Decision |
| --- | --- |
| Shell | Tauri 2, Rust backend, WebView2 front end |
| Front end | React + TypeScript + Vite, CSS animations. The SwiftUI views are re-created pixel for pixel |
| Anchor | Borderless, transparent, always-on-top, no taskbar entry window at top center. Collapsed it is click-through |
| Storage | `rusqlite`, same schema and migrations as the Mac app, at `%APPDATA%\NotchCat\notchcat.sqlite` |
| Sync | Same protocol as the Mac client against api.notchcat.cloud. `reqwest` + `keyring` (Windows Credential Manager). No server changes |
| Hotkey | `Alt+N` via the Tauri global-shortcut plugin, rebindable if taken |
| Now Playing | Windows System Media Transport Controls through the `windows` crate. No permissions needed |
| Events card | Reminders plus read-only calendar events from an ICS subscription URL (Google, Outlook, iCloud all offer one). No OAuth |
| Battery / sleep | `GetSystemPowerStatus` and power broadcast events (timer pauses on suspend) |
| Installer | Tauri NSIS bundle, built by GitHub Actions on `v*` tags, same flow as the Mac DMG |
| Updates | Same GitHub-releases check and "Update available" button |
| Repo | New project `notchcat-windows`, separate from `notchcat`, shares only the server API |

## Architecture

```
  React UI (cards, views, theme)      <-- invoke / events -->     Rust core
        |                                                          |-- data      (rusqlite, migrations, repositories)
   Island window                                                   |-- models    (tasks, timer, notes, reminders, settings)
   (transparent, topmost,                                          |-- sync      (engine, outbox, api, credentials)
    click-through when closed)                                     |-- platform  (hotkey, tray, media, power, battery, cursor)
                                                                   '-- updater   (GitHub releases)
```

The Rust core owns all state and logic. The UI is a thin view layer that calls commands and listens to events. This mirrors the Mac split between Models and Views, so each Swift file has a clear Rust or React counterpart.

### Island window

- One window sized to the largest tray, positioned at top center of the active monitor. The tray animates inside it with CSS (grow from the pill, retract back).
- Collapsed: `set_ignore_cursor_events(true)`. Rust polls the cursor position (about 60 Hz, only while collapsed) to detect hover over the pill area and to open the tray.
- Open: the window takes input. Click outside or Esc closes it.
- Handles multi-monitor, per-monitor DPI, taskbar position, and hides over exclusive fullscreen apps.
- The pill with the dot-matrix countdown is the collapsed state, matching the menu-bar pill on Mac.
- System tray cat icon replaces the menu-bar cat (open, settings, quit).

## Feature parity map

| Mac feature | Windows |
| --- | --- |
| Tray from notch, hover / click / ⌥N | Island, hover / click / Alt+N, tray icon |
| Fake Notch / Island styles | Island is the default. Notch-shaped variant kept as a style option |
| Tasks: quick add, drag reorder, Today/Tomorrow, rollover, time limits, footer finish time | Direct port |
| Remind Me, snooze, Done card | Rust scheduler, island card, optional Windows toast |
| Focus timer, task attach, auto-done, pause on sleep | Direct port, power events |
| Dot-matrix pill and seven-segment display | SVG/canvas components |
| Notepad, ⌘↵ line to task, undo | Ctrl+Enter, Ctrl+Z |
| Events and reminders | Reminders plus ICS calendar events |
| Week view with drag between days | Direct port |
| Insights, History, Restore to Today | Direct port |
| Now Playing card and album chip | SMTC (play, pause, next, previous, artwork) |
| Battery in header | Power status |
| Update available | Same |
| Settings (Appearance, General), card show/hide/reorder | Direct port, plus launch at startup |
| Cloud sync, sign in / sign up with access key, welcome animation | Direct port |
| Back up database to Downloads | Direct port |
| Developer flags (`--open`, `--tab`, `--snapshot`, `--selftest-*`, `NOTCHCAT_DB`) | CLI args and env vars, same names |

## Risks

- **Unsigned installer:** SmartScreen will warn, like Gatekeeper does on the Mac. Code signing costs money, so v1 ships unsigned with install instructions.
- **Hover detection while click-through:** needs the cursor poll. Must stay cheap (idle CPU near zero).
- **Transparent windows on WebView2:** known rough edges with shadows and DPI scaling. Spike this first.
- **Visual parity:** SwiftUI to CSS is manual. Use the Mac `--snapshot` PNGs as the reference for side-by-side checks.
- **Sync correctness:** the field-level LWW and note conflict logic must match the server exactly. Port the Mac tests and run an end-to-end two-device test against a local server.
- **Mascot artwork** is a project trademark. Reuse is fine for your own app.

## Plan (phases)

Each phase ends in something runnable and is committed on its own.

0. **Reference capture.** Generate Mac snapshots of every tab and state with `--snapshot`. Extract theme tokens (colors, fonts, sizes, animation timings) from `Theme.swift`, `NotchGeometry.swift` and `NotchController.swift` into a design-tokens file.
1. **Island spike.** Tauri scaffold. Transparent topmost window at top center, pill, hover open and close, Esc, Alt+N, tray icon, DPI and multi-monitor. Gate: it feels like the Mac tray open and close.
2. **Data layer.** Port `Database.swift` migrations and `Repositories.swift` to `rusqlite`, with unit tests. Confirm a Mac database backup opens cleanly.
3. **Core cards.** Tasks, Focus timer, Notepad, with theme, dot-matrix and seven-segment components, and the footer finish-time math.
4. **Reminders, Week, Insights, History.** Reminder scheduler and island card, Week drag between days, charts, History search and restore.
5. **Settings and shell polish.** Appearance, General, card reorder and visibility, fake-notch style, startup launch, DB backup, dev flags.
6. **Cloud sync.** API client, credentials, outbox, engine, tombstones, note conflict merge, sign in/up UI with access-key check and welcome animation. Port the Mac self-test as an integration test against a local `notchcat-server`.
7. **System integrations.** Now Playing (SMTC), battery, sleep pause, ICS calendar events, update checker.
8. **Packaging.** NSIS installer, GitHub Actions release on `v*` tags, README and install notes.
9. **Parity QA.** Side-by-side snapshot comparison against the Mac, on a few DPI and monitor setups, plus a manual checklist of every item in the parity map.

Phases 1 and 2 are independent and can run in parallel. Everything after depends on both.

## Testing

- Rust unit tests for repositories, timer math, rollover, finish-time, sync merge.
- Integration test for two-device sync against a local server.
- Snapshot command renders each tab to PNG for visual comparison with the Mac output.
- A manual parity checklist at the end of phase 9.

## Open questions

- Project name and bundle id (`NotchCat` and `cloud.notchcat.windows`?).
- GitHub repo name for the Windows project.
