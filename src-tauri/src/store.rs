use crate::db::Db;
use crate::models::*;
use crate::repo;
use chrono::{DateTime, Duration as ChronoDuration, Local, LocalResult, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn now_ms() -> i64 {
    (now_secs() * 1000.0).round() as i64
}

pub fn secs_to_ms(secs: f64) -> i64 {
    (secs * 1000.0).round() as i64
}

pub fn local_day_start(ts: f64) -> f64 {
    let secs = ts.floor() as i64;
    let dt = Local
        .timestamp_opt(secs, 0)
        .single()
        .unwrap_or_else(Local::now);
    let date = dt.date_naive();
    local_midnight(date)
}

pub fn local_midnight(date: chrono::NaiveDate) -> f64 {
    let naive = date.and_time(NaiveTime::MIN);
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt.timestamp() as f64,
        LocalResult::Ambiguous(dt, _) => dt.timestamp() as f64,
        LocalResult::None => (naive.and_utc().timestamp()) as f64,
    }
}

pub fn today_start() -> f64 {
    local_midnight(Local::now().date_naive())
}

pub fn add_local_days(day_ts: f64, days: i64) -> f64 {
    let dt = Local
        .timestamp_opt(day_ts.floor() as i64, 0)
        .single()
        .unwrap_or_else(Local::now);
    let target = dt.date_naive() + ChronoDuration::days(days);
    local_midnight(target)
}

pub fn is_same_local_day(a: f64, b: f64) -> bool {
    let da = Local
        .timestamp_opt(a.floor() as i64, 0)
        .single()
        .map(|d| d.date_naive());
    let db = Local
        .timestamp_opt(b.floor() as i64, 0)
        .single()
        .map(|d| d.date_naive());
    da.is_some() && da == db
}

// ---------- TaskStore ----------

