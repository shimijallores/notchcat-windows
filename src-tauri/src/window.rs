use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos,
    GetMessageW, SetWindowLongPtrW, TranslateMessage, GWLP_WNDPROC, MSG, WM_POWERBROADCAST,
    WS_OVERLAPPED,
};

use crate::island::IslandGeometry;
use crate::platform::{
    fetch_ics_text, fetch_latest_release, parse_ics_events_for_range, play_timer_sound,
    poll_music_now, read_battery_status, CalendarAccess,
};
use crate::state::{AppRuntime, CompactBanner};
use crate::store::{add_local_days, now_ms, now_secs, today_start};
use crate::timer::poll_due_reminders;

pub const ISLAND_LABEL: &str = "island";
pub const HOTKEY_STR: &str = "Ctrl+Space";
pub const ALT_HOTKEY_STR: &str = "Alt+N";
pub const HOTKEYS: &[&str] = &[HOTKEY_STR, ALT_HOTKEY_STR];

pub fn emit_snapshot(app: &AppHandle, runtime: &AppRuntime) {
    let snap = runtime.build_snapshot();
    let _ = app.emit("app-state-changed", snap);
}

pub fn create_island_window(app: &AppHandle, runtime: &Arc<AppRuntime>) -> tauri::Result<()> {
    let (screen_x, screen_y, screen_w) = primary_monitor_logical_origin_and_width(app);
    let overlay_rect = IslandGeometry::overlay_window_rect(screen_w);

    {
        let mut ui = runtime.ui.lock().unwrap();
        ui.overlay_w = overlay_rect.w;
    }

    let win = WebviewWindowBuilder::new(app, ISLAND_LABEL, WebviewUrl::App("index.html".into()))
        .title("NotchCat")
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .visible(false)
        .position(screen_x + overlay_rect.x, screen_y)
        .inner_size(overlay_rect.w, overlay_rect.h)
        .build()?;

    let _ = win.set_position(LogicalPosition::new(screen_x + overlay_rect.x, screen_y));
    let _ = win.set_size(LogicalSize::new(overlay_rect.w, overlay_rect.h));
    let _ = win.set_ignore_cursor_events(true);

    let rt_focus = Arc::clone(runtime);
    let app_focus = app.clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            let should_close = {
                let mut ui = rt_focus.ui.lock().unwrap();
                let suppressed = ui
                    .suppress_auto_close_until
                    .map(|t| Instant::now() < t)
                    .unwrap_or(false);
                if ui.is_open && !suppressed && !ui.drag_active {
                    ui.is_open = false;
                    ui.pinned = false;
                    ui.modal_open = false;
                    ui.hover_armed = true;
                    ui.exit_ticks = 0;
                    true
                } else {
                    false
                }
            };
            if should_close {
                emit_snapshot(&app_focus, &rt_focus);
            }
        }
    });

    win.show()?;
    Ok(())
}

pub fn primary_monitor_logical_origin_and_width(app: &AppHandle) -> (f64, f64, f64) {
    if let Ok(Some(mon)) = app.primary_monitor() {
        let scale = mon.scale_factor();
        let pos = mon.position();
        let size = mon.size();
        return (
            pos.x as f64 / scale,
            pos.y as f64 / scale,
            size.width as f64 / scale,
        );
    }
    (0.0, 0.0, 1920.0)
}

pub fn open_panel(app: &AppHandle, runtime: &AppRuntime, pin: bool) {
    {
        let mut ui = runtime.ui.lock().unwrap();
        ui.compact = None;
        ui.is_open = true;
        ui.pinned = pin;
        ui.hover_armed = false;
        ui.exit_ticks = 0;
        ui.ignore_cursor_dirty = true;
        if pin {
            ui.suppress_auto_close_until = Some(Instant::now() + Duration::from_millis(350));
        }
    }
    if pin {
        if let Some(w) = app.get_webview_window(ISLAND_LABEL) {
            let _ = w.set_ignore_cursor_events(false);
            let _ = w.set_focus();
        }
    }
    emit_snapshot(app, runtime);
}

