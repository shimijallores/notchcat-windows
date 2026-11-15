use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::insights::{load_history, HistoryPage};
use crate::island::Tab;
use crate::models::{CardID, FakeNotchStyle, SettingsData};
use crate::platform::{
    fetch_latest_release, launch_spotify_app, open_external_url, play_timer_sound, reveal_folder,
    send_smtc_control, set_launch_at_login, MediaControlAction,
};
use crate::state::{AppRuntime, AppSnapshot};
use crate::store::{now_ms, today_start, NoteStore};
use crate::window::{close_panel, emit_snapshot, open_panel, refresh_calendar_once, sync_hotkey_registration};

type Rt<'a> = State<'a, Arc<AppRuntime>>;

fn sync_and_emit(app: &AppHandle, rt: &Arc<AppRuntime>) {
    emit_snapshot(app, rt);
    AppRuntime::trigger_debounced_sync(rt, app.clone());
}

#[tauri::command]
pub fn get_snapshot(rt: Rt) -> AppSnapshot {
    rt.build_snapshot()
}

#[tauri::command]
pub fn open_tray(app: AppHandle, rt: Rt, pin: bool) {
    open_panel(&app, &rt, pin);
}

#[tauri::command]
pub fn close_tray(app: AppHandle, rt: Rt) {
    close_panel(&app, &rt);
}

#[tauri::command]
pub fn set_tab(app: AppHandle, rt: Rt, tab: String) {
    if let Some(t) = Tab::from_cli(&tab) {
        let mut ui = rt.ui.lock().unwrap();
        ui.tab = t;
    }
    emit_snapshot(&app, &rt);
}

#[tauri::command]
pub fn set_insights_week_offset(app: AppHandle, rt: Rt, offset: i64) {
    rt.ui.lock().unwrap().insights_week_offset = offset.min(0);
    emit_snapshot(&app, &rt);
}

#[tauri::command]
pub fn set_modal_open(app: AppHandle, rt: Rt, open: bool) {
    rt.ui.lock().unwrap().modal_open = open;
    emit_snapshot(&app, &rt);
}

#[tauri::command]
pub fn set_drag_active(rt: Rt, active: bool) {
    rt.ui.lock().unwrap().drag_active = active;
}

#[tauri::command]
pub fn dismiss_compact(app: AppHandle, rt: Rt) {
    {
        let mut ui = rt.ui.lock().unwrap();
        ui.compact = None;
    }
    emit_snapshot(&app, &rt);
}

// ---------- Tasks ----------

#[tauri::command]
pub fn add_task(
    app: AppHandle,
    rt: Rt,
    title: String,
    day: Option<f64>,
    time_limit_minutes: Option<i64>,
) {
    let target_day = day.unwrap_or_else(today_start);
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        if c.tasks.add(&c.db, &title, target_day, time_limit_minutes).is_some() {
            c.invalidate_insights();
        }
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn toggle_task(app: AppHandle, rt: Rt, id: String) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        if c.tasks.toggle_done(&c.db, &id) {
            c.invalidate_insights();
        }
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn rename_task(app: AppHandle, rt: Rt, id: String, title: String) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        c.tasks.rename(&c.db, &id, &title);
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn set_task_time_limit(app: AppHandle, rt: Rt, id: String, minutes: Option<i64>) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        c.tasks.set_time_limit(&c.db, &id, minutes);
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn move_task_to_date(app: AppHandle, rt: Rt, id: String, day: f64) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        if c.tasks.move_to_date(&c.db, &id, day) {
            c.invalidate_insights();
        }
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn duplicate_task(app: AppHandle, rt: Rt, id: String) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        if c.tasks.duplicate(&c.db, &id).is_some() {
            c.invalidate_insights();
        }
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn delete_task(app: AppHandle, rt: Rt, id: String) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        if c.timer.linked_task_id.as_deref() == Some(id.as_str()) {
            c.timer.reset(&c.db);
        }
        if c.tasks.delete(&c.db, &id) {
            c.invalidate_insights();
        }
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn reorder_tasks(app: AppHandle, rt: Rt, day: f64, ordered_ids: Vec<String>) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        c.tasks.reorder_visible(&c.db, day, &ordered_ids);
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn set_task_reminder(app: AppHandle, rt: Rt, id: String, remind_at: Option<f64>) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        c.tasks.set_reminder(&c.db, &id, remind_at);
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn restore_task(app: AppHandle, rt: Rt, id: String) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        if c.tasks.restore(&c.db, &id) {
            c.invalidate_insights();
        }
    }
    sync_and_emit(&app, &rt);
}

