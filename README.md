# NotchCat for Windows

NotchCat ported from macOS to Windows in Rust, Tauri 2, and React + TypeScript. It places a subtle dark top-center floating island at the top of the primary monitor with pastel paper workspace cards, a 6-day planner, activity insights, Windows media controls, ICS calendar agenda, and optional multi-device cloud sync.

## Features

- **Top-center floating island**: Default `116x22` pill (`Island`), `Notch`, or `Off` mode with hover-to-open, click, `Alt+N` global shortcut, system tray menu, and auto-close on focus loss.
- **Collapsed live status**: Left animated Windows SMTC Now Playing chip with artwork tint and right 5x7 dot-matrix Focus countdown pill, plus compact `IslandView` banners for `Time's up` and due reminders.
- **Workspace cards**: `Tasks` (priorities 1-3, inline editing, drag-and-drop reorder, reminder presets + custom time), `Focus Timer` (7-segment SVG display, work/break presets + custom duration, optional linked task auto-completion, WinMM synthesized chime, `WM_POWERBROADCAST` sleep/wake pause/resume), `Notepad` (debounced persistence, `/task` line conversion), `Events` (local & ICS calendar feed + Windows battery status), and `Music` (Windows Global System Media Transport Controls play/pause/prev/next + dominant pastel album tint).
- **Week & Insights**: 6-day drag-and-drop task scheduler, 7-day SVG activity bar chart, completed/archived history with debounced search, pagination, and one-click restore to today.
- **Account & Cloud Sync**: Two-step sign-up (access key verification + account creation), sign-in, Windows Credential Manager token storage, and offline-first dirty-field sync compatible with `notchcat-server`.
- **Settings & Platform**: Card visibility and order customization, hover/shortcut toggles, Launch at Windows login (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`), SQLite database backup, and GitHub Releases update checker.

## Prerequisites

- Windows 10 or Windows 11 with Microsoft Edge WebView2 Runtime (built into Windows 11)
- Rust stable toolchain (`rustup`, MSVC build tools)
- Node.js 20+ and npm

## Development

```powershell
cd notchcat-windows
npm install
npm run tauri dev
```

Pass CLI arguments through Cargo after `--`:

```powershell
cargo run --manifest-path src-tauri\Cargo.toml -- --open --fake-music
```

Supported CLI flags: `--open`, `--open-hover`, `--tab <workspace|week|insights|settings>`, `--tab-after <secs>`, `--demo`, `--demo-secs <secs>`, `--close-after <secs>`, `--remind-in <secs>`, `--fake-notch <island|notch|off>`, `--fake-music`, `--seed-demo`.

## Testing

```powershell
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Run standalone self-tests against the built binary:

```powershell
.\src-tauri\target\debug\notchcat-windows.exe --selftest-note
.\src-tauri\target\debug\notchcat-windows.exe --selftest-settings
.\src-tauri\target\debug\notchcat-windows.exe --selftest-focus
.\src-tauri\target\debug\notchcat-windows.exe --selftest-update
.\src-tauri\target\debug\notchcat-windows.exe --selftest-music
```

Run the 2-device sync self-test against a running `notchcat-server`:

```powershell
$env:NOTCHCAT_SERVER = "http://127.0.0.1:8000"
$env:NOTCHCAT_TEST_KEY = "<single-use-access-key>"
.\src-tauri\target\debug\notchcat-windows.exe --selftest-sync
```

## Release Build

```powershell
npm run build
npm run tauri build
```
