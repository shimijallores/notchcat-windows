use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::db::Db;
use crate::insights::{compute_insights, load_history};
use crate::models::{CardID, FakeNotchStyle, FocusSession};
use crate::platform::{
    fetch_latest_release, is_newer_version, process_artwork_bytes, release_from_bytes, MusicPlayer,
    MusicService,
};
use crate::repo;
use crate::store::{
    add_local_days, now_secs, today_start, AppSettings, NoteStore, TaskDay, TaskStore,
};
use crate::sync::{CredentialStore, DirectStores, SyncEngine};
use crate::timer::FocusTimer;

pub fn handle_cli_tooling() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--seed-demo") {
        let path = db_path_from_env();
        let db = Db::open(&path).expect("Failed to open NotchCat DB");
        seed_demo_data(&db);
        println!("Seeded demo data into {}", path.display());
        return Some(0);
    }
    if args.iter().any(|a| a == "--selftest-note") {
        return Some(run_selftest_note());
    }
    if args.iter().any(|a| a == "--selftest-settings") {
        return Some(run_selftest_settings());
    }
    if args.iter().any(|a| a == "--selftest-focus") {
        return Some(run_selftest_focus());
    }
    if args.iter().any(|a| a == "--selftest-update") {
        return Some(run_selftest_update());
    }
    if args.iter().any(|a| a == "--selftest-music") {
        return Some(run_selftest_music());
    }
    if args.iter().any(|a| a == "--selftest-sync") {
        return Some(run_selftest_sync());
    }
    None
}

pub fn db_path_from_env() -> PathBuf {
    Db::default_path()
}

pub fn seed_demo_data(db: &Db) {
    let today = today_start();
    let mut tasks = TaskStore::new(db);
    if !tasks.visible_tasks(TaskDay::Today).is_empty() {
        return;
    }

    if let Some(t1) = tasks.add(db, "Ship Windows island animations", today, Some(25)) {
        tasks.toggle_done(db, &t1.id);
    }
    let _ = tasks.add(db, "Verify pastel card palette", today, None);
    let _ = tasks.add(db, "Test ICS calendar feed", add_local_days(today, 1), Some(15));
    let _ = tasks.add(db, "Tag GitHub release v1.0.6", add_local_days(today, 2), None);

    let yesterday = add_local_days(today, -1);
    let two_days_ago = add_local_days(today, -2);
    for (day, mins) in [
        (yesterday, 50i64),
        (yesterday, 25),
        (two_days_ago, 45),
        (today, 25),
    ] {
        repo::session_insert(
            db,
            &FocusSession {
                task_id: None,
                started_at: day + 36000.0,
                active_seconds: mins * 60,
                planned_seconds: mins * 60,
                completed: true,
            },
        );
    }

    let mut note = NoteStore::new(db);
    if note.text.trim().is_empty() {
        note.set_text(
            db,
            "- Double-check Alt+N hotkey\n- Brew oolong tea\n- Review Insights chart".into(),
        );
    }
}

fn check(cond: bool, msg: &str) -> Result<(), String> {
    if cond {
        Ok(())
    } else {
        Err(msg.to_string())
    }
}

fn run_selftest_note() -> i32 {
    let run = || -> Result<(), String> {
        let db = Db::open_memory().map_err(|e| e.to_string())?;
        let mut tasks = TaskStore::new(&db);
        let mut note = NoteStore::new(&db);

        note.set_text(&db, "Ship Windows release\nRegular note line".into());
        let (next_text, _, title) = NoteStore::convert_line(&note.text, 0)
            .ok_or_else(|| "expected line 0 to convert".to_string())?;
        note.set_text(&db, next_text);
        let task = tasks.add(&db, &title, today_start(), None).unwrap();

        check(
            note.text == "Regular note line",
            &format!("unexpected note text: {:?}", note.text),
        )?;
        check(
            tasks.visible_tasks(TaskDay::Today).len() == 1,
            "expected converted task in today's list",
        )?;

        // Check conflict reconciliation preserves both devices' edits cleanly
        let merged = NoteStore::reconcile("Remote line", "Local line");
        check(
            merged.contains("Remote line") && merged.contains("Local line"),
            "reconcile must preserve both edits",
        )?;
        tasks.delete(&db, &task.id);
        Ok(())
    };
    match run() {
        Ok(()) => {
            println!("SELFTEST NOTE: PASSED");
            0
        }
        Err(e) => {
            eprintln!("SELFTEST NOTE: FAILED: {e}");
            1
        }
    }
}