// ---------- Reminders ----------

#[tauri::command]
pub fn reminder_complete(app: AppHandle, rt: Rt, task_id: String) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        c.tasks.mark_done(&c.db, &task_id);
        c.invalidate_insights();
    }
    {
        let mut ui = rt.ui.lock().unwrap();
        ui.pending_reminder = None;
        ui.compact = None;
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn reminder_snooze(app: AppHandle, rt: Rt, task_id: String) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        let mins = c.settings.data.snooze_minutes;
        c.tasks.snooze(&c.db, &task_id, mins);
    }
    {
        let mut ui = rt.ui.lock().unwrap();
        ui.pending_reminder = None;
        ui.compact = None;
    }
    sync_and_emit(&app, &rt);
}

#[tauri::command]
pub fn reminder_dismiss(app: AppHandle, rt: Rt) {
    {
        let mut ui = rt.ui.lock().unwrap();
        ui.pending_reminder = None;
        ui.compact = None;
    }
    emit_snapshot(&app, &rt);
}

// ---------- Focus Timer ----------

#[tauri::command]
pub fn timer_start(
    app: AppHandle,
    rt: Rt,
    minutes: Option<i64>,
    task_id: Option<String>,
) {
    let logged = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        let logged = c.timer.start(&c.db, minutes, task_id);
        if logged {
            c.invalidate_insights();
        }
        logged
    };
    rt.ui.lock().unwrap().compact = None;
    if logged {
        sync_and_emit(&app, &rt);
    } else {
        emit_snapshot(&app, &rt);
    }
}

#[tauri::command]
pub fn timer_toggle(app: AppHandle, rt: Rt) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        if c.timer.toggle(&c.db) {
            c.invalidate_insights();
        }
    }
    rt.ui.lock().unwrap().compact = None;
    emit_snapshot(&app, &rt);
}

#[tauri::command]
pub fn timer_reset(app: AppHandle, rt: Rt) {
    let logged = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        let logged = c.timer.reset(&c.db);
        if logged {
            c.invalidate_insights();
        }
        logged
    };
    rt.ui.lock().unwrap().compact = None;
    if logged {
        sync_and_emit(&app, &rt);
    } else {
        emit_snapshot(&app, &rt);
    }
}

#[tauri::command]
pub fn timer_set_duration(app: AppHandle, rt: Rt, minutes: i64) {
    let logged = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        let logged = c.timer.set_duration(&c.db, minutes);
        if logged {
            c.invalidate_insights();
        }
        logged
    };
    if logged {
        sync_and_emit(&app, &rt);
    } else {
        emit_snapshot(&app, &rt);
    }
}

#[tauri::command]
pub fn timer_add_time(app: AppHandle, rt: Rt, seconds: i64) {
    let (outcome, timer_sound) = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        let tui = c.settings.data.time_up_island;
        let ts = c.settings.data.timer_sound;
        let (_, outcome) = c.timer.add_time(&c.db, &mut c.tasks, seconds, tui, ts);
        if outcome.is_some() {
            c.invalidate_insights();
        }
        (outcome, ts)
    };
    rt.ui.lock().unwrap().compact = None;
    if let Some(out) = outcome {
        if out.play_sound && timer_sound {
            play_timer_sound("Glass");
        }
        sync_and_emit(&app, &rt);
    } else {
        emit_snapshot(&app, &rt);
    }
}

// ---------- Notepad ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertLineResult {
    pub converted: bool,
    pub next_text: String,
    pub cursor_utf16: usize,
}

#[tauri::command]
pub fn note_set_text(app: AppHandle, rt: Rt, text: String) {
    let changed = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        if c.note.text != text {
            let prev = c.note.text.clone();
            c.note_undo.push((prev, None));
            if c.note_undo.len() > 25 {
                c.note_undo.remove(0);
            }
        }
        c.note.set_text(&c.db, text)
    };
    if changed {
        sync_and_emit(&app, &rt);
    }
}

