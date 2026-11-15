use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::insights::{compute_insights, InsightsSummary};
use crate::island::{IslandGeometry, Tab};
use crate::models::{FakeNotchStyle, SettingsData, TaskItem, WireChange, WireRecord};
use crate::platform::{
    current_app_version, is_launch_at_login, BatteryStatus, CalendarSnapshot, MusicSnapshot,
    PlatformState, ReleaseInfo,
};
use crate::repo;
use crate::store::{
    add_local_days, now_ms, now_secs, today_start, AppSettings, NoteStore, TaskDay, TaskStore,
};
use crate::sync::{SessionSync, SyncEngine, SyncPhase, SyncStatusSnapshot, SyncStoresAdapter};
use crate::timer::{reminder_presets, FocusTimer, ReminderPresetOption, TimerSnapshot};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CompactBanner {
    TimesUp {
        sound: bool,
        #[serde(rename = "taskTitle")]
        task_title: Option<String>,
    },
    Reminder {
        #[serde(rename = "taskId")]
        task_id: String,
        title: String,
    },
}

pub struct UiState {
    pub is_open: bool,
    pub pinned: bool,
    pub tab: Tab,
    pub insights_week_offset: i64,
    pub compact: Option<CompactBanner>,
    pub pending_reminder: Option<TaskItem>,
    pub modal_open: bool,
    pub drag_active: bool,
    pub hover_armed: bool,
    pub hover_enter_at: Option<Instant>,
    pub exit_ticks: u32,
    pub suppress_auto_close_until: Option<Instant>,
    pub overlay_w: f64,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            is_open: false,
            pinned: false,
            tab: Tab::Workspace,
            insights_week_offset: 0,
            compact: None,
            pending_reminder: None,
            modal_open: false,
            drag_active: false,
            hover_armed: true,
            hover_enter_at: None,
            exit_ticks: 0,
            suppress_auto_close_until: None,
            overlay_w: IslandGeometry::OVERLAY_MAX_W,
        }
    }
}

pub struct CoreState {
    pub db: Db,
    pub tasks: TaskStore,
    pub note: NoteStore,
    pub note_undo: Vec<(String, Option<String>)>, // (prev_text, optional converted task_id to undo)
    pub settings: AppSettings,
    pub timer: FocusTimer,
    pub insights_cache: InsightsSummary,
    pub insights_week_cached: i64,
    pub insights_dirty: bool,
}

impl CoreState {
    pub fn new(db: Db) -> Self {
        let tasks = TaskStore::new(&db);
        let note = NoteStore::new(&db);
        let settings = AppSettings::new(&db);
        let default_mins = settings.data.default_focus_minutes.max(1);
        let timer = FocusTimer::new(default_mins);
        let insights_cache = compute_insights(&db, 0);

        Self {
            db,
            tasks,
            note,
            note_undo: Vec::new(),
            settings,
            timer,
            insights_cache,
            insights_week_cached: 0,
            insights_dirty: false,
        }
    }

    pub fn invalidate_insights(&mut self) {
        self.insights_dirty = true;
    }

    pub fn insights(&mut self, week_offset: i64) -> InsightsSummary {
        let clamped = week_offset.min(0);
        if self.insights_dirty || self.insights_week_cached != clamped {
            self.insights_cache = compute_insights(&self.db, clamped);
            self.insights_week_cached = clamped;
            self.insights_dirty = false;
        }
        self.insights_cache.clone()
    }
}

impl SyncStoresAdapter for CoreState {
    fn db(&self) -> &Db {
        &self.db
    }

    fn pending_changes(&mut self, limit: usize) -> Vec<WireChange> {
        let mut all = self.tasks.pending_changes(limit);
        if all.len() < limit {
            all.extend(self.note.pending_changes(&self.db, limit - all.len()));
        }
        if all.len() < limit {
            all.extend(SessionSync::pending_changes(&self.db, limit - all.len()));
        }
        if all.len() < limit {
            all.extend(self.settings.pending_changes());
        }
        all
    }

    fn pending_count(&self) -> usize {
        self.tasks.pending_count()
            + self.note.pending_count()
            + (repo::session_unsent_count(&self.db).max(0) as usize)
            + self.settings.pending_count()
    }

    fn apply_records(&mut self, records: &[WireRecord]) {
        self.tasks.apply_remote(&self.db, records);
        self.note.apply(&self.db, records);
        let arrived = SessionSync::apply(&self.db, records);
        if arrived > 0 || records.iter().any(|r| r.collection == "tasks") {
            self.invalidate_insights();
        }
        self.settings.apply(&self.db, records);
    }