fn run_selftest_settings() -> i32 {
    let run = || -> Result<(), String> {
        let path = std::env::temp_dir().join(format!("notchcat-settings-{}.db", Uuid::new_v4()));
        {
            let db = Db::open(&path).map_err(|e| e.to_string())?;
            let mut settings = AppSettings::new(&db);
            check(
                settings.data.fake_notch == FakeNotchStyle::Island,
                "expected Island default fakeNotch on Windows",
            )?;
            let mut next = settings.data.clone();
            next.show_week_tab = false;
            next.fake_notch = FakeNotchStyle::Notch;
            settings.replace_data(&db, next);

            for card in [
                CardID::Events,
                CardID::Music,
                CardID::Notepad,
                CardID::Timer,
            ] {
                settings.set_enabled(&db, card, false);
            }
            check(
                settings.visible_cards() == vec![CardID::Tasks],
                "expected Tasks left visible",
            )?;
            check(
                !settings.set_enabled(&db, CardID::Tasks, false),
                "disabling last remaining card must be refused",
            )?;
            settings.set_enabled(&db, CardID::Timer, true);
            settings.move_card(&db, CardID::Timer, -1);
            check(
                settings.visible_cards() == vec![CardID::Timer, CardID::Tasks],
                "expected Timer moved before Tasks",
            )?;
        }
        {
            let db2 = Db::open(&path).map_err(|e| e.to_string())?;
            let reloaded = AppSettings::new(&db2);
            check(
                !reloaded.data.show_week_tab,
                "expected showWeekTab=false persisted",
            )?;
            check(
                reloaded.data.fake_notch == FakeNotchStyle::Notch,
                "expected Notch style persisted",
            )?;
            check(
                reloaded.visible_cards() == vec![CardID::Timer, CardID::Tasks],
                "expected card order persisted",
            )?;
        }
        let _ = std::fs::remove_file(&path);
        Ok(())
    };
    match run() {
        Ok(()) => {
            println!("SELFTEST SETTINGS: PASSED");
            0
        }
        Err(e) => {
            eprintln!("SELFTEST SETTINGS: FAILED: {e}");
            1
        }
    }
}

fn run_selftest_focus() -> i32 {
    let run = || -> Result<(), String> {
        let db = Db::open_memory().map_err(|e| e.to_string())?;
        let mut tasks = TaskStore::new(&db);
        let task = tasks.add(&db, "Focus Test Task", today_start(), None).unwrap();
        let mut timer = FocusTimer::new(25);

        // Demo 1s timer finishes and auto-completes linked task
        timer.start_demo(&db, 1, Some(task.id.clone()));
        std::thread::sleep(std::time::Duration::from_millis(1150));
        let (_, outcome) = timer.tick(&db, &mut tasks, true, true);
        let out = outcome.ok_or_else(|| "expected timer finish outcome".to_string())?;
        check(
            out.completed_task_id.as_deref() == Some(task.id.as_str()),
            "expected linked task auto-completed on timer finish",
        )?;
        check(
            tasks.task(&task.id).unwrap().is_done,
            "expected is_done=true",
        )?;

        let summary = compute_insights(&db, 0);
        check(
            summary.completed >= 1,
            "expected >= 1 completed task in Insights",
        )?;
        let hist = load_history(&db, "", 60);
        check(
            hist.sections
                .iter()
                .any(|s| s.items.iter().any(|i| i.title == "Focus Test Task")),
            "expected completed task in History",
        )?;

        // Abandoned short timer doesn't complete task
        let task2 = tasks.add(&db, "Abandoned Task", today_start(), None).unwrap();
        timer.start(&db, Some(25), Some(task2.id.clone()));
        timer.reset(&db);
        check(
            !tasks.task(&task2.id).unwrap().is_done,
            "expected reset timer not to complete task",
        )?;
        Ok(())
    };
    match run() {
        Ok(()) => {
            println!("SELFTEST FOCUS: PASSED");
            0
        }
        Err(e) => {
            eprintln!("SELFTEST FOCUS: FAILED: {e}");
            1
        }
    }
}

