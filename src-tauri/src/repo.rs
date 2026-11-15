use crate::db::Db;
use crate::models::*;
use rusqlite::{params, OptionalExtension};
use std::collections::{HashMap, HashSet};

// ---------- Tasks ----------

pub fn task_load_all(db: &Db) -> Vec<TaskItem> {
    let sql = "SELECT id, title, is_done, day, planned_day, time_limit_minutes, remind_at, reminded, created_at, completed_at
               FROM tasks ORDER BY position, created_at";
    let mut stmt = match db.conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |r| {
        Ok(TaskItem {
            id: r.get(0)?,
            title: r.get(1)?,
            is_done: r.get::<_, i64>(2)? != 0,
            day: r.get(3)?,
            planned_day: r.get(4)?,
            time_limit_minutes: r.get(5)?,
            remind_at: r.get(6)?,
            reminded: r.get::<_, i64>(7)? != 0,
            created_at: r.get(8)?,
            completed_at: r.get(9)?,
        })
    })
    .map(|rows| rows.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

pub fn task_positions(db: &Db) -> HashMap<String, i64> {
    let mut stmt = match db.conn.prepare("SELECT id, position FROM tasks") {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default()
}

pub fn task_upsert(db: &Db, t: &TaskItem, position: i64) {
    let sql = "INSERT INTO tasks (id, title, is_done, day, planned_day, position, time_limit_minutes, remind_at, reminded, created_at, completed_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
               ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title, is_done = excluded.is_done, day = excluded.day,
                 planned_day = excluded.planned_day, position = excluded.position,
                 time_limit_minutes = excluded.time_limit_minutes, remind_at = excluded.remind_at,
                 reminded = excluded.reminded, completed_at = excluded.completed_at";
    let _ = db.conn.execute(
        sql,
        params![
            t.id,
            t.title,
            t.is_done as i64,
            t.day,
            t.planned_day,
            position,
            t.time_limit_minutes,
            t.remind_at,
            t.reminded as i64,
            t.created_at,
            t.completed_at
        ],
    );
}

pub fn task_delete(db: &Db, id: &str) {
    let _ = db.conn.execute("DELETE FROM tasks WHERE id = ?1", [id]);
}

/// Writes the display order for all tasks in one transaction.
pub fn task_save_positions(db: &Db, ids: &[String]) {
    let _ = db.conn.execute_batch("BEGIN IMMEDIATE");
    for (i, id) in ids.iter().enumerate() {
        let _ = db
            .conn
            .execute("UPDATE tasks SET position = ?1 WHERE id = ?2", params![i as i64, id]);
    }
    let _ = db.conn.execute_batch("COMMIT");
}

pub fn task_count(db: &Db) -> i64 {
    db.conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap_or(0)
}

/// Escapes a user search string for use inside a LIKE pattern (with ESCAPE '\').
fn like_pattern(search: &str) -> String {
    let escaped = search
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// Finished tasks, newest first, with the focus time that was spent on each.
pub fn task_finished(db: &Db, search: &str, limit: i64, offset: i64) -> Vec<FinishedTask> {
    let sql = "SELECT t.id, t.title, t.completed_at, t.planned_day, t.time_limit_minutes,
                      COALESCE((SELECT SUM(s.active_seconds) FROM focus_sessions s WHERE s.task_id = t.id), 0)
               FROM tasks t
               WHERE t.is_done = 1 AND t.completed_at IS NOT NULL AND t.title LIKE ?1 ESCAPE '\\'
               ORDER BY t.completed_at DESC
               LIMIT ?2 OFFSET ?3";
    let mut stmt = match db.conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(params![like_pattern(search), limit, offset], |r| {
        Ok(FinishedTask {
            id: r.get(0)?,
            title: r.get(1)?,
            completed_at: r.get(2)?,
            planned_day: r.get(3)?,
            time_limit_minutes: r.get(4)?,
            focus_seconds: r.get(5)?,
        })
    })
    .map(|rows| rows.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

pub fn task_finished_count(db: &Db, search: &str) -> i64 {
    db.conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE is_done = 1 AND completed_at IS NOT NULL AND title LIKE ?1 ESCAPE '\\'",
            [like_pattern(search)],
            |r| r.get(0),
        )
        .unwrap_or(0)
}

// ---------- Focus sessions and Insights ----------

pub fn session_insert(db: &Db, s: &FocusSession) {
    let _ = db.conn.execute(
        "INSERT INTO focus_sessions (task_id, started_at, active_seconds, planned_seconds, completed, sync_id, pushed)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
        params![
            s.task_id,
            s.started_at,
            s.active_seconds,
            s.planned_seconds,
            s.completed as i64,
            uuid::Uuid::new_v4().to_string()
        ],
    );
}

pub fn session_unsent(db: &Db, limit: i64) -> Vec<(String, FocusSession)> {
    let mut stmt = match db.conn.prepare(
        "SELECT sync_id, task_id, started_at, active_seconds, planned_seconds, completed
         FROM focus_sessions WHERE pushed = 0 AND sync_id IS NOT NULL ORDER BY id LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([limit], |r| {
        Ok((
            r.get::<_, String>(0)?,
            FocusSession {
                task_id: r.get(1)?,
                started_at: r.get(2)?,
                active_seconds: r.get(3)?,
                planned_seconds: r.get(4)?,
                completed: r.get::<_, i64>(5)? != 0,
            },
        ))
    })
    .map(|rows| rows.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

pub fn session_unsent_count(db: &Db) -> i64 {
    db.conn
        .query_row(
            "SELECT COUNT(*) FROM focus_sessions WHERE pushed = 0 AND sync_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
}

pub fn session_mark_pushed(db: &Db, sync_ids: &[String]) {
    let _ = db.conn.execute_batch("BEGIN IMMEDIATE");
    for id in sync_ids {
        let _ = db.conn.execute(
            "UPDATE focus_sessions SET pushed = 1 WHERE sync_id = ?1",
            [id],
        );
    }
    let _ = db.conn.execute_batch("COMMIT");
}

/// Stores a session another device logged. Returns false if it was already here.
pub fn session_insert_remote(db: &Db, sync_id: &str, s: &FocusSession) -> bool {
    db.conn
        .execute(
            "INSERT OR IGNORE INTO focus_sessions (task_id, started_at, active_seconds, planned_seconds, completed, sync_id, pushed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
            params![
                s.task_id,
                s.started_at,
                s.active_seconds,
                s.planned_seconds,
                s.completed as i64,
                sync_id
            ],
        )
        .map(|n| n > 0)
        .unwrap_or(false)
}

pub fn session_reset_pushed(db: &Db) {
    let _ = db.conn.execute("UPDATE focus_sessions SET pushed = 0", []);
}

/// Seconds of focus that started in [from, to).
pub fn session_focus_seconds(db: &Db, from: f64, to: f64) -> i64 {
    db.conn
        .query_row(
            "SELECT COALESCE(SUM(active_seconds), 0) FROM focus_sessions WHERE started_at >= ?1 AND started_at < ?2",
            [from, to],
            |r| r.get(0),
        )
        .unwrap_or(0)
}

/// Tasks first planned for a day in [from, to).
pub fn session_planned_count(db: &Db, from: f64, to: f64) -> i64 {
    db.conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE planned_day >= ?1 AND planned_day < ?2",
            [from, to],
            |r| r.get(0),
        )
        .unwrap_or(0)
}

/// Tasks completed in [from, to).
pub fn session_completed_count(db: &Db, from: f64, to: f64) -> i64 {
    db.conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE is_done = 1 AND completed_at >= ?1 AND completed_at < ?2",
            [from, to],
            |r| r.get(0),
        )
        .unwrap_or(0)
}

// ---------- Notes ----------

/// The one long-lived note is stored under day = 0 (1970-01-01), same as the Mac app.
const NOTE_MAIN_KEY: f64 = 0.0;

pub fn note_load_main(db: &Db) -> Option<String> {
    db.conn
        .query_row(
            "SELECT body FROM notes WHERE day = ?1",
            [NOTE_MAIN_KEY],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten()
}

pub fn note_save_main(db: &Db, body: &str, now: f64) {
    let _ = db.conn.execute(
        "INSERT INTO notes (day, body, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(day) DO UPDATE SET body = excluded.body, updated_at = excluded.updated_at",
        params![NOTE_MAIN_KEY, body, now],
    );
}

/// Notes from when the notepad was one-per-day, oldest first, blank ones dropped.
pub fn note_legacy_daily(db: &Db) -> Vec<(f64, String)> {
    let mut stmt = match db
        .conn
        .prepare("SELECT day, body FROM notes WHERE day != ?1 ORDER BY day ASC")
    {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([NOTE_MAIN_KEY], |r| {
        Ok((r.get::<_, f64>(0)?, r.get::<_, String>(1)?))
    })
    .map(|rows| {
        rows.filter_map(Result::ok)
            .filter(|(_, b)| !b.trim().is_empty())
            .collect()
    })
    .unwrap_or_default()
}

// ---------- Settings ----------

pub fn settings_load_raw(db: &Db, key: &str) -> Option<String> {
    db.conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()
        .ok()
        .flatten()
}

pub fn settings_load<T: serde::de::DeserializeOwned>(db: &Db, key: &str) -> Option<T> {
    let json = settings_load_raw(db, key)?;
    serde_json::from_str(&json).ok()
}

pub fn settings_save<T: serde::Serialize>(db: &Db, key: &str, value: &T) {
    if let Ok(json) = serde_json::to_string(value) {
        let _ = db.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, json],
        );
    }
}

// ---------- Sync bookkeeping ----------

pub fn task_sync_load_all(db: &Db) -> HashMap<String, TaskSyncMeta> {
    let mut stmt = match db
        .conn
        .prepare("SELECT task_id, field_ts, dirty, deleted FROM task_sync")
    {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)? != 0,
            ))
        })
        .map(|it| it.filter_map(Result::ok).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut out = HashMap::new();
    for (id, ts, dirty, deleted) in rows {
        let Some(norm_id) = normalize_uuid_upper(&id) else {
            continue;
        };
        let field_ts: HashMap<String, i64> = serde_json::from_str(&ts).unwrap_or_default();
        let dirty_vec: Vec<String> = serde_json::from_str(&dirty).unwrap_or_default();
        out.insert(
            norm_id,
            TaskSyncMeta {
                field_ts,
                dirty: dirty_vec.into_iter().collect::<HashSet<_>>(),
                deleted,
            },
        );
    }
    out
}

pub fn task_sync_save(db: &Db, id: &str, meta: &TaskSyncMeta) {
    let ts = serde_json::to_string(&meta.field_ts).unwrap_or_else(|_| "{}".to_string());
    let mut dirty_sorted: Vec<&String> = meta.dirty.iter().collect();
    dirty_sorted.sort();
    let dirty = serde_json::to_string(&dirty_sorted).unwrap_or_else(|_| "[]".to_string());
    let _ = db.conn.execute(
        "INSERT INTO task_sync (task_id, field_ts, dirty, deleted) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(task_id) DO UPDATE SET field_ts = excluded.field_ts, dirty = excluded.dirty, deleted = excluded.deleted",
        params![id, ts, dirty, meta.deleted as i64],
    );
}

pub fn task_sync_delete_all(db: &Db) {
    let _ = db.conn.execute("DELETE FROM task_sync", []);
}

pub fn sync_meta_get(db: &Db, key: &str) -> Option<String> {
    db.conn
        .query_row("SELECT value FROM sync_meta WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()
        .ok()
        .flatten()
}

pub fn sync_meta_set(db: &Db, key: &str, value: Option<&str>) {
    if let Some(v) = value {
        let _ = db.conn.execute(
            "INSERT INTO sync_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, v],
        );
    } else {
        let _ = db.conn.execute("DELETE FROM sync_meta WHERE key = ?1", [key]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn task(title: &str) -> TaskItem {
        TaskItem {
            id: uuid::Uuid::new_v4().to_string().to_uppercase(),
            title: title.into(),
            is_done: false,
            day: 100.0,
            planned_day: 100.0,
            time_limit_minutes: None,
            remind_at: None,
            reminded: false,
            created_at: 1.0,
            completed_at: None,
        }
    }

    fn session(task_id: Option<&str>, started: f64, secs: i64) -> FocusSession {
        FocusSession {
            task_id: task_id.map(String::from),
            started_at: started,
            active_seconds: secs,
            planned_seconds: 1500,
            completed: true,
        }
    }

    #[test]
    fn upsert_then_load_round_trips() {
        let db = Db::open_memory().unwrap();
        let mut t = task("write plan");
        t.time_limit_minutes = Some(25);
        t.remind_at = Some(500.0);
        task_upsert(&db, &t, 0);
        assert_eq!(task_load_all(&db), vec![t]);
    }

    #[test]
    fn upsert_updates_but_keeps_created_at() {
        let db = Db::open_memory().unwrap();
        let mut t = task("a");
        task_upsert(&db, &t, 0);
        t.title = "b".into();
        t.created_at = 999.0;
        t.is_done = true;
        t.completed_at = Some(7.0);
        task_upsert(&db, &t, 0);
        let all = task_load_all(&db);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "b");
        assert!(all[0].is_done);
        assert_eq!(all[0].created_at, 1.0);
    }

    #[test]
    fn load_orders_by_position_then_created_at() {
        let db = Db::open_memory().unwrap();
        let (a, b, c) = (task("a"), task("b"), task("c"));
        task_upsert(&db, &a, 2);
        task_upsert(&db, &b, 0);
        task_upsert(&db, &c, 1);
        let titles: Vec<_> = task_load_all(&db).into_iter().map(|t| t.title).collect();
        assert_eq!(titles, ["b", "c", "a"]);
        task_save_positions(&db, &[a.id.clone(), b.id.clone(), c.id.clone()]);
        let titles: Vec<_> = task_load_all(&db).into_iter().map(|t| t.title).collect();
        assert_eq!(titles, ["a", "b", "c"]);
        assert_eq!(task_positions(&db)[&c.id], 2);
    }

    #[test]
    fn delete_and_count() {
        let db = Db::open_memory().unwrap();
        let t = task("x");
        task_upsert(&db, &t, 0);
        assert_eq!(task_count(&db), 1);
        task_delete(&db, &t.id);
        assert_eq!(task_count(&db), 0);
    }

    #[test]
    fn finished_search_escapes_like_wildcards_and_sums_focus() {
        let db = Db::open_memory().unwrap();
        let mut a = task("100% done");
        a.is_done = true;
        a.completed_at = Some(20.0);
        let mut b = task("1000 things");
        b.is_done = true;
        b.completed_at = Some(10.0);
        task_upsert(&db, &a, 0);
        task_upsert(&db, &b, 1);
        db.conn
            .execute(
                "INSERT INTO focus_sessions (task_id, started_at, active_seconds, planned_seconds, completed, sync_id) VALUES (?1, 1.0, 90, 120, 1, 's1')",
                [&a.id],
            )
            .unwrap();
        let hits = task_finished(&db, "100%", 10, 0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "100% done");
        assert_eq!(hits[0].focus_seconds, 90);
        assert_eq!(task_finished_count(&db, "100%"), 1);
        let all = task_finished(&db, "", 10, 0);
        assert_eq!(
            all.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
            ["100% done", "1000 things"]
        );
        assert_eq!(task_finished(&db, "", 1, 1)[0].title, "1000 things");
    }

    #[test]
    fn sessions_track_unsent_and_pushed() {
        let db = Db::open_memory().unwrap();
        session_insert(&db, &session(None, 10.0, 60));
        session_insert(&db, &session(None, 20.0, 120));
        assert_eq!(session_unsent_count(&db), 2);
        let unsent = session_unsent(&db, 1);
        assert_eq!(unsent.len(), 1);
        assert_eq!(unsent[0].1.active_seconds, 60);
        session_mark_pushed(&db, &[unsent[0].0.clone()]);
        assert_eq!(session_unsent_count(&db), 1);
        session_reset_pushed(&db);
        assert_eq!(session_unsent_count(&db), 2);
    }

    #[test]
    fn remote_session_is_inserted_once_and_marked_pushed() {
        let db = Db::open_memory().unwrap();
        let s = session(None, 5.0, 30);
        assert!(session_insert_remote(&db, "abc", &s));
        assert!(!session_insert_remote(&db, "abc", &s));
        assert_eq!(session_unsent_count(&db), 0);
    }

    #[test]
    fn insights_ranges_are_half_open() {
        let db = Db::open_memory().unwrap();
        session_insert(&db, &session(None, 10.0, 60));
        session_insert(&db, &session(None, 20.0, 40));
        assert_eq!(session_focus_seconds(&db, 10.0, 20.0), 60);
        assert_eq!(session_focus_seconds(&db, 10.0, 21.0), 100);

        let mut t = task("done");
        t.planned_day = 50.0;
        t.is_done = true;
        t.completed_at = Some(60.0);
        task_upsert(&db, &t, 0);
        assert_eq!(session_planned_count(&db, 50.0, 51.0), 1);
        assert_eq!(session_planned_count(&db, 51.0, 99.0), 0);
        assert_eq!(session_completed_count(&db, 60.0, 61.0), 1);
        assert_eq!(session_completed_count(&db, 61.0, 99.0), 0);
    }

    #[test]
    fn main_note_saves_and_legacy_notes_are_separate() {
        let db = Db::open_memory().unwrap();
        assert_eq!(note_load_main(&db), None);
        note_save_main(&db, "hello", 1.0);
        note_save_main(&db, "hello again", 2.0);
        assert_eq!(note_load_main(&db).as_deref(), Some("hello again"));
        db.conn
            .execute(
                "INSERT INTO notes (day, body, updated_at) VALUES (86400.0, 'old', 1.0)",
                [],
            )
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO notes (day, body, updated_at) VALUES (172800.0, '  ', 1.0)",
                [],
            )
            .unwrap();
        assert_eq!(note_legacy_daily(&db), vec![(86400.0, "old".to_string())]);
    }

    #[test]
    fn settings_and_sync_meta_round_trip() {
        let db = Db::open_memory().unwrap();
        assert_eq!(settings_load::<Vec<i32>>(&db, "nums"), None);
        settings_save(&db, "nums", &vec![1, 2, 3]);
        settings_save(&db, "nums", &vec![4]);
        assert_eq!(settings_load::<Vec<i32>>(&db, "nums"), Some(vec![4]));

        let id = uuid::Uuid::new_v4().to_string().to_uppercase();
        let mut meta = TaskSyncMeta::default();
        meta.field_ts.insert("title".into(), 1234);
        meta.dirty.insert("title".into());
        task_sync_save(&db, &id, &meta);
        assert_eq!(task_sync_load_all(&db)[&id], meta);
        task_sync_delete_all(&db);
        assert!(task_sync_load_all(&db).is_empty());

        sync_meta_set(&db, "cursor", Some("42"));
        assert_eq!(sync_meta_get(&db, "cursor").as_deref(), Some("42"));
        sync_meta_set(&db, "cursor", None);
        assert_eq!(sync_meta_get(&db, "cursor"), None);
    }
}