pub fn close_panel(app: &AppHandle, runtime: &AppRuntime) {
    {
        let mut ui = runtime.ui.lock().unwrap();
        ui.is_open = false;
        ui.pinned = false;
        ui.modal_open = false;
        ui.drag_active = false;
        ui.exit_ticks = 0;
        ui.hover_enter_at = None;
    }
    emit_snapshot(app, runtime);
}

pub fn toggle_panel(app: &AppHandle, runtime: &AppRuntime) {
    let currently_open = runtime.ui.lock().unwrap().is_open;
    if currently_open {
        close_panel(app, runtime);
    } else {
        open_panel(app, runtime, true);
    }
}

pub fn setup_tray(app: &AppHandle, runtime: Arc<AppRuntime>) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "Open NotchCat (Ctrl+Space)", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit NotchCat", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &quit_item])?;

    let rt_menu = Arc::clone(&runtime);
    let rt_click = Arc::clone(&runtime);

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .tooltip("NotchCat (Ctrl+Space)")
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => open_panel(app, &rt_menu, true),
            "quit" => {
                {
                    let mut guard = rt_menu.core.lock().unwrap();
                    let c = &mut *guard;
                    c.timer.flush_for_quit(&c.db);
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_panel(tray.app_handle(), &rt_click);
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

pub fn sync_hotkey_registration(app: &AppHandle, runtime: Arc<AppRuntime>, enabled: bool) {
    let gs = app.global_shortcut();
    for hk in HOTKEYS {
        let _ = gs.unregister(*hk);
    }
    if !enabled {
        return;
    }
    for hk in HOTKEYS {
        let rt = Arc::clone(&runtime);
        let _ = gs.on_shortcut(*hk, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_panel(app, &rt);
            }
        });
    }
}