#[tauri::command]
pub fn note_convert_line(app: AppHandle, rt: Rt, cursor_utf16: usize) -> ConvertLineResult {
    let result = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        let prev_text = c.note.text.clone();
        if let Some((next_text, new_cursor, title)) =
            NoteStore::convert_line(&prev_text, cursor_utf16)
        {
            c.note.set_text(&c.db, next_text.clone());
            let task_id = c
                .tasks
                .add(&c.db, &title, today_start(), None)
                .map(|t| t.id);
            c.note_undo.push((prev_text, task_id));
            if c.note_undo.len() > 25 {
                c.note_undo.remove(0);
            }
            c.invalidate_insights();
            ConvertLineResult {
                converted: true,
                next_text,
                cursor_utf16: new_cursor,
            }
        } else {
            ConvertLineResult {
                converted: false,
                next_text: prev_text,
                cursor_utf16,
            }
        }
    };
    if result.converted {
        sync_and_emit(&app, &rt);
    }
    result
}

#[tauri::command]
pub fn note_undo(app: AppHandle, rt: Rt) {
    let did = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        if let Some((prev_text, maybe_task_id)) = c.note_undo.pop() {
            c.note.set_text(&c.db, prev_text);
            if let Some(tid) = maybe_task_id {
                c.tasks.delete(&c.db, &tid);
                c.invalidate_insights();
            }
            true
        } else {
            false
        }
    };
    if did {
        sync_and_emit(&app, &rt);
    }
}

// ---------- History ----------

#[tauri::command]
pub fn get_history(rt: Rt, search: String, page_size: i64) -> HistoryPage {
    let core = rt.core.lock().unwrap();
    load_history(&core.db, &search, page_size.max(10))
}

// ---------- Settings ----------

#[tauri::command]
pub fn settings_update(app: AppHandle, rt: Rt, data: SettingsData) {
    let (synced_changed, hotkey_changed, new_hotkey, ics_changed) = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        let old_hotkey = c.settings.data.hotkey_enabled;
        let old_ics = c.settings.data.ics_url.clone();
        let synced_changed = c.settings.replace_data(&c.db, data.clone());
        let new_hotkey = c.settings.data.hotkey_enabled;
        let hotkey_changed = old_hotkey != new_hotkey;
        let ics_changed = old_ics != c.settings.data.ics_url;
        (synced_changed, hotkey_changed, new_hotkey, ics_changed)
    };

    if hotkey_changed {
        sync_hotkey_registration(&app, Arc::clone(&*rt), new_hotkey);
    }
    if ics_changed {
        let rt_c = Arc::clone(&*rt);
        let app_c = app.clone();
        tauri::async_runtime::spawn(async move {
            refresh_calendar_once(&rt_c).await;
            emit_snapshot(&app_c, &rt_c);
        });
    }
    if synced_changed {
        sync_and_emit(&app, &rt);
    } else {
        emit_snapshot(&app, &rt);
    }
}

#[tauri::command]
pub fn settings_set_card_enabled(app: AppHandle, rt: Rt, card: CardID, enabled: bool) {
    let changed = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        c.settings.set_enabled(&c.db, card, enabled)
    };
    if changed {
        sync_and_emit(&app, &rt);
    }
}

#[tauri::command]
pub fn settings_move_card(app: AppHandle, rt: Rt, card: CardID, offset: isize) {
    let changed = {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        c.settings.move_card(&c.db, card, offset)
    };
    if changed {
        sync_and_emit(&app, &rt);
    }
}

#[tauri::command]
pub fn settings_set_launch_at_login(app: AppHandle, rt: Rt, enabled: bool) -> Result<(), String> {
    set_launch_at_login(enabled)?;
    emit_snapshot(&app, &rt);
    Ok(())
}

// ---------- Music ----------

#[tauri::command]
pub fn music_play_pause(app: AppHandle, rt: Rt) {
    let handled_fake = rt
        .platform
        .music
        .lock()
        .unwrap()
        .toggle_fake_play_pause(now_ms());
    if handled_fake {
        emit_snapshot(&app, &rt);
        return;
    }
    let rt_c = Arc::clone(&*rt);
    tauri::async_runtime::spawn(async move {
        let pf = Arc::clone(&rt_c.platform);
        let _ = tokio::task::spawn_blocking(move || {
            send_smtc_control(MediaControlAction::PlayPause);
            crate::platform::poll_music_now(&pf);
        })
        .await;
        emit_snapshot(&app, &rt_c);
    });
}

#[tauri::command]
pub fn music_next(app: AppHandle, rt: Rt) {
    let rt_c = Arc::clone(&*rt);
    tauri::async_runtime::spawn(async move {
        let pf = Arc::clone(&rt_c.platform);
        let _ = tokio::task::spawn_blocking(move || {
            send_smtc_control(MediaControlAction::Next);
            crate::platform::poll_music_now(&pf);
        })
        .await;
        emit_snapshot(&app, &rt_c);
    });
}