fn run_selftest_update() -> i32 {
    let run = || -> Result<(), String> {
        check(is_newer_version("v1.0.7", "1.0.6"), "v1.0.7 > 1.0.6")?;
        check(is_newer_version("2.0.0", "1.9.9"), "2.0.0 > 1.9.9")?;
        check(!is_newer_version("1.0.6", "1.0.6"), "1.0.6 == 1.0.6")?;

        let tmp = std::env::temp_dir().join(format!("notchcat-update-{}.json", Uuid::new_v4()));
        let json = r#"{
            "tag_name": "v9.9.9",
            "html_url": "https://github.com/PranavSathy/notchcat-windows/releases/v9.9.9",
            "assets": [
                {"name": "NotchCat_9.9.9_x64-setup.exe", "browser_download_url": "https://example.com/NotchCat_9.9.9_x64-setup.exe"}
            ]
        }"#;
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
        let rel = release_from_bytes(&bytes).ok_or("failed release_from_bytes")?;
        check(rel.version == "9.9.9", "expected 9.9.9")?;
        check(rel.url.ends_with(".exe"), "expected .exe asset URL")?;

        std::env::set_var("NOTCHCAT_UPDATE_URL", tmp.to_string_lossy().as_ref());
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        let fetched = rt.block_on(fetch_latest_release())?;
        let _ = std::fs::remove_file(&tmp);
        check(
            fetched.as_ref().map(|r| r.version.as_str()) == Some("9.9.9"),
            "expected fetch_latest_release to find v9.9.9",
        )?;
        Ok(())
    };
    match run() {
        Ok(()) => {
            println!("SELFTEST UPDATE: PASSED");
            0
        }
        Err(e) => {
            eprintln!("SELFTEST UPDATE: FAILED: {e}");
            1
        }
    }
}