/// 30Hz Win32 cursor tracker managing hover-to-open, 400ms hover-exit close, and dynamic click-through.
pub fn spawn_cursor_poller(app: AppHandle, runtime: Arc<AppRuntime>) {
    std::thread::spawn(move || {
        let mut last_ignore: Option<bool> = None;
        loop {
            std::thread::sleep(Duration::from_millis(33));

            let Some(win) = app.get_webview_window(ISLAND_LABEL) else {
                break;
            };
            let Ok(phys_pos) = win.outer_position() else {
                continue;
            };
            let Ok(scale) = win.scale_factor() else {
                continue;
            };
            let mut pt = POINT::default();
            if unsafe { GetCursorPos(&mut pt) }.is_err() {
                continue;
            }
            let lx = (pt.x - phys_pos.x) as f64 / scale;
            let ly = (pt.y - phys_pos.y) as f64 / scale;

            let (style, hover_to_open, visible_cards, show_pill, show_chip) = {
                let core = runtime.core.lock().unwrap();
                let m = runtime.platform.music.lock().unwrap();
                let snap = core.timer.snapshot();
                let show_pill = core.settings.data.show_pill
                    && (core.timer.is_active() || snap.state == crate::timer::TimerState::Finished);
                let show_chip = core.settings.data.show_music_chip
                    && m.is_active()
                    && m.track.is_some();
                (
                    core.settings.data.fake_notch,
                    core.settings.data.hover_to_open,
                    core.settings.visible_cards(),
                    show_pill,
                    show_chip,
                )
            };

            let mut need_emit = false;
            let desired_ignore: bool;
            let force_ignore_sync: bool;

            {
                let mut ui = runtime.ui.lock().unwrap();
                force_ignore_sync = std::mem::replace(&mut ui.ignore_cursor_dirty, false);
                let overlay_w = ui.overlay_w;
                let has_modal_or_banner = ui.modal_open || ui.pending_reminder.is_some();

                if ui.is_open {
                    let expanded = IslandGeometry::expanded_rect(
                        overlay_w,
                        style,
                        ui.tab,
                        &visible_cards,
                        has_modal_or_banner,
                    );
                    let interactive_rect = if ui.modal_open {
                        crate::island::Rect::new(expanded.x, expanded.y, expanded.w, expanded.h + 48.0)
                    } else {
                        expanded
                    };
                    let inside_exact = interactive_rect.contains(lx, ly);
                    desired_ignore = !(inside_exact || ui.drag_active);

                    if !ui.pinned {
                        let suppressed = ui
                            .suppress_auto_close_until
                            .map(|t| Instant::now() < t)
                            .unwrap_or(false);
                        if suppressed || ui.drag_active {
                            ui.exit_ticks = 0;
                        } else if expanded.inflated(14.0, 14.0).contains(lx, ly) {
                            ui.exit_ticks = 0;
                        } else {
                            ui.exit_ticks += 1;
                            if ui.exit_ticks >= 12 {
                                ui.is_open = false;
                                ui.pinned = false;
                                ui.modal_open = false;
                                ui.exit_ticks = 0;
                                ui.hover_armed = true;
                                ui.hover_enter_at = None;
                                need_emit = true;
                            }
                        }
                    }
                } else {
                    let hit = IslandGeometry::collapsed_hit_rect(
                        overlay_w,
                        style,
                        show_pill,
                        show_chip,
                        ui.compact.is_some(),
                    );
                    let in_collapsed = hit.map(|r| r.contains(lx, ly)).unwrap_or(false);
                    desired_ignore = !in_collapsed;

                    if ui.compact.is_none() && hover_to_open {
                        if !in_collapsed {
                            ui.hover_enter_at = None;
                            ui.hover_armed = true;
                        } else if ui.hover_armed {
                            let now = Instant::now();
                            match ui.hover_enter_at {
                                None => ui.hover_enter_at = Some(now),
                                Some(t0) if now.duration_since(t0) >= Duration::from_millis(120) => {
                                    ui.is_open = true;
                                    ui.pinned = false;
                                    ui.hover_armed = false;
                                    ui.hover_enter_at = None;
                                    ui.exit_ticks = 0;
                                    need_emit = true;
                                }
                                _ => {}
                            }
                        }
                    } else if !in_collapsed {
                        ui.hover_enter_at = None;
                        ui.hover_armed = true;
                    }
                }
            }

            if force_ignore_sync || last_ignore != Some(desired_ignore) {
                let _ = win.set_ignore_cursor_events(desired_ignore);
                last_ignore = Some(desired_ignore);
            }
            if need_emit {
                emit_snapshot(&app, &runtime);
            }
        }
    });
}