    fn apply_conflicts(&mut self, conflicts: &[WireRecord]) {
        self.note.resolve_conflicts(&self.db, conflicts);
    }

    fn drop_pending(&mut self, collection: &str, ids: &[String]) {
        match collection {
            "tasks" => self.tasks.drop_pending(&self.db, ids),
            "notes" => self.note.drop_pending(&self.db, ids),
            "sessions" => SessionSync::drop_pending(&self.db, ids),
            "settings" => self.settings.drop_pending(&self.db, ids),
            _ => {}
        }
    }

    fn enroll_all(&mut self) {
        self.tasks.enroll_all_for_sync(&self.db);
        self.note.enroll(&self.db);
        let _ = self.settings.enroll(&self.db);
    }

    fn reset_all_sync_state(&mut self) {
        self.tasks.reset_sync_state(&self.db);
        self.note.reset_sync_state(&self.db);
        SessionSync::reset_sync_state(&self.db);
        self.settings.reset_sync_state(&self.db);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekDaySnapshot {
    pub day: f64,
    pub tasks: Vec<TaskItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSnapshot {
    pub is_open: bool,
    pub pinned: bool,
    pub tab: String,
    pub insights_week_offset: i64,
    pub compact: Option<CompactBanner>,
    pub pending_reminder: Option<TaskItem>,
    pub overlay_w: f64,
    pub panel_w: f64,
    pub panel_h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub version: String,
    pub ui: UiSnapshot,
    pub today_start_secs: f64,
    pub today_tasks: Vec<TaskItem>,
    pub tomorrow_tasks: Vec<TaskItem>,
    pub week_days: Vec<WeekDaySnapshot>,
    pub today_reminders: Vec<TaskItem>,
    pub reminder_presets: Vec<ReminderPresetOption>,
    pub note_text: String,
    pub note_word_count: usize,
    pub note_has_undo: bool,
    pub timer: TimerSnapshot,
    pub settings: SettingsData,
    pub launch_at_login: bool,
    pub battery: BatteryStatus,
    pub music: MusicSnapshot,
    pub calendar: CalendarSnapshot,
    pub sync: SyncStatusSnapshot,
    pub update_available: Option<ReleaseInfo>,
    pub update_status_message: Option<String>,
    pub insights: InsightsSummary,
}

pub struct AppRuntime {
    pub db_path: PathBuf,
    pub core: Arc<Mutex<CoreState>>,
    pub ui: Mutex<UiState>,
    pub platform: Arc<PlatformState>,
    pub sync: Arc<SyncEngine>,
    pub sync_debounce_gen: Mutex<u64>,
    pub startup_flags: StartupFlags,
}

#[derive(Debug, Clone, Default)]
pub struct StartupFlags {
    pub open_mode: Option<bool>, // Some(true) pinned, Some(false) hover
    pub tab: Option<Tab>,
    pub tab_after: Option<(u64, Tab)>,
    pub demo: Option<i64>,
    pub close_after_secs: Option<u64>,
    pub remind_in_secs: Option<u64>,
    pub fake_notch: Option<FakeNotchStyle>,
    pub fake_music: bool,
    pub no_smtc: bool,
}

impl StartupFlags {
    pub fn from_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut flags = Self::default();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--open" => flags.open_mode = Some(true),
                "--open-hover" => flags.open_mode = Some(false),
                "--demo" => flags.demo = Some(flags.demo.unwrap_or(5)),
                "--fake-music" => flags.fake_music = true,
                "--no-smtc" => flags.no_smtc = true,
                "--tab" if i + 1 < args.len() => {
                    i += 1;
                    flags.tab = Tab::from_cli(&args[i]);
                }
                "--demo-secs" if i + 1 < args.len() => {
                    i += 1;
                    flags.demo = args[i].parse().ok().or(Some(5));
                }
                "--close-after" if i + 1 < args.len() => {
                    i += 1;
                    flags.close_after_secs = args[i].parse().ok();
                }
                "--remind-in" if i + 1 < args.len() => {
                    i += 1;
                    flags.remind_in_secs = args[i].parse().ok();
                }
                "--fake-notch" if i + 1 < args.len() => {
                    i += 1;
                    flags.fake_notch = match args[i].to_ascii_lowercase().as_str() {
                        "off" => Some(FakeNotchStyle::Off),
                        "notch" => Some(FakeNotchStyle::Notch),
                        "island" => Some(FakeNotchStyle::Island),
                        _ => None,
                    };
                }
                "--tab-after" if i + 2 < args.len() => {
                    let secs = args[i + 1].parse().ok();
                    let tab = Tab::from_cli(&args[i + 2]);
                    if let (Some(s), Some(t)) = (secs, tab) {
                        flags.tab_after = Some((s, t));
                    }
                    i += 2;
                }
                _ => {}
            }
            i += 1;
        }
        if std::env::var_os("NOTCHCAT_NO_APPLESCRIPT").is_some()
            || std::env::var_os("NOTCHCAT_NO_SMTC").is_some()
        {
            flags.no_smtc = true;
        }
        flags
    }
}