pub const SYNC_FIELDS: [&str; 11] = [
    "title",
    "is_done",
    "day",
    "planned_day",
    "position",
    "time_limit_minutes",
    "remind_at",
    "reminded",
    "created_at",
    "completed_at",
    "deleted",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskDay {
    Today,
    Tomorrow,
}

pub struct TaskStore {
    pub tasks: Vec<TaskItem>,
    pub sync_meta: HashMap<String, TaskSyncMeta>,
    pub positions: HashMap<String, i64>,
}

impl TaskStore {
    pub fn new(db: &Db) -> Self {
        let mut store = Self {
            tasks: repo::task_load_all(db),
            sync_meta: repo::task_sync_load_all(db),
            positions: repo::task_positions(db),
        };
        store.roll_over_if_needed(db);
        store
    }

    pub fn date_for_day(&self, day: TaskDay) -> f64 {
        let today = today_start();
        match day {
            TaskDay::Today => today,
            TaskDay::Tomorrow => add_local_days(today, 1),
        }
    }

    pub fn tasks_on(&self, date: f64) -> Vec<TaskItem> {
        self.tasks
            .iter()
            .filter(|t| is_same_local_day(t.day, date))
            .cloned()
            .collect()
    }

    pub fn visible_tasks(&self, day: TaskDay) -> Vec<TaskItem> {
        self.tasks_on(self.date_for_day(day))
    }

    pub fn task(&self, id: &str) -> Option<&TaskItem> {
        self.tasks.iter().find(|t| t.id.eq_ignore_ascii_case(id))
    }

    pub fn due_reminders(&self, now: f64) -> Vec<TaskItem> {
        self.tasks
            .iter()
            .filter(|t| {
                !t.is_done
                    && !t.reminded
                    && t.remind_at.map(|r| r <= now).unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    pub fn reminders_on(&self, date: f64) -> Vec<TaskItem> {
        let mut out: Vec<TaskItem> = self
            .tasks
            .iter()
            .filter(|t| {
                if t.is_done {
                    return false;
                }
                t.remind_at
                    .map(|r| is_same_local_day(r, date))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        out.sort_by(|a, b| {
            a.remind_at
                .unwrap_or(0.0)
                .partial_cmp(&b.remind_at.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    // ---------- Mutations ----------

    pub fn add(
        &mut self,
        db: &Db,
        title: &str,
        date: f64,
        time_limit_minutes: Option<i64>,
    ) -> Option<TaskItem> {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return None;
        }
        let day = local_day_start(date);
        let task = TaskItem {
            id: uuid::Uuid::new_v4().to_string().to_uppercase(),
            title: trimmed.to_string(),
            is_done: false,
            day,
            planned_day: day,
            time_limit_minutes,
            remind_at: None,
            reminded: false,
            created_at: now_secs(),
            completed_at: None,
        };
        let next_pos = self.positions.values().copied().max().unwrap_or(-1) + 1;
        self.positions.insert(task.id.clone(), next_pos);
        self.tasks.push(task.clone());
        self.persist(db, self.tasks.len() - 1);
        self.create_sync_meta(db, &task);
        Some(task)
    }

    pub fn toggle_done(&mut self, db: &Db, id: &str) -> bool {
        self.update(db, id, |t| {
            t.is_done = !t.is_done;
            t.completed_at = if t.is_done { Some(now_secs()) } else { None };
        })
    }

    /// Marks a task done. Unlike `toggle_done`, doing it twice leaves it done without re-stamping.
    pub fn mark_done(&mut self, db: &Db, id: &str) -> bool {
        let already_done = self.task(id).map(|t| t.is_done).unwrap_or(true);
        if already_done {
            return false;
        }
        self.update(db, id, |t| {
            t.is_done = true;
            t.completed_at = Some(now_secs());
        })
    }

    pub fn restore(&mut self, db: &Db, id: &str) -> bool {
        let today = today_start();
        self.update(db, id, |t| {
            t.is_done = false;
            t.completed_at = None;
            t.day = today;
        })
    }

    pub fn rename(&mut self, db: &Db, id: &str, title: &str) -> bool {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return false;
        }
        let new_title = trimmed.to_string();
        self.update(db, id, |t| {
            t.title = new_title;
        })
    }

    pub fn set_time_limit(&mut self, db: &Db, id: &str, minutes: Option<i64>) -> bool {
        self.update(db, id, |t| {
            t.time_limit_minutes = minutes;
        })
    }

    pub fn move_to_day(&mut self, db: &Db, id: &str, day: TaskDay) -> bool {
        let d = self.date_for_day(day);
        self.move_to_date(db, id, d)
    }

    pub fn move_to_date(&mut self, db: &Db, id: &str, date: f64) -> bool {
        let d = local_day_start(date);
        self.update(db, id, |t| {
            t.day = d;
            if !t.is_done {
                t.planned_day = d;
            }
        })
    }

    pub fn duplicate(&mut self, db: &Db, id: &str) -> Option<TaskItem> {
        let idx = self
            .tasks
            .iter()
            .position(|t| t.id.eq_ignore_ascii_case(id))?;
        let mut copy = self.tasks[idx].clone();
        copy.id = uuid::Uuid::new_v4().to_string().to_uppercase();
        copy.is_done = false;
        copy.completed_at = None;
        copy.remind_at = None;
        copy.reminded = false;
        copy.created_at = now_secs();
        copy.planned_day = copy.day;
        self.tasks.insert(idx + 1, copy.clone());
        self.persist(db, idx + 1);
        self.renumber_positions(db);
        self.create_sync_meta(db, &copy);
        Some(copy)
    }

    pub fn delete(&mut self, db: &Db, id: &str) -> bool {
        let Some(norm_id) = normalize_uuid_upper(id)
            .or_else(|| self.task(id).map(|t| t.id.clone()))
        else {
            return false;
        };
        let prev_len = self.tasks.len();
        self.tasks.retain(|t| !t.id.eq_ignore_ascii_case(&norm_id));
        repo::task_delete(db, &norm_id);
        self.positions.remove(&norm_id);

        let mut meta = self.sync_meta.get(&norm_id).cloned().unwrap_or_default();
        meta.deleted = true;
        let ts = Self::next_timestamp(&meta);
        meta.field_ts.insert("deleted".into(), ts);
        meta.dirty = HashSet::from(["deleted".into()]);
        self.save_meta(db, &norm_id, meta);
        self.tasks.len() != prev_len
    }

    /// Reorders within a day's visible tasks (by ordered task IDs) while leaving other days in place.
    pub fn reorder_visible(&mut self, db: &Db, day_ts: f64, ordered_ids: &[String]) -> bool {
        let slots: Vec<usize> = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| is_same_local_day(t.day, day_ts))
            .map(|(i, _)| i)
            .collect();
        if slots.len() <= 1 {
            return false;
        }
        let mut by_id: HashMap<String, TaskItem> = slots
            .iter()
            .map(|&i| (self.tasks[i].id.to_uppercase(), self.tasks[i].clone()))
            .collect();
        let mut reordered = Vec::with_capacity(slots.len());
        for id in ordered_ids {
            if let Some(item) = by_id.remove(&id.to_uppercase()) {
                reordered.push(item);
            }
        }
        // Append any tasks that weren't in ordered_ids in their previous relative order.
        for &i in &slots {
            let key = self.tasks[i].id.to_uppercase();
            if let Some(item) = by_id.remove(&key) {
                reordered.push(item);
            }
        }
        let mut changed = false;
        for (&slot, item) in slots.iter().zip(reordered.into_iter()) {
            if self.tasks[slot].id != item.id {
                changed = true;
            }
            self.tasks[slot] = item;
        }
        if changed {
            self.renumber_positions(db);
        }
        changed
    }

    /// Reorders using SwiftUI-style `move(fromOffsets: IndexSet, toOffset: Int)`.
    pub fn move_visible_index(&mut self, db: &Db, day_ts: f64, from: usize, to: usize) -> bool {
        let mut visible: Vec<String> = self
            .tasks_on(day_ts)
            .into_iter()
            .map(|t| t.id)
            .collect();
        if from >= visible.len() || to > visible.len() || from == to {
            return false;
        }
        let item = visible.remove(from);
        let dest = if to > from { to - 1 } else { to };
        visible.insert(dest.min(visible.len()), item);
        self.reorder_visible(db, day_ts, &visible)
    }

    // ---------- Reminders ----------

    pub fn set_reminder(&mut self, db: &Db, id: &str, remind_at: Option<f64>) -> bool {
        self.update(db, id, |t| {
            t.remind_at = remind_at;
            t.reminded = false;
        })
    }

    pub fn mark_reminded(&mut self, db: &Db, id: &str) -> bool {
        self.update(db, id, |t| {
            t.reminded = true;
        })
    }

    pub fn snooze(&mut self, db: &Db, id: &str, minutes: i64) -> bool {
        self.set_reminder(db, id, Some(now_secs() + (minutes as f64) * 60.0))
    }

    // ---------- Day rollover ----------

    pub fn roll_over_if_needed(&mut self, db: &Db) -> bool {
        let today = today_start();
        let mut any = false;
        let len = self.tasks.len();
        for i in 0..len {
            if !self.tasks[i].is_done && self.tasks[i].day < today {
                let before = self.tasks[i].clone();
                self.tasks[i].day = today;
                self.persist(db, i);
                let after = self.tasks[i].clone();
                self.touch(db, &before, &["day"], &after);
                any = true;
            }
        }
        any
    }

    // ---------- Internals ----------

    fn update<F: FnOnce(&mut TaskItem)>(&mut self, db: &Db, id: &str, change: F) -> bool {
        let Some(idx) = self.tasks.iter().position(|t| t.id.eq_ignore_ascii_case(id)) else {
            return false;
        };
        let before = self.tasks[idx].clone();
        change(&mut self.tasks[idx]);
        self.persist(db, idx);
        let changed = Self::changed_fields(&before, &self.tasks[idx]);
        if !changed.is_empty() {
            let after = self.tasks[idx].clone();
            let refs: Vec<&str> = changed.iter().copied().collect();
            self.touch(db, &before, &refs, &after);
            true
        } else {
            false
        }
    }

    fn persist(&self, db: &Db, index: usize) {
        if let Some(task) = self.tasks.get(index) {
            let pos = self.positions.get(&task.id).copied().unwrap_or(index as i64);
            repo::task_upsert(db, task, pos);
        }
    }

    fn renumber_positions(&mut self, db: &Db) {
        let ids: Vec<String> = self.tasks.iter().map(|t| t.id.clone()).collect();
        repo::task_save_positions(db, &ids);
        let tasks_snapshot = self.tasks.clone();
        for (index, task) in tasks_snapshot.iter().enumerate() {
            let idx_i64 = index as i64;
            if self.positions.get(&task.id).copied() != Some(idx_i64) {
                self.positions.insert(task.id.clone(), idx_i64);
                self.touch(db, task, &["position"], task);
            }
        }
    }

    // ---------- Sync bookkeeping ----------

    fn next_timestamp(meta: &TaskSyncMeta) -> i64 {
        let max_ts = meta.field_ts.values().copied().max().unwrap_or(0);
        now_ms().max(max_ts + 1)
    }

    fn save_meta(&mut self, db: &Db, id: &str, meta: TaskSyncMeta) {
        repo::task_sync_save(db, id, &meta);
        self.sync_meta.insert(id.to_string(), meta);
    }

    fn baseline_meta(task: &TaskItem) -> TaskSyncMeta {
        let known = task
            .created_at
            .max(task.completed_at.unwrap_or(f64::NEG_INFINITY));
        let ts = secs_to_ms(known);
        let mut meta = TaskSyncMeta::default();
        for field in SYNC_FIELDS {
            meta.field_ts.insert(field.to_string(), ts);
            meta.dirty.insert(field.to_string());
        }
        meta
    }

    fn create_sync_meta(&mut self, db: &Db, task: &TaskItem) {
        let mut meta = TaskSyncMeta::default();
        let now = now_ms();
        for field in SYNC_FIELDS {
            meta.field_ts.insert(field.to_string(), now);
            meta.dirty.insert(field.to_string());
        }
        self.save_meta(db, &task.id, meta);
    }

    fn touch(&mut self, db: &Db, before: &TaskItem, fields: &[&str], _after: &TaskItem) {
        let mut meta = self
            .sync_meta
            .get(&before.id)
            .cloned()
            .unwrap_or_else(|| Self::baseline_meta(before));
        let ts = Self::next_timestamp(&meta);
        for &field in fields {
            meta.field_ts.insert(field.to_string(), ts);
            meta.dirty.insert(field.to_string());
        }
        meta.deleted = false;
        self.save_meta(db, &before.id, meta);
    }

    fn changed_fields(a: &TaskItem, b: &TaskItem) -> Vec<&'static str> {
        let mut out = Vec::new();
        if a.title != b.title {
            out.push("title");
        }
        if a.is_done != b.is_done {
            out.push("is_done");
        }
        if a.day != b.day {
            out.push("day");
        }
        if a.planned_day != b.planned_day {
            out.push("planned_day");
        }
        if a.time_limit_minutes != b.time_limit_minutes {
            out.push("time_limit_minutes");
        }
        if a.remind_at != b.remind_at {
            out.push("remind_at");
        }
        if a.reminded != b.reminded {
            out.push("reminded");
        }
        if a.created_at != b.created_at {
            out.push("created_at");
        }
        if a.completed_at != b.completed_at {
            out.push("completed_at");
        }
        out
    }

    fn wire_value(&self, field: &str, task: &TaskItem) -> JSONValue {
        match field {
            "title" => JSONValue::String(task.title.clone()),
            "is_done" => JSONValue::Bool(task.is_done),
            "day" => JSONValue::Double(task.day),
            "planned_day" => JSONValue::Double(task.planned_day),
            "position" => JSONValue::Int(self.positions.get(&task.id).copied().unwrap_or(0)),
            "time_limit_minutes" => JSONValue::from_opt_i64(task.time_limit_minutes),
            "remind_at" => JSONValue::from_opt_f64(task.remind_at),
            "reminded" => JSONValue::Bool(task.reminded),
            "created_at" => JSONValue::Double(task.created_at),
            "completed_at" => JSONValue::from_opt_f64(task.completed_at),
            _ => JSONValue::Null,
        }
    }

    pub fn pending_count(&self) -> usize {
        self.sync_meta
            .values()
            .filter(|m| !m.dirty.is_empty())
            .count()
    }

    pub fn pending_changes(&self, limit: usize) -> Vec<WireChange> {
        let mut out = Vec::new();
        let fallback_ts = now_ms();
        for (id, meta) in &self.sync_meta {
            if meta.dirty.is_empty() {
                continue;
            }
            if out.len() >= limit {
                break;
            }
            let mut fields = BTreeMap::new();
            let mut stamps = BTreeMap::new();
            if meta.deleted {
                fields.insert("deleted".into(), JSONValue::Bool(true));
                stamps.insert(
                    "deleted".into(),
                    meta.field_ts.get("deleted").copied().unwrap_or(fallback_ts),
                );
            } else {
                let Some(task) = self.tasks.iter().find(|t| t.id == *id) else {
                    continue;
                };
                for field in &meta.dirty {
                    let val = if field == "deleted" {
                        JSONValue::Bool(false)
                    } else {
                        self.wire_value(field, task)
                    };
                    fields.insert(field.clone(), val);
                    stamps.insert(
                        field.clone(),
                        meta.field_ts.get(field).copied().unwrap_or(fallback_ts),
                    );
                }
            }
            out.push(WireChange {
                collection: "tasks".into(),
                id: id.to_lowercase(),
                fields,
                field_ts: stamps,
                base_seq: None,
            });
        }
        out
    }

    pub fn drop_pending(&mut self, db: &Db, ids: &[String]) {
        for raw in ids {
            let Some(id) = normalize_uuid_upper(raw) else {
                continue;
            };
            if let Some(meta) = self.sync_meta.get_mut(&id) {
                meta.dirty.clear();
                let copy = meta.clone();
                repo::task_sync_save(db, &id, &copy);
            }
        }
    }

    pub fn enroll_all_for_sync(&mut self, db: &Db) {
        let snapshot = self.tasks.clone();
        for task in &snapshot {
            if !self.sync_meta.contains_key(&task.id) {
                let meta = Self::baseline_meta(task);
                repo::task_sync_save(db, &task.id, &meta);
                self.sync_meta.insert(task.id.clone(), meta);
            }
        }
    }

    pub fn reset_sync_state(&mut self, db: &Db) {
        self.sync_meta.clear();
        repo::task_sync_delete_all(db);
    }

    pub fn apply_remote(&mut self, db: &Db, records: &[WireRecord]) {
        let mut working = self.tasks.clone();
        let mut touched = false;

        for record in records {
            if record.collection != "tasks" {
                continue;
            }
            let Some(id) = normalize_uuid_upper(&record.id) else {
                continue;
            };
            let index = working.iter().position(|t| t.id == id);
            let mut meta = self
                .sync_meta
                .get(&id)
                .cloned()
                .unwrap_or_else(|| {
                    index
                        .map(|i| Self::baseline_meta(&working[i]))
                        .unwrap_or_default()
                });
            let mut task = index.map(|i| working[i].clone());
            let mut position = self.positions.get(&id).copied();
            let mut deleted = meta.deleted;

            for (field, value) in &record.fields {
                let remote_ts = record.field_ts.get(field).copied().unwrap_or(0);
                let local_ts = meta.field_ts.get(field).copied().unwrap_or(0);
                if remote_ts < local_ts {
                    continue;
                }
                meta.field_ts.insert(field.clone(), remote_ts);
                meta.dirty.remove(field);

                if field == "deleted" {
                    deleted = value.as_bool().unwrap_or(deleted);
                    continue;
                }
                if field == "position" {
                    position = value.as_i64().or(position);
                    continue;
                }
                if task.is_none() {
                    let Some(title) = record.fields.get("title").and_then(|v| v.as_str()) else {
                        break;
                    };
                    let Some(day) = record.fields.get("day").and_then(|v| v.as_f64()) else {
                        break;
                    };
                    let Some(planned) = record.fields.get("planned_day").and_then(|v| v.as_f64())
                    else {
                        break;
                    };
                    let Some(created) = record.fields.get("created_at").and_then(|v| v.as_f64())
                    else {
                        break;
                    };
                    task = Some(TaskItem {
                        id: id.clone(),
                        title: title.to_string(),
                        is_done: false,
                        day,
                        planned_day: planned,
                        time_limit_minutes: None,
                        remind_at: None,
                        reminded: false,
                        created_at: created,
                        completed_at: None,
                    });
                }
                let Some(t) = task.as_mut() else {
                    continue;
                };
                match field.as_str() {
                    "title" => {
                        if let Some(s) = value.as_str() {
                            if !s.is_empty() {
                                t.title = s.to_string();
                            }
                        }
                    }
                    "is_done" => {
                        if let Some(b) = value.as_bool() {
                            t.is_done = b;
                        }
                    }
                    "day" => {
                        if let Some(d) = value.as_f64() {
                            t.day = d;
                        }
                    }
                    "planned_day" => {
                        if let Some(d) = value.as_f64() {
                            t.planned_day = d;
                        }
                    }
                    "time_limit_minutes" => t.time_limit_minutes = value.as_i64(),
                    "remind_at" => t.remind_at = value.as_f64(),
                    "reminded" => {
                        if let Some(b) = value.as_bool() {
                            t.reminded = b;
                        }
                    }
                    "created_at" => {
                        if let Some(c) = value.as_f64() {
                            t.created_at = c;
                        }
                    }
                    "completed_at" => t.completed_at = value.as_f64(),
                    _ => {}
                }
            }

            meta.deleted = deleted;
            if deleted {
                if let Some(i) = index {
                    working.remove(i);
                    repo::task_delete(db, &id);
                    touched = true;
                }
                self.positions.remove(&id);
                meta.dirty.remove("deleted");
            } else if let Some(t) = task {
                let resolved_pos = position
                    .or_else(|| self.positions.get(&id).copied())
                    .unwrap_or_else(|| self.positions.values().copied().max().unwrap_or(-1) + 1);
                self.positions.insert(id.clone(), resolved_pos);
                repo::task_upsert(db, &t, resolved_pos);
                if let Some(i) = index {
                    working[i] = t;
                } else {
                    working.push(t);
                }
                touched = true;
            }
            repo::task_sync_save(db, &id, &meta);
            self.sync_meta.insert(id, meta);
        }

        if touched {
            let positions = &self.positions;
            working.sort_by(|a, b| {
                let pa = positions.get(&a.id).copied().unwrap_or(i64::MAX);
                let pb = positions.get(&b.id).copied().unwrap_or(i64::MAX);
                pa.cmp(&pb).then_with(|| {
                    a.created_at
                        .partial_cmp(&b.created_at)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
            self.tasks = working;
        }
    }
}

// ---------- NoteStore ----------

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSyncState {
    pub base_seq: Option<i64>,
    pub dirty: bool,
    pub ts: i64,
    pub last_sent: Option<String>,
}

pub struct NoteStore {
    pub text: String,
    pub sync_state: NoteSyncState,
}

impl NoteStore {
    pub fn new(db: &Db) -> Self {
        let sync_state = repo::sync_meta_get(db, "note_sync")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let text = if let Some(saved) = repo::note_load_main(db) {
            saved
        } else {
            let merged = Self::merge_legacy(&repo::note_legacy_daily(db));
            repo::note_save_main(db, &merged, now_secs());
            merged
        };
        Self { text, sync_state }
    }

    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    pub fn flush(&self, db: &Db) {
        repo::note_save_main(db, &self.text, now_secs());
    }

    pub fn set_text(&mut self, db: &Db, new_text: String) -> bool {
        if self.text == new_text {
            return false;
        }
        self.text = new_text;
        self.flush(db);
        self.mark_dirty(db);
        true
    }

    pub fn merge_legacy(notes: &[(f64, String)]) -> String {
        if notes.len() <= 1 {
            return notes.first().map(|(_, b)| b.clone()).unwrap_or_default();
        }
        notes
            .iter()
            .map(|(day, body)| {
                let dt: DateTime<Local> = Local
                    .timestamp_opt(day.floor() as i64, 0)
                    .single()
                    .unwrap_or_else(Local::now);
                let label = dt.format("%b %-d, %Y").to_string();
                format!("— {label} —\n{body}")
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn reconcile(remote: &str, local: &str) -> String {
        if remote == local || local.is_empty() {
            return remote.to_string();
        }
        if remote.is_empty() {
            return local.to_string();
        }
        if remote.contains(local) {
            return remote.to_string();
        }
        if local.contains(remote) {
            return local.to_string();
        }
        format!("{remote}\n\n— also written elsewhere at the same time —\n\n{local}")
    }

    /// Converts the line containing `cursor_utf16` into a task title and removes that line.
    /// Mirrors `NoteTextView.convertCurrentLine()` in NotepadCard.swift.
    pub fn convert_line(text: &str, cursor_utf16: usize) -> Option<(String, usize, String)> {
        if text.is_empty() {
            return None;
        }
        // Convert UTF-16 offset (from textarea selectionStart) to byte offset.
        let mut utf16_pos = 0usize;
        let mut cursor_byte = text.len();
        for (byte_idx, ch) in text.char_indices() {
            if utf16_pos >= cursor_utf16 {
                cursor_byte = byte_idx;
                break;
            }
            utf16_pos += ch.len_utf16();
        }
        cursor_byte = cursor_byte.min(text.len());

        // Find start of line (right after preceding '\n', or 0)
        let line_start = text[..cursor_byte]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        // Find end of line (including trailing '\n' if present)
        let (content_end, line_end) = match text[line_start..].find('\n') {
            Some(rel) => (line_start + rel, line_start + rel + 1),
            None => (text.len(), text.len()),
        };

        let raw_content = text[line_start..content_end].trim_end_matches('\r');
        let trimmed = raw_content.trim();
        if trimmed.is_empty() || trimmed.starts_with('✓') {
            return None;
        }

        let has_trailing_nl = line_end > content_end;
        let mut remove_start = line_start;
        if !has_trailing_nl && remove_start > 0 {
            // Last line has no trailing newline: consume preceding '\n' (or '\r\n')
            remove_start -= 1;
            if remove_start > 0 && text.as_bytes()[remove_start - 1] == b'\r' {
                remove_start -= 1;
            }
        }

        let mut next_text = String::with_capacity(text.len() - (line_end - remove_start));
        next_text.push_str(&text[..remove_start]);
        next_text.push_str(&text[line_end..]);
        let new_cursor_utf16: usize = next_text[..remove_start]
            .chars()
            .map(|c| c.len_utf16())
            .sum();
        Some((next_text, new_cursor_utf16, trimmed.to_string()))
    }

    fn save_sync_state(&self, db: &Db) {
        let json = serde_json::to_string(&self.sync_state).ok();
        repo::sync_meta_set(db, "note_sync", json.as_deref());
    }

    fn mark_dirty(&mut self, db: &Db) {
        self.sync_state.dirty = true;
        self.sync_state.ts = now_ms().max(self.sync_state.ts + 1);
        self.save_sync_state(db);
    }

    fn set_from_server(&mut self, db: &Db, body: String) {
        self.text = body;
        self.flush(db);
    }

    pub fn pending_count(&self) -> usize {
        if self.sync_state.dirty {
            1
        } else {
            0
        }
    }

    pub fn pending_changes(&mut self, db: &Db, limit: usize) -> Vec<WireChange> {
        if !self.sync_state.dirty || limit == 0 {
            return vec![];
        }
        self.sync_state.last_sent = Some(self.text.clone());
        self.save_sync_state(db);
        let mut fields = BTreeMap::new();
        fields.insert("body".into(), JSONValue::String(self.text.clone()));
        let mut field_ts = BTreeMap::new();
        field_ts.insert("body".into(), self.sync_state.ts);
        vec![WireChange {
            collection: "notes".into(),
            id: "main".into(),
            fields,
            field_ts,
            base_seq: self.sync_state.base_seq,
        }]
    }

    pub fn apply(&mut self, db: &Db, records: &[WireRecord]) {
        for record in records {
            if record.collection != "notes" || record.id != "main" {
                continue;
            }
            let body = record
                .fields
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(base) = self.sync_state.base_seq {
                if record.seq <= base {
                    continue;
                }
            }

            if self.sync_state.last_sent.as_deref() == Some(body.as_str()) {
                self.sync_state.base_seq = Some(record.seq);
                if self.text == body {
                    self.sync_state.dirty = false;
                }
                self.save_sync_state(db);
            } else if !self.sync_state.dirty {
                self.set_from_server(db, body);
                self.sync_state.base_seq = Some(record.seq);
                self.save_sync_state(db);
            } else {
                let combined = Self::reconcile(&body, &self.text);
                self.sync_state.base_seq = Some(record.seq);
                if combined == body {
                    self.set_from_server(db, body);
                    self.sync_state.dirty = false;
                    self.save_sync_state(db);
                } else {
                    if combined != self.text {
                        self.set_from_server(db, combined);
                    }
                    self.sync_state.dirty = true;
                    self.sync_state.ts = now_ms().max(self.sync_state.ts + 1);
                    self.save_sync_state(db);
                }
            }
        }
    }

    pub fn resolve_conflicts(&mut self, db: &Db, records: &[WireRecord]) {
        self.sync_state.last_sent = None;
        self.apply(db, records);
    }

    pub fn drop_pending(&mut self, db: &Db, _ids: &[String]) {
        self.sync_state.dirty = false;
        self.save_sync_state(db);
    }

    pub fn enroll(&mut self, db: &Db) {
        if self.sync_state.base_seq.is_none() && !self.text.is_empty() && !self.sync_state.dirty {
            self.mark_dirty(db);
        }
    }

    pub fn reset_sync_state(&mut self, db: &Db) {
        self.sync_state = NoteSyncState::default();
        self.save_sync_state(db);
    }
}

// ---------- AppSettings & SettingsSync ----------

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSyncState {
    pub field_ts: HashMap<String, i64>,
    pub dirty: HashSet<String>,
}

struct SyncedSettingsField {
    name: &'static str,
    read: fn(&SettingsData) -> JSONValue,
    write: fn(&mut SettingsData, &JSONValue),
}

fn read_cards(d: &SettingsData) -> JSONValue {
    JSONValue::String(serde_json::to_string(&d.cards).unwrap_or_else(|_| "[]".to_string()))
}

fn write_cards(d: &mut SettingsData, v: &JSONValue) {
    if let Some(text) = v.as_str() {
        if let Ok(cards) = serde_json::from_str::<Vec<CardSetting>>(text) {
            d.cards = cards;
            d.normalize_cards();
        }
    }
}

macro_rules! bool_field {
    ($name:expr, $field:ident) => {
        SyncedSettingsField {
            name: $name,
            read: |d| JSONValue::Bool(d.$field),
            write: |d, v| {
                if let Some(b) = v.as_bool() {
                    d.$field = b;
                }
            },
        }
    };
}

macro_rules! int_field {
    ($name:expr, $field:ident) => {
        SyncedSettingsField {
            name: $name,
            read: |d| JSONValue::Int(d.$field),
            write: |d, v| {
                if let Some(n) = v.as_i64() {
                    d.$field = n;
                }
            },
        }
    };
}

const SETTINGS_SYNC_FIELDS: [SyncedSettingsField; 9] = [
    SyncedSettingsField {
        name: "cards",
        read: read_cards,
        write: write_cards,
    },
    bool_field!("show_week_tab", show_week_tab),
    bool_field!("show_insights_tab", show_insights_tab),
    int_field!("default_focus_minutes", default_focus_minutes),
    bool_field!("timer_sound", timer_sound),
    bool_field!("reminders_enabled", reminders_enabled),
    int_field!("snooze_minutes", snooze_minutes),
    bool_field!("show_reminders_in_events", show_reminders_in_events),
    bool_field!("paper_grain", paper_grain),
];

pub struct AppSettings {
    pub data: SettingsData,
    pub sync_state: SettingsSyncState,
    snapshot: SettingsData,
}

impl AppSettings {
    const KEY: &'static str = "app";
    const SYNC_KEY: &'static str = "settings_sync";

    pub fn new(db: &Db) -> Self {
        let data = repo::settings_load_raw(db, Self::KEY)
            .map(|raw| SettingsData::from_json_tolerant(&raw))
            .unwrap_or_default();
        let sync_state = repo::sync_meta_get(db, Self::SYNC_KEY)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let snapshot = data.clone();
        Self {
            data,
            sync_state,
            snapshot,
        }
    }

    pub fn music_wanted(&self) -> bool {
        self.is_enabled(CardID::Music) || self.data.show_music_chip
    }

    pub fn visible_cards(&self) -> Vec<CardID> {
        self.data
            .cards
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.card)
            .collect()
    }

    pub fn is_enabled(&self, card: CardID) -> bool {
        self.data
            .cards
            .iter()
            .find(|c| c.card == card)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    pub fn set_enabled(&mut self, db: &Db, card: CardID, on: bool) -> bool {
        let Some(i) = self.data.cards.iter().position(|c| c.card == card) else {
            return false;
        };
        if !on && self.data.cards.iter().filter(|c| c.enabled).count() == 1 && self.data.cards[i].enabled {
            return false;
        }
        let mut next = self.data.clone();
        next.cards[i].enabled = on;
        self.replace_data(db, next)
    }

    pub fn move_card(&mut self, db: &Db, card: CardID, offset: isize) -> bool {
        let Some(i) = self.data.cards.iter().position(|c| c.card == card) else {
            return false;
        };
        let j = i as isize + offset;
        if j < 0 || j as usize >= self.data.cards.len() {
            return false;
        }
        let mut next = self.data.clone();
        next.cards.swap(i, j as usize);
        self.replace_data(db, next)
    }

    /// Updates settings from UI/caller and returns true if any synced field changed.
    pub fn replace_data(&mut self, db: &Db, mut next: SettingsData) -> bool {
        next.normalize_cards();
        if next == self.data {
            return false;
        }
        self.data = next;
        repo::settings_save(db, Self::KEY, &self.data);

        let now = now_ms();
        let mut synced_changed = false;
        for field in &SETTINGS_SYNC_FIELDS {
            if (field.read)(&self.data) != (field.read)(&self.snapshot) {
                let prev = self.sync_state.field_ts.get(field.name).copied().unwrap_or(0);
                self.sync_state
                    .field_ts
                    .insert(field.name.to_string(), now.max(prev + 1));
                self.sync_state.dirty.insert(field.name.to_string());
                synced_changed = true;
            }
        }
        self.snapshot = self.data.clone();
        if synced_changed {
            self.save_sync_state(db);
        }
        synced_changed
    }

    fn save_sync_state(&self, db: &Db) {
        let json = serde_json::to_string(&self.sync_state).ok();
        repo::sync_meta_set(db, Self::SYNC_KEY, json.as_deref());
    }

    pub fn pending_count(&self) -> usize {
        if self.sync_state.dirty.is_empty() {
            0
        } else {
            1
        }
    }

    pub fn pending_changes(&self) -> Vec<WireChange> {
        if self.sync_state.dirty.is_empty() {
            return vec![];
        }
        let mut fields = BTreeMap::new();
        let mut field_ts = BTreeMap::new();
        for field in &SETTINGS_SYNC_FIELDS {
            if self.sync_state.dirty.contains(field.name) {
                fields.insert(field.name.to_string(), (field.read)(&self.data));
                field_ts.insert(
                    field.name.to_string(),
                    self.sync_state.field_ts.get(field.name).copied().unwrap_or(0),
                );
            }
        }
        vec![WireChange {
            collection: "settings".into(),
            id: "app".into(),
            fields,
            field_ts,
            base_seq: None,
        }]
    }

    pub fn apply(&mut self, db: &Db, records: &[WireRecord]) {
        for record in records {
            if record.collection != "settings" || record.id != "app" {
                continue;
            }
            let mut next = self.data.clone();
            for field in &SETTINGS_SYNC_FIELDS {
                let Some(value) = record.fields.get(field.name) else {
                    continue;
                };
                let server_ts = record.field_ts.get(field.name).copied().unwrap_or(0);
                let local_ts = self.sync_state.field_ts.get(field.name).copied().unwrap_or(0);
                if server_ts < local_ts {
                    continue;
                }
                (field.write)(&mut next, value);
                self.sync_state.field_ts.insert(field.name.to_string(), server_ts);
                self.sync_state.dirty.remove(field.name);
            }
            if next != self.data {
                self.data = next;
                repo::settings_save(db, Self::KEY, &self.data);
            }
            self.snapshot = self.data.clone();
            self.save_sync_state(db);
        }
    }

    pub fn drop_pending(&mut self, db: &Db, _ids: &[String]) {
        self.sync_state.dirty.clear();
        self.save_sync_state(db);
    }

    pub fn enroll(&mut self, db: &Db) -> bool {
        let defaults = SettingsData::default();
        let now = now_ms();
        let mut any = false;
        for field in &SETTINGS_SYNC_FIELDS {
            if !self.sync_state.field_ts.contains_key(field.name)
                && (field.read)(&self.data) != (field.read)(&defaults)
            {
                self.sync_state.field_ts.insert(field.name.to_string(), now);
                self.sync_state.dirty.insert(field.name.to_string());
                any = true;
            }
        }
        if any {
            self.save_sync_state(db);
        }
        any
    }

    pub fn reset_sync_state(&mut self, db: &Db) {
        self.sync_state = SettingsSyncState::default();
        self.save_sync_state(db);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_store_add_rollover_move_and_sync_merge() {
        let db = Db::open_memory().unwrap();
        let mut store = TaskStore::new(&db);
        let yesterday = add_local_days(today_start(), -1);
        let t = store.add(&db, "Old task", yesterday, Some(20)).unwrap();
        assert!(store.roll_over_if_needed(&db));
        assert_eq!(store.task(&t.id).unwrap().day, today_start());
        assert_eq!(store.task(&t.id).unwrap().planned_day, yesterday);

        // Moving to tomorrow updates planned_day too
        let tomorrow = add_local_days(today_start(), 1);
        assert!(store.move_to_date(&db, &t.id, tomorrow));
        assert_eq!(store.task(&t.id).unwrap().planned_day, tomorrow);

        // Reorder visible
        let b = store.add(&db, "Second", tomorrow, None).unwrap();
        assert!(store.move_visible_index(&db, tomorrow, 1, 0));
        let vis: Vec<_> = store.tasks_on(tomorrow).into_iter().map(|x| x.title).collect();
        assert_eq!(vis, ["Second", "Old task"]);

        // Delete creates tombstone
        assert!(store.delete(&db, &b.id));
        assert!(store.sync_meta[&b.id].deleted);
    }

    #[test]
    fn note_store_line_conversion_and_reconcile() {
        let r = NoteStore::convert_line("Buy milk\nCall dentist\n\n✓ already done\nLast line no newline", 2);
        let (text1, cur1, title1) = r.unwrap();
        assert_eq!(title1, "Buy milk");
        assert_eq!(text1, "Call dentist\n\n✓ already done\nLast line no newline");
        assert_eq!(cur1, 0);

        // Pre-marked line ignored
        let idx_done = text1.find("already done").unwrap();
        assert!(NoteStore::convert_line(&text1, idx_done).is_none());

        // Last line with no trailing newline removes preceding newline
        let idx_last = text1.find("Last line").unwrap();
        let (text2, _, title2) = NoteStore::convert_line(&text1, idx_last).unwrap();
        assert_eq!(title2, "Last line no newline");
        assert_eq!(text2, "Call dentist\n\n✓ already done");

        // Reconcile preserves both
        assert_eq!(NoteStore::reconcile("hello", "hello world"), "hello world");
        assert!(NoteStore::reconcile("alpha", "beta").contains("also written elsewhere"));
    }

    #[test]
    fn settings_zero_card_guard_and_tolerant_loading() {
        let db = Db::open_memory().unwrap();
        let mut s = AppSettings::new(&db);
        assert!(s.set_enabled(&db, CardID::Timer, false));
        assert!(s.set_enabled(&db, CardID::Notepad, false));
        assert!(s.set_enabled(&db, CardID::Events, false));
        // Only Tasks left enabled: disabling it must be refused
        assert!(!s.set_enabled(&db, CardID::Tasks, false));
        assert_eq!(s.visible_cards(), vec![CardID::Tasks]);

        // Partial JSON keeps specified fields and defaults missing ones
        db.conn
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('app', ?1)",
                ["{\"showWeekTab\":false,\"cards\":[{\"card\":\"notepad\",\"enabled\":true}]}"],
            )
            .unwrap();
        let partial = AppSettings::new(&db);
        assert!(!partial.data.show_week_tab);
        assert!(partial.data.show_insights_tab);
        assert_eq!(partial.data.cards[0].card, CardID::Notepad);
        assert_eq!(partial.data.cards.len(), 5);

        // Corrupt JSON falls back to defaults
        db.conn
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('app', 'not json')",
                [],
            )
            .unwrap();
        let corrupt = AppSettings::new(&db);
        assert_eq!(corrupt.data.default_focus_minutes, 25);
        assert_eq!(corrupt.visible_cards().len(), 4);
    }
}