pub fn spawn_background_loops(app: AppHandle, runtime: Arc<AppRuntime>) {
    // 1. Timer tick (250ms), midnight rollover & compact/timer state broadcasts
    let app_timer = app.clone();
    let rt_timer = Arc::clone(&runtime);
    tauri::async_runtime::spawn(async move {
        let mut last_day = today_start();
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let (changed, finished_outcome, timer_sound, linked_title, rolled_over) = {
                let mut guard = rt_timer.core.lock().unwrap();
                let c = &mut *guard;
                let time_up_island = c.settings.data.time_up_island;
                let timer_sound = c.settings.data.timer_sound;
                let linked_title = c
                    .timer
                    .linked_task_id
                    .as_deref()
                    .and_then(|id| c.tasks.task(id).map(|t| t.title.clone()));
                let (changed, outcome) =
                    c.timer
                        .tick(&c.db, &mut c.tasks, time_up_island, timer_sound);
                let cur_day = today_start();
                let rolled_over = if (cur_day - last_day).abs() >= 1.0 {
                    last_day = cur_day;
                    c.tasks.roll_over_if_needed(&c.db)
                } else {
                    false
                };
                if let Some(ref out) = outcome {
                    if out.session_logged || out.completed_task_id.is_some() {
                        c.invalidate_insights();
                    }
                }
                (changed, outcome, timer_sound, linked_title, rolled_over)
            };

            if let Some(out) = finished_outcome {
                if out.play_sound {
                    play_timer_sound("Glass");
                }
                if out.show_time_up_island {
                    let mut ui = rt_timer.ui.lock().unwrap();
                    if !ui.is_open {
                        ui.compact = Some(CompactBanner::TimesUp {
                            sound: timer_sound,
                            task_title: linked_title,
                        });
                    }
                }
                emit_snapshot(&app_timer, &rt_timer);
                AppRuntime::trigger_debounced_sync(&rt_timer, app_timer.clone());
            } else if changed || rolled_over {
                emit_snapshot(&app_timer, &rt_timer);
            }
        }
    });

    // 2. Windows SMTC Now Playing poll (1s) unless disabled or in fake-music mode
    if !runtime.startup_flags.no_smtc && !runtime.startup_flags.fake_music {
        let app_music = app.clone();
        let rt_music = Arc::clone(&runtime);
        tauri::async_runtime::spawn(async move {
            let mut prev_key = String::new();
            loop {
                let wanted = {
                    let core = rt_music.core.lock().unwrap();
                    core.settings.music_wanted()
                };
                if wanted {
                    let pf = Arc::clone(&rt_music.platform);
                    let _ = tokio::task::spawn_blocking(move || {
                        poll_music_now(&pf);
                    })
                    .await;
                }

                let cur_key = {
                    let m = rt_music.platform.music.lock().unwrap();
                    match &m.track {
                        Some(t) => format!(
                            "{}:{}:{}:{}",
                            t.id,
                            t.playing,
                            (t.elapsed / 2.0) as i64,
                            t.artwork_data_url.is_some()
                        ),
                        None => String::new(),
                    }
                };
                if cur_key != prev_key {
                    prev_key = cur_key;
                    emit_snapshot(&app_music, &rt_music);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }

    // 3. Reminder scheduler poll (every 2s)
    let app_rem = app.clone();
    let rt_rem = Arc::clone(&runtime);
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let (due_list, play_sound) = {
                let mut guard = rt_rem.core.lock().unwrap();
                let c = &mut *guard;
                let enabled = c.settings.data.reminders_enabled;
                let sound = c.settings.data.timer_sound;
                let due = poll_due_reminders(&c.db, &mut c.tasks, enabled, now_secs());
                (due, sound)
            };
            if let Some(first) = due_list.into_iter().next() {
                if play_sound {
                    play_timer_sound("Ping");
                }
                {
                    let mut ui = rt_rem.ui.lock().unwrap();
                    ui.pending_reminder = Some(first.clone());
                    if !ui.is_open {
                        ui.compact = Some(CompactBanner::Reminder {
                            task_id: first.id,
                            title: first.title,
                        });
                    }
                }
                emit_snapshot(&app_rem, &rt_rem);
                AppRuntime::trigger_debounced_sync(&rt_rem, app_rem.clone());
            }
        }
    });

    // 4. Battery, ICS calendar & Cloud sync loop (boot + every 60s)
    let app_periodic = app.clone();
    let rt_periodic = Arc::clone(&runtime);
    tauri::async_runtime::spawn(async move {
        loop {
            let bat = read_battery_status();
            *rt_periodic.platform.battery.lock().unwrap() = bat;

            refresh_calendar_once(&rt_periodic).await;

            if rt_periodic.sync.is_signed_in() {
                rt_periodic.sync.sync_now(Arc::clone(&rt_periodic.core)).await;
            }

            emit_snapshot(&app_periodic, &rt_periodic);
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });

    // 5. GitHub Update check once 5s after startup
    let app_upd = app.clone();
    let rt_upd = Arc::clone(&runtime);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let should_check = rt_upd.core.lock().unwrap().settings.data.check_for_updates;
        if should_check {
            if let Ok(rel) = fetch_latest_release().await {
                let mut u = rt_upd.platform.updater.lock().unwrap();
                u.available = rel;
                u.last_checked_ms = now_ms();
            }
            emit_snapshot(&app_upd, &rt_upd);
        }
    });

    // 6. CLI dev switches
    if let Some((secs, target_tab)) = runtime.startup_flags.tab_after {
        let app_tab = app.clone();
        let rt_tab = Arc::clone(&runtime);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(secs)).await;
            rt_tab.ui.lock().unwrap().tab = target_tab;
            emit_snapshot(&app_tab, &rt_tab);
        });
    }
    if let Some(secs) = runtime.startup_flags.close_after_secs {
        let app_close = app.clone();
        let rt_close = Arc::clone(&runtime);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(secs)).await;
            close_panel(&app_close, &rt_close);
        });
    }
    if let Some(secs) = runtime.startup_flags.remind_in_secs {
        let rt_ri = Arc::clone(&runtime);
        let mut guard = rt_ri.core.lock().unwrap();
        let c = &mut *guard;
        let due_secs = now_secs() + (secs as f64);
        if let Some(t) = c.tasks.add(&c.db, "Test reminder from --remind-in", today_start(), None) {
            c.tasks.set_reminder(&c.db, &t.id, Some(due_secs));
        }
    }
}