pub fn tab_to_str(tab: Tab) -> &'static str {
    match tab {
        Tab::Workspace => "workspace",
        Tab::Week => "week",
        Tab::Insights => "insights",
        Tab::Settings => "settings",
    }
}

impl AppRuntime {
    pub fn build_snapshot(&self) -> AppSnapshot {
        let now_millis = now_ms();
        let now_s = now_secs();
        let today = today_start();

        let week_offset = self.ui.lock().unwrap().insights_week_offset;

        let (
            today_tasks,
            tomorrow_tasks,
            week_days,
            today_reminders,
            note_text,
            note_word_count,
            note_has_undo,
            timer_snap,
            settings_data,
            visible_cards,
            insights,
            pending_count,
        ) = {
            let mut core = self.core.lock().unwrap();
            let visible = core.settings.visible_cards();
            let settings_data = core.settings.data.clone();
            let today_tasks = core.tasks.visible_tasks(TaskDay::Today);
            let tomorrow_tasks = core.tasks.visible_tasks(TaskDay::Tomorrow);
            let mut week_days = Vec::with_capacity(7);
            for offset in 0..7 {
                let d = add_local_days(today, offset);
                week_days.push(WeekDaySnapshot {
                    day: d,
                    tasks: core.tasks.tasks_on(d),
                });
            }
            let today_reminders = core.tasks.reminders_on(today);
            let note_text = core.note.text.clone();
            let note_word_count = core.note.word_count();
            let note_has_undo = !core.note_undo.is_empty();
            let timer_snap = core.timer.snapshot();
            let insights = core.insights(week_offset);
            let pending = core.pending_count();
            (
                today_tasks,
                tomorrow_tasks,
                week_days,
                today_reminders,
                note_text,
                note_word_count,
                note_has_undo,
                timer_snap,
                settings_data,
                visible,
                insights,
                pending,
            )
        };

        let ui_g = self.ui.lock().unwrap();
        let has_banner = ui_g.pending_reminder.is_some() || ui_g.modal_open;
        let expanded = IslandGeometry::expanded_rect(
            ui_g.overlay_w,
            settings_data.fake_notch,
            ui_g.tab,
            &visible_cards,
            has_banner,
        );
        let ui = UiSnapshot {
            is_open: ui_g.is_open,
            pinned: ui_g.pinned,
            tab: tab_to_str(ui_g.tab).to_string(),
            insights_week_offset: ui_g.insights_week_offset,
            compact: ui_g.compact.clone(),
            pending_reminder: ui_g.pending_reminder.clone(),
            overlay_w: ui_g.overlay_w,
            panel_w: expanded.w,
            panel_h: expanded.h,
        };
        drop(ui_g);

        let battery = self.platform.battery.lock().unwrap().clone();
        let music = self.platform.music.lock().unwrap().snapshot(now_millis);
        let calendar = self.platform.calendar.lock().unwrap().snapshot();
        let updater = self.platform.updater.lock().unwrap();
        let update_available = updater.available.clone();
        let update_status_message = updater.status_message.clone();
        drop(updater);

        let sync = self.sync.snapshot(pending_count);
        let _keep = &SyncPhase::Idle;

        AppSnapshot {
            version: current_app_version(),
            ui,
            today_start_secs: today,
            today_tasks,
            tomorrow_tasks,
            week_days,
            today_reminders,
            reminder_presets: reminder_presets(now_s),
            note_text,
            note_word_count,
            note_has_undo,
            timer: timer_snap,
            settings: settings_data,
            launch_at_login: is_launch_at_login(),
            battery,
            music,
            calendar,
            sync,
            update_available,
            update_status_message,
            insights,
        }
    }

    pub fn trigger_debounced_sync(runtime: &Arc<Self>, app: tauri::AppHandle) {
        if !runtime.sync.is_signed_in() {
            return;
        }
        let gen = {
            let mut g = runtime.sync_debounce_gen.lock().unwrap();
            *g += 1;
            *g
        };
        let rt = Arc::clone(runtime);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            let current_gen = *rt.sync_debounce_gen.lock().unwrap();
            if current_gen != gen {
                return;
            }
            rt.sync.sync_now(Arc::clone(&rt.core)).await;
            crate::window::emit_snapshot(&app, &rt);
        });
    }
}