#[tauri::command]
pub fn music_previous(app: AppHandle, rt: Rt) {
    let rt_c = Arc::clone(&*rt);
    tauri::async_runtime::spawn(async move {
        let pf = Arc::clone(&rt_c.platform);
        let _ = tokio::task::spawn_blocking(move || {
            send_smtc_control(MediaControlAction::Previous);
            crate::platform::poll_music_now(&pf);
        })
        .await;
        emit_snapshot(&app, &rt_c);
    });
}

#[tauri::command]
pub fn music_launch_player() {
    launch_spotify_app();
}

// ---------- Calendar & Updates ----------

#[tauri::command]
pub async fn calendar_refresh(app: AppHandle, rt: Rt<'_>) -> Result<(), String> {
    refresh_calendar_once(&rt).await;
    emit_snapshot(&app, &rt);
    Ok(())
}

#[tauri::command]
pub async fn update_check(app: AppHandle, rt: Rt<'_>) -> Result<(), String> {
    match fetch_latest_release().await {
        Ok(rel) => {
            let mut u = rt.platform.updater.lock().unwrap();
            u.status_message = if let Some(ref r) = rel {
                Some(format!("Update {} available", r.version))
            } else {
                Some("You're up to date".to_string())
            };
            u.available = rel;
            u.last_checked_ms = now_ms();
        }
        Err(e) => {
            let mut u = rt.platform.updater.lock().unwrap();
            u.status_message = Some(format!("Update check failed: {e}"));
        }
    }
    emit_snapshot(&app, &rt);
    Ok(())
}

#[tauri::command]
pub fn open_url(url: String) {
    open_external_url(&url);
}

#[tauri::command]
pub fn reveal_data_folder(rt: Rt) {
    if let Some(dir) = rt.db_path.parent() {
        reveal_folder(dir);
    }
}

#[tauri::command]
pub fn backup_database(rt: Rt, dest_path: Option<String>) -> Result<String, String> {
    let target = match dest_path {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => {
            let dir = rt
                .db_path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
            dir.join(format!("notchcat-backup-{ts}.sqlite"))
        }
    };
    let core = rt.core.lock().unwrap();
    core.db.backup_to(&target).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}

// ---------- Cloud Sync & Account ----------

#[tauri::command]
pub async fn sync_check_key(rt: Rt<'_>, server_url: String, key: String) -> Result<(), String> {
    rt.sync.check_access_key(&server_url, &key).await
}

#[tauri::command]
pub async fn sync_sign_up(
    app: AppHandle,
    rt: Rt<'_>,
    server_url: String,
    access_key: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let res = rt
        .sync
        .sign_up(Arc::clone(&rt.core), &server_url, &access_key, &username, &password)
        .await;
    emit_snapshot(&app, &rt);
    res
}

#[tauri::command]
pub async fn sync_sign_in(
    app: AppHandle,
    rt: Rt<'_>,
    server_url: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let res = rt
        .sync
        .sign_in(Arc::clone(&rt.core), &server_url, &username, &password)
        .await;
    emit_snapshot(&app, &rt);
    res
}

#[tauri::command]
pub async fn sync_sign_out(app: AppHandle, rt: Rt<'_>) -> Result<(), String> {
    rt.sync.sign_out(Arc::clone(&rt.core)).await;
    emit_snapshot(&app, &rt);
    Ok(())
}

#[tauri::command]
pub async fn sync_now(app: AppHandle, rt: Rt<'_>) -> Result<(), String> {
    rt.sync.sync_now(Arc::clone(&rt.core)).await;
    emit_snapshot(&app, &rt);
    Ok(())
}

#[tauri::command]
pub fn sync_set_server_url(app: AppHandle, rt: Rt, server_url: String) {
    let core = rt.core.lock().unwrap();
    rt.sync.set_server_url(&core.db, &server_url);
    drop(core);
    emit_snapshot(&app, &rt);
}

#[tauri::command]
pub fn set_fake_notch_cli_override(app: AppHandle, rt: Rt, style: FakeNotchStyle) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        let mut next = c.settings.data.clone();
        next.fake_notch = style;
        c.settings.replace_data(&c.db, next);
    }
    emit_snapshot(&app, &rt);
}

#[tauri::command]
pub fn quit_app(app: AppHandle, rt: Rt) {
    {
        let mut guard = rt.core.lock().unwrap();
        let c = &mut *guard;
        c.timer.flush_for_quit(&c.db);
    }
    app.exit(0);
}