pub async fn refresh_calendar_once(runtime: &AppRuntime) {
    let ics_url = std::env::var("NOTCHCAT_ICS_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| runtime.core.lock().unwrap().settings.data.ics_url.clone())
        .unwrap_or_default();

    let trimmed = ics_url.trim().to_string();
    if trimmed.is_empty() {
        let mut cal = runtime.platform.calendar.lock().unwrap();
        cal.access = CalendarAccess::NotDetermined;
        cal.events.clear();
        return;
    }

    match fetch_ics_text(&trimmed).await {
        Ok(text) => {
            let start_s = today_start();
            let end_s = add_local_days(start_s, 1);
            let events =
                parse_ics_events_for_range(&text, (start_s * 1000.0) as i64, (end_s * 1000.0) as i64);
            let mut cal = runtime.platform.calendar.lock().unwrap();
            cal.access = CalendarAccess::Authorized;
            cal.events = events;
        }
        Err(err) => {
            let mut cal = runtime.platform.calendar.lock().unwrap();
            cal.access = CalendarAccess::Failed(err);
        }
    }
}

// ---------------------------------------------------------------------------
// Win32 Sleep / Wake Listener (WM_POWERBROADCAST)
// ---------------------------------------------------------------------------

static POWER_CTX: OnceLock<(AppHandle, Arc<AppRuntime>, isize)> = OnceLock::new();

pub fn spawn_power_listener(app: AppHandle, runtime: Arc<AppRuntime>) {
    std::thread::spawn(move || unsafe {
        let class_name: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let win_name: Vec<u16> = "NotchCatPowerListener\0".encode_utf16().collect();
        let Ok(hwnd) = CreateWindowExW(
            Default::default(),
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR(win_name.as_ptr()),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        ) else {
            return;
        };

        let prev_proc = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, power_wnd_proc as *const () as isize);
        let _ = POWER_CTX.set((app, runtime, prev_proc));

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn power_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_POWERBROADCAST {
        if let Some((app, rt, _)) = POWER_CTX.get() {
            const PBT_APMSUSPEND: usize = 0x0004;
            const PBT_APMRESUMESUSPEND: usize = 0x0007;
            const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
            match wparam.0 {
                PBT_APMSUSPEND => {
                    if rt.core.lock().unwrap().timer.on_sleep() {
                        emit_snapshot(app, rt);
                    }
                }
                PBT_APMRESUMESUSPEND | PBT_APMRESUMEAUTOMATIC => {
                    if rt.core.lock().unwrap().timer.on_wake() {
                        emit_snapshot(app, rt);
                    }
                }
                _ => {}
            }
        }
    }
    if let Some((_, _, prev)) = POWER_CTX.get() {
        if *prev != 0 {
            let f: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
                std::mem::transmute(*prev);
            return CallWindowProcW(Some(f), hwnd, msg, wparam, lparam);
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::HOTKEYS;
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::Shortcut;

    #[test]
    fn hotkeys_parse_cleanly() {
        for hk in HOTKEYS {
            assert!(
                Shortcut::from_str(hk).is_ok(),
                "expected global shortcut '{hk}' to parse"
            );
        }
    }
}
