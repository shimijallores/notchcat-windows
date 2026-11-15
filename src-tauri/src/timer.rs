use crate::db::Db;
use crate::models::{FocusSession, TaskItem};
use crate::repo;
use crate::store::{now_secs, today_start, TaskStore};
use chrono::{Datelike, Local, NaiveTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimerState {
    Idle,
    Running,
    Paused,
    Finished,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerSnapshot {
    pub state: TimerState,
    pub remaining: i64,
    pub session_total: i64,
    pub linked_task_id: Option<String>,
    pub progress: f64,
}

pub struct TimerFinishOutcome {
    pub session_logged: bool,
    pub completed_task_id: Option<String>,
    pub show_time_up_island: bool,
    pub play_sound: bool,
}

pub struct FocusTimer {
    pub state: TimerState,
    pub remaining: i64,
    pub session_total: i64,
    pub linked_task_id: Option<String>,

    end_instant: Option<Instant>,
    paused_remaining: f64,
    paused_by_sleep: bool,

    session_started_at: Option<f64>,
    active_accumulated: f64,
    last_resume_instant: Option<Instant>,
}

impl FocusTimer {
    pub fn new(default_minutes: i64) -> Self {
        let secs = default_minutes.max(1) * 60;
        Self {
            state: TimerState::Idle,
            remaining: secs,
            session_total: secs,
            linked_task_id: None,
            end_instant: None,
            paused_remaining: secs as f64,
            paused_by_sleep: false,
            session_started_at: None,
            active_accumulated: 0.0,
            last_resume_instant: None,
        }
    }

    pub fn snapshot(&self) -> TimerSnapshot {
        let progress = if self.session_total > 0 {
            (1.0 - (self.remaining as f64) / (self.session_total as f64)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        TimerSnapshot {
            state: self.state,
            remaining: self.remaining,
            session_total: self.session_total,
            linked_task_id: self.linked_task_id.clone(),
            progress,
        }
    }

    pub fn is_active(&self) -> bool {
        self.state != TimerState::Idle
    }

    /// Starts a fresh session. Returns true if an abandoned prior session (>=30s) was logged.
    pub fn start(
        &mut self,
        db: &Db,
        minutes: Option<i64>,
        task_id: Option<String>,
    ) -> bool {
        let mut logged = false;
        if self.state == TimerState::Running || self.state == TimerState::Paused {
            logged = self.end_tracking(db, false);
        }
        if let Some(m) = minutes {
            self.session_total = m.max(1) * 60;
            self.remaining = self.session_total;
        } else if self.state == TimerState::Idle || self.state == TimerState::Finished {
            self.remaining = self.session_total;
        }
        self.linked_task_id = task_id;
        self.paused_remaining = self.remaining as f64;
        self.end_instant =
            Some(Instant::now() + std::time::Duration::from_secs_f64(self.paused_remaining));
        self.state = TimerState::Running;
        self.paused_by_sleep = false;
        self.begin_tracking();
        logged
    }

    pub fn start_demo(&mut self, db: &Db, seconds: i64, task_id: Option<String>) -> bool {
        let mut logged = self.reset(db);
        self.session_total = seconds.max(1);
        self.remaining = self.session_total;
        logged |= self.start(db, None, task_id);
        logged
    }

    pub fn toggle(&mut self, db: &Db) -> bool {
        match self.state {
            TimerState::Idle => self.start(db, None, None),
            TimerState::Running => {
                self.pause();
                false
            }
            TimerState::Paused => {
                self.resume();
                false
            }
            TimerState::Finished => false,
        }
    }

    pub fn pause(&mut self) {
        if self.state != TimerState::Running {
            return;
        }
        if let Some(end) = self.end_instant.take() {
            let now = Instant::now();
            self.paused_remaining = if end > now {
                (end - now).as_secs_f64()
            } else {
                0.0
            };
        }
        self.accumulate_active();
        self.state = TimerState::Paused;
    }

    pub fn resume(&mut self) {
        if self.state != TimerState::Paused {
            return;
        }
        self.end_instant =
            Some(Instant::now() + std::time::Duration::from_secs_f64(self.paused_remaining));
        self.state = TimerState::Running;
        self.last_resume_instant = Some(Instant::now());
    }

    pub fn reset(&mut self, db: &Db) -> bool {
        let logged = self.end_tracking(db, false);
        self.end_instant = None;
        self.linked_task_id = None;
        self.paused_by_sleep = false;
        self.remaining = self.session_total;
        self.paused_remaining = self.session_total as f64;
        self.state = TimerState::Idle;
        logged
    }

    pub fn set_duration(&mut self, db: &Db, minutes: i64) -> bool {
        let clamped = minutes.clamp(1, 240);
        let logged = self.reset(db);
        self.session_total = clamped * 60;
        self.remaining = self.session_total;
        self.paused_remaining = self.session_total as f64;
        logged
    }

    pub fn add_time(
        &mut self,
        db: &Db,
        tasks: &mut TaskStore,
        seconds: i64,
        time_up_island: bool,
        timer_sound: bool,
    ) -> (bool, Option<TimerFinishOutcome>) {
        match self.state {
            TimerState::Running => {
                if let Some(end) = self.end_instant {
                    self.end_instant =
                        Some(end + std::time::Duration::from_secs_f64(seconds.max(0) as f64));
                }
                self.session_total += seconds;
                let (changed, outcome) = self.tick(db, tasks, time_up_island, timer_sound);
                (changed || true, outcome)
            }
            TimerState::Paused => {
                self.paused_remaining = (self.paused_remaining + seconds as f64).max(0.0);
                self.session_total += seconds;
                self.remaining = self.paused_remaining.ceil() as i64;
                (true, None)
            }
            TimerState::Finished => {
                self.session_total = seconds.max(1);
                self.remaining = self.session_total;
                self.paused_remaining = self.session_total as f64;
                self.end_instant = Some(
                    Instant::now() + std::time::Duration::from_secs_f64(self.paused_remaining),
                );
                self.state = TimerState::Running;
                self.begin_tracking();
                (true, None)
            }
            TimerState::Idle => {
                self.session_total = (self.session_total + seconds).max(60);
                self.remaining = self.session_total;
                self.paused_remaining = self.session_total as f64;
                (true, None)
            }
        }
    }

    /// Ticks the timer. Returns `(state_or_display_changed, optional_finish_outcome)`.
    pub fn tick(
        &mut self,
        db: &Db,
        tasks: &mut TaskStore,
        time_up_island: bool,
        timer_sound: bool,
    ) -> (bool, Option<TimerFinishOutcome>) {
        if self.state != TimerState::Running {
            return (false, None);
        }
        let Some(end) = self.end_instant else {
            return (false, None);
        };
        let now = Instant::now();
        let left = if end > now {
            (end - now).as_secs_f64()
        } else {
            0.0
        };
        let whole = left.ceil().max(0.0) as i64;
        let changed = whole != self.remaining;
        if changed {
            self.remaining = whole;
        }
        if left <= 0.0 {
            let outcome = self.finish(db, tasks, time_up_island, timer_sound);
            return (true, Some(outcome));
        }
        (changed, None)
    }

    fn finish(
        &mut self,
        db: &Db,
        tasks: &mut TaskStore,
        time_up_island: bool,
        timer_sound: bool,
    ) -> TimerFinishOutcome {
        self.end_instant = None;
        self.remaining = 0;
        self.paused_remaining = 0.0;
        let session_logged = self.end_tracking(db, true);
        self.state = TimerState::Finished;
        let completed_task_id = if let Some(id) = self.linked_task_id.clone() {
            if tasks.mark_done(db, &id) {
                Some(id)
            } else {
                None
            }
        } else {
            None
        };
        TimerFinishOutcome {
            session_logged,
            completed_task_id,
            show_time_up_island: time_up_island,
            play_sound: timer_sound,
        }
    }

    fn begin_tracking(&mut self) {
        self.session_started_at = Some(now_secs());
        self.active_accumulated = 0.0;
        self.last_resume_instant = Some(Instant::now());
    }

    fn accumulate_active(&mut self) {
        if let Some(resumed) = self.last_resume_instant.take() {
            self.active_accumulated += resumed.elapsed().as_secs_f64();
        }
    }

    fn end_tracking(&mut self, db: &Db, completed: bool) -> bool {
        let Some(started_at) = self.session_started_at.take() else {
            return false;
        };
        self.accumulate_active();
        let active_seconds = self.active_accumulated.round() as i64;
        self.active_accumulated = 0.0;
        if !completed && active_seconds < 30 {
            return false;
        }
        repo::session_insert(
            db,
            &FocusSession {
                task_id: self.linked_task_id.clone(),
                started_at,
                active_seconds,
                planned_seconds: self.session_total,
                completed,
            },
        );
        true
    }

    pub fn flush_for_quit(&mut self, db: &Db) -> bool {
        if self.state == TimerState::Running || self.state == TimerState::Paused {
            self.end_tracking(db, false)
        } else {
            false
        }
    }

    pub fn on_sleep(&mut self) -> bool {
        if self.state == TimerState::Running {
            self.pause();
            self.paused_by_sleep = true;
            true
        } else {
            false
        }
    }

    pub fn on_wake(&mut self) -> bool {
        if self.paused_by_sleep {
            self.paused_by_sleep = false;
            self.resume();
            true
        } else {
            false
        }
    }
}

pub fn clock_string(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{:02}:{:02}", s / 60, s % 60)
}

// ---------- ReminderScheduler ----------

pub const REMINDER_STALE_AFTER_SECS: f64 = 6.0 * 3600.0;

pub fn poll_due_reminders(
    db: &Db,
    tasks: &mut TaskStore,
    reminders_enabled: bool,
    now: f64,
) -> Vec<TaskItem> {
    if !reminders_enabled {
        return vec![];
    }
    let due = tasks.due_reminders(now);
    let mut fire = Vec::new();
    for task in due {
        tasks.mark_reminded(db, &task.id);
        if let Some(at) = task.remind_at {
            if now - at > REMINDER_STALE_AFTER_SECS {
                continue;
            }
        }
        fire.push(task);
    }
    fire
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderPresetOption {
    pub id: String,
    pub title: String,
    pub timestamp: f64,
}

pub fn reminder_presets(now: f64) -> Vec<ReminderPresetOption> {
    let mut out = vec![
        ReminderPresetOption {
            id: "in30Minutes".into(),
            title: "In 30 Minutes".into(),
            timestamp: now + 30.0 * 60.0,
        },
        ReminderPresetOption {
            id: "in1Hour".into(),
            title: "In 1 Hour".into(),
            timestamp: now + 60.0 * 60.0,
        },
    ];
    let dt = Local
        .timestamp_opt(now.floor() as i64, 0)
        .single()
        .unwrap_or_else(Local::now);
    let today_date = dt.date_naive();
    if let Some(evening) = Local
        .from_local_datetime(&today_date.and_time(NaiveTime::from_hms_opt(18, 0, 0).unwrap()))
        .single()
    {
        let ev_ts = evening.timestamp() as f64;
        if ev_ts - now > 15.0 * 60.0 {
            out.push(ReminderPresetOption {
                id: "thisEvening".into(),
                title: "This Evening".into(),
                timestamp: ev_ts,
            });
        }
    }
    if let Some(tomorrow_date) = today_date.succ_opt() {
        if let Some(morning) = Local
            .from_local_datetime(
                &tomorrow_date.and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
            )
            .single()
        {
            out.push(ReminderPresetOption {
                id: "tomorrowMorning".into(),
                title: "Tomorrow Morning".into(),
                timestamp: morning.timestamp() as f64,
            });
        }
    }
    let _ = (today_start(), dt.hour(), dt.weekday());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::TaskDay;

    #[test]
    fn focus_timer_completes_linked_task_only_when_finished() {
        let db = Db::open_memory().unwrap();
        let mut tasks = TaskStore::new(&db);
        let t_done = tasks
            .add(&db, "Finishes", tasks.date_for_day(TaskDay::Today), None)
            .unwrap();
        let t_abandoned = tasks
            .add(&db, "Abandoned", tasks.date_for_day(TaskDay::Today), None)
            .unwrap();

        let mut timer = FocusTimer::new(25);
        timer.start_demo(&db, 60, Some(t_abandoned.id.clone()));
        timer.reset(&db);
        assert!(!tasks.task(&t_abandoned.id).unwrap().is_done);
        assert_eq!(repo::session_unsent_count(&db), 0); // <30s abandoned ignored

        timer.start_demo(&db, 1, Some(t_done.id.clone()));
        // Force finish via end_instant in past
        timer.end_instant = Some(Instant::now() - std::time::Duration::from_secs(2));
        let (_, outcome) = timer.tick(&db, &mut tasks, true, true);
        let outcome = outcome.unwrap();
        assert_eq!(outcome.completed_task_id.as_deref(), Some(t_done.id.as_str()));
        assert!(tasks.task(&t_done.id).unwrap().is_done);
        assert_eq!(repo::session_unsent_count(&db), 1);
    }

    #[test]
    fn stale_reminders_are_marked_reminded_without_firing() {
        let db = Db::open_memory().unwrap();
        let mut tasks = TaskStore::new(&db);
        let t1 = tasks
            .add(&db, "Fresh", tasks.date_for_day(TaskDay::Today), None)
            .unwrap();
        let t2 = tasks
            .add(&db, "Stale", tasks.date_for_day(TaskDay::Today), None)
            .unwrap();
        let now = now_secs();
        tasks.set_reminder(&db, &t1.id, Some(now - 10.0));
        tasks.set_reminder(&db, &t2.id, Some(now - REMINDER_STALE_AFTER_SECS - 60.0));

        let fired = poll_due_reminders(&db, &mut tasks, true, now);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].id, t1.id);
        assert!(tasks.task(&t1.id).unwrap().reminded);
        assert!(tasks.task(&t2.id).unwrap().reminded);
    }
}
