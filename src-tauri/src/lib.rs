pub mod commands;
pub mod db;
pub mod insights;
pub mod island;
pub mod models;
pub mod platform;
pub mod repo;
pub mod selftest;
pub mod state;
pub mod store;
pub mod sync;
pub mod timer;
pub mod window;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use db::Db;
use island::Tab;
use platform::PlatformState;
use selftest::{db_path_from_env, handle_cli_tooling};
use state::{AppRuntime, CoreState, StartupFlags, UiState};
use store::now_ms;
use sync::{CredentialStore, SyncEngine};
use window::{
    create_island_window, open_panel, setup_tray, spawn_background_loops, spawn_cursor_poller,
    spawn_power_listener, sync_hotkey_registration,
};

pub fn acquire_single_instance_lock(db_path: &std::path::Path) -> Option<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let lock_path = db_path.with_extension("lock");
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .share_mode(0)
        .open(&lock_path)
        .ok()
}

pub fn run() {
    if let Some(code) = handle_cli_tooling() {
        std::process::exit(code);
    }

    let flags = StartupFlags::from_args();
    let db_path = db_path_from_env();
    let instance_lock = match acquire_single_instance_lock(&db_path) {
        Some(lock) => lock,
        None => {
            let activate_path = db_path.with_extension("activate");
            let _ = std::fs::write(&activate_path, b"1");
            std::process::exit(0);
        }
    };
    let _ = std::fs::remove_file(db_path.with_extension("activate"));
    if std::env::var_os("NOTCHCAT_DB").is_none() {
        platform::ensure_startup_registered();
    }
    let db = Db::open_or_memory(&db_path);

    let mut core_state = CoreState::new(db);
    if let Some(style) = flags.fake_notch {
        let mut next = core_state.settings.data.clone();
        next.fake_notch = style;
        core_state.settings.replace_data(&core_state.db, next);
    }
    if let Some(demo_secs) = flags.demo {
        let first_task_id = core_state
            .tasks
            .tasks
            .iter()
            .find(|t| !t.is_done)
            .map(|t| t.id.clone());
        core_state
            .timer
            .start_demo(&core_state.db, demo_secs, first_task_id);
    }

    let platform = PlatformState::new();
    if flags.fake_music {
        platform.music.lock().unwrap().seed_fake_music(true, now_ms());
    }

    let creds = CredentialStore::for_db(Some(&db_path), "refresh-token", false);
    let sync_engine = Arc::new(SyncEngine::new(&core_state.db, creds));

    let mut ui_state = UiState::default();
    if let Some(t) = flags.tab {
        ui_state.tab = t;
    } else {
        ui_state.tab = Tab::Workspace;
    }
    if let Some(pin) = flags.open_mode {
        ui_state.is_open = true;
        ui_state.pinned = pin;
        ui_state.suppress_auto_close_until = Some(Instant::now() + Duration::from_secs(30));
    }

    let hotkey_enabled = core_state.settings.data.hotkey_enabled;

    let runtime = Arc::new(AppRuntime {
        db_path,
        core: Arc::new(Mutex::new(core_state)),
        ui: Mutex::new(ui_state),
        platform,
        sync: sync_engine,
        sync_debounce_gen: Mutex::new(0),
        startup_flags: flags.clone(),
    });

    let rt_setup = Arc::clone(&runtime);
    let rt_run = Arc::clone(&runtime);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Arc::clone(&runtime))
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::open_tray,
            commands::close_tray,
            commands::set_tab,
            commands::set_insights_week_offset,
            commands::set_modal_open,
            commands::set_drag_active,
            commands::dismiss_compact,
            commands::add_task,
            commands::toggle_task,
            commands::rename_task,
            commands::set_task_time_limit,
            commands::move_task_to_date,
            commands::duplicate_task,
            commands::delete_task,
            commands::reorder_tasks,
            commands::set_task_reminder,
            commands::restore_task,
            commands::reminder_complete,
            commands::reminder_snooze,
            commands::reminder_dismiss,
            commands::timer_start,
            commands::timer_toggle,
            commands::timer_reset,
            commands::timer_set_duration,
            commands::timer_add_time,
            commands::note_set_text,
            commands::note_convert_line,
            commands::note_undo,
            commands::get_history,
            commands::settings_update,
            commands::settings_set_card_enabled,
            commands::settings_move_card,
            commands::settings_set_launch_at_login,
            commands::music_play_pause,
            commands::music_next,
            commands::music_previous,
            commands::music_launch_player,
            commands::calendar_refresh,
            commands::update_check,
            commands::open_url,
            commands::reveal_data_folder,
            commands::backup_database,
            commands::sync_check_key,
            commands::sync_sign_up,
            commands::sync_sign_in,
            commands::sync_sign_out,
            commands::sync_now,
            commands::sync_set_server_url,
            commands::set_fake_notch_cli_override,
            commands::quit_app,
        ])
        .setup(move |app| {
            let handle = app.handle();
            create_island_window(handle, &rt_setup)?;
            setup_tray(handle, Arc::clone(&rt_setup))?;
            sync_hotkey_registration(handle, Arc::clone(&rt_setup), hotkey_enabled);
            spawn_cursor_poller(handle.clone(), Arc::clone(&rt_setup));
            spawn_background_loops(handle.clone(), Arc::clone(&rt_setup));
            spawn_power_listener(handle.clone(), Arc::clone(&rt_setup));

            if let Some(pin) = flags.open_mode {
                open_panel(handle, &rt_setup, pin);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building NotchCat Windows");

    app.run(move |_app_handle, event| {
        let _keep_lock = &instance_lock;
        if let tauri::RunEvent::ExitRequested { .. } = event {
            let mut guard = rt_run.core.lock().unwrap();
            let c = &mut *guard;
            c.timer.flush_for_quit(&c.db);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::acquire_single_instance_lock;
    use uuid::Uuid;

    #[test]
    fn single_instance_lock_prevents_second_acquisition() {
        let tmp_db =
            std::env::temp_dir().join(format!("notchcat-lock-test-{}.sqlite", Uuid::new_v4()));
        let first = acquire_single_instance_lock(&tmp_db);
        assert!(first.is_some(), "first instance should acquire lock");

        let second = acquire_single_instance_lock(&tmp_db);
        assert!(
            second.is_none(),
            "second instance must be blocked while first holds lock"
        );

        drop(first);
        let third = acquire_single_instance_lock(&tmp_db);
        assert!(
            third.is_some(),
            "lock should be acquirable after first instance exits"
        );
        drop(third);
        let _ = std::fs::remove_file(tmp_db.with_extension("lock"));
    }
}