fn run_selftest_music() -> i32 {
    let run = || -> Result<(), String> {
        let mut music = MusicService::new();
        let mut sp = serde_json::Map::new();
        sp.insert("Player State".into(), "Playing".into());
        sp.insert("Name".into(), "Soft Rain".into());
        sp.insert("Artist".into(), "Neko".into());
        sp.insert("Album".into(), "Window".into());
        sp.insert("Duration".into(), serde_json::json!(210.0));
        sp.insert("Playback Position".into(), serde_json::json!(15.0));
        music.handle_notification("com.spotify.client.PlaybackStateChanged", None, &sp, 10_000);
        check(
            music.track.as_ref().map(|t| t.title.as_str()) == Some("Soft Rain"),
            "expected Spotify track active",
        )?;
        check(
            music.track.as_ref().unwrap().position_at(20_000) == 25.0,
            "expected interpolated elapsed 25s",
        )?;

        let mut am = serde_json::Map::new();
        am.insert("Player State".into(), "Paused".into());
        am.insert("Name".into(), "Ignore Me".into());
        am.insert("Total Time".into(), serde_json::json!(120_000.0));
        music.handle_notification("com.apple.Music.playerInfo", None, &am, 21_000);
        check(
            music.track.as_ref().unwrap().player == MusicPlayer::Spotify,
            "paused secondary player should not clobber active Spotify",
        )?;

        am.insert("Player State".into(), "Playing".into());
        am.insert("Name".into(), "Takes Over".into());
        music.handle_notification("com.apple.Music.playerInfo", None, &am, 22_000);
        check(
            music.track.as_ref().unwrap().title == "Takes Over",
            "playing Apple Music should take over",
        )?;
        check(
            music.track.as_ref().unwrap().duration == 120.0,
            "Apple Music ms duration should convert to 120s",
        )?;

        let img = image::RgbImage::from_pixel(4, 4, image::Rgb([100, 140, 200]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let (url, tint) = process_artwork_bytes(&png_bytes);
        check(url.is_some(), "expected artwork data URL")?;
        let t = tint.ok_or("expected pastel tint")?;
        check(
            t[0] > 180 && t[1] > 190 && t[2] > 210,
            &format!("expected lightened pastel tint, got {t:?}"),
        )?;
        Ok(())
    };
    match run() {
        Ok(()) => {
            println!("SELFTEST MUSIC: PASSED");
            0
        }
        Err(e) => {
            eprintln!("SELFTEST MUSIC: FAILED: {e}");
            1
        }
    }
}

fn run_selftest_sync() -> i32 {
    let server_url = std::env::var("NOTCHCAT_SERVER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8787".to_string());
    let access_key = match std::env::var("NOTCHCAT_TEST_KEY") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => {
            eprintln!("SELFTEST SYNC: FAILED: NOTCHCAT_TEST_KEY env var is required");
            return 1;
        }
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let res: Result<(), String> = rt.block_on(async move {
        let dir = std::env::temp_dir().join(format!("notchcat-sync-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let db_a_path = dir.join("device-a.db");
        let db_b_path = dir.join("device-b.db");

        let db_a = Db::open(&db_a_path).map_err(|e| e.to_string())?;
        let db_b = Db::open(&db_b_path).map_err(|e| e.to_string())?;

        let engine_a = SyncEngine::new(
            &db_a,
            CredentialStore::for_db(Some(&db_a_path), "refresh-a", true),
        );
        let engine_b = SyncEngine::new(
            &db_b,
            CredentialStore::for_db(Some(&db_b_path), "refresh-b", true),
        );

        let stores_a = Arc::new(Mutex::new(DirectStores {
            tasks: TaskStore::new(&db_a),
            note: NoteStore::new(&db_a),
            settings: AppSettings::new(&db_a),
            db: db_a,
        }));
        let stores_b = Arc::new(Mutex::new(DirectStores {
            tasks: TaskStore::new(&db_b),
            note: NoteStore::new(&db_b),
            settings: AppSettings::new(&db_b),
            db: db_b,
        }));

        let username = format!("cat{}", std::process::id());
        let password = "SuperSecretPassword123!";
        engine_a.check_access_key(&server_url, &access_key).await?;

        // Device A creates pre-existing local data and signs up
        let task_a_id = {
            let mut guard = stores_a.lock().unwrap();
            let s = &mut *guard;
            let t = s.tasks.add(&s.db, "Synced from A", today_start(), Some(25)).unwrap();
            s.note.set_text(&s.db, "Hello from Device A".into());
            let mut next = s.settings.data.clone();
            next.default_focus_minutes = 35;
            s.settings.replace_data(&s.db, next);
            repo::session_insert(
                &s.db,
                &FocusSession {
                    task_id: Some(t.id.clone()),
                    started_at: now_secs() - 300.0,
                    active_seconds: 300,
                    planned_seconds: 300,
                    completed: true,
                },
            );
            t.id
        };

        engine_a
            .sign_up(Arc::clone(&stores_a), &server_url, &access_key, &username, password)
            .await?;

        // Device B signs in and pulls everything
        engine_b
            .sign_in(Arc::clone(&stores_b), &server_url, &username, password)
            .await?;

        {
            let s_b = stores_b.lock().unwrap();
            let task_b = s_b.tasks.task(&task_a_id);
            check(task_b.is_some(), "Device B should have task from Device A")?;
            check(
                s_b.note.text == "Hello from Device A",
                "Device B should have note from Device A",
            )?;
            check(
                s_b.settings.data.default_focus_minutes == 35,
                "Device B should have default_focus_minutes=35 from Device A",
            )?;
            let ins_b = compute_insights(&s_b.db, 0);
            check(
                ins_b.focus_seconds >= 300,
                "Device B should have >= 300 focus seconds synced from A",
            )?;
        }

        // Device B toggles done, adds new task, updates note & timer_sound, syncs
        let task_b_id = {
            let mut guard = stores_b.lock().unwrap();
            let s_b = &mut *guard;
            s_b.tasks.toggle_done(&s_b.db, &task_a_id);
            let t2 = s_b.tasks.add(&s_b.db, "Added on B", today_start(), None).unwrap();
            s_b.note.set_text(&s_b.db, "Updated by Device B".into());
            let mut next = s_b.settings.data.clone();
            next.timer_sound = false;
            s_b.settings.replace_data(&s_b.db, next);
            t2.id
        };
        engine_b.sync_now(Arc::clone(&stores_b)).await;

        // Device A pulls updates from B
        engine_a.sync_now(Arc::clone(&stores_a)).await;
        {
            let s_a = stores_a.lock().unwrap();
            let t1 = s_a.tasks.task(&task_a_id).unwrap();
            check(t1.is_done, "Device A should see task completed by B")?;
            check(
                s_a.tasks.task(&task_b_id).is_some(),
                "Device A should see task added on B",
            )?;
            check(
                s_a.note.text == "Updated by Device B",
                "Device A should see note updated by B",
            )?;
            check(
                !s_a.settings.data.timer_sound,
                "Device A should see timer_sound=false updated by B",
            )?;
        }

        // Device A deletes task_b_id -> Device B syncs -> gone
        {
            let mut guard = stores_a.lock().unwrap();
            let s_a = &mut *guard;
            s_a.tasks.delete(&s_a.db, &task_b_id);
        }
        engine_a.sync_now(Arc::clone(&stores_a)).await;
        engine_b.sync_now(Arc::clone(&stores_b)).await;
        {
            let s_b = stores_b.lock().unwrap();
            check(
                s_b.tasks.task(&task_b_id).is_none(),
                "Device B should tombstone-delete task removed on A",
            )?;
        }

        engine_a.sign_out(Arc::clone(&stores_a)).await;
        engine_b.sign_out(Arc::clone(&stores_b)).await;
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    });

    match res {
        Ok(()) => {
            println!("SELFTEST SYNC: ALL PASSED");
            0
        }
        Err(e) => {
            eprintln!("SELFTEST SYNC: FAILED: {e}");
            1
        }
    }
}
