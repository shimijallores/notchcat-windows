use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Db {
    pub conn: Connection,
    pub path: String,
}

/// Each entry runs once, in order. Never edit a shipped migration; append a new one.
/// These are byte-for-byte identical to the Mac app's migrations in Database.swift.
pub const MIGRATIONS: [&str; 3] = [
    // 1: initial schema
    "CREATE TABLE tasks (
        id                 TEXT PRIMARY KEY,
        title              TEXT NOT NULL,
        is_done            INTEGER NOT NULL DEFAULT 0,
        day                REAL NOT NULL,
        planned_day        REAL NOT NULL,
        position           INTEGER NOT NULL DEFAULT 0,
        time_limit_minutes INTEGER,
        remind_at          REAL,
        reminded           INTEGER NOT NULL DEFAULT 0,
        created_at         REAL NOT NULL,
        completed_at       REAL
    );
    CREATE INDEX idx_tasks_day ON tasks(day);
    CREATE INDEX idx_tasks_planned ON tasks(planned_day);
    CREATE INDEX idx_tasks_completed ON tasks(completed_at);
    CREATE INDEX idx_tasks_due ON tasks(remind_at) WHERE reminded = 0 AND remind_at IS NOT NULL;

    CREATE TABLE focus_sessions (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        task_id         TEXT,
        started_at      REAL NOT NULL,
        active_seconds  INTEGER NOT NULL,
        planned_seconds INTEGER NOT NULL,
        completed       INTEGER NOT NULL
    );
    CREATE INDEX idx_sessions_started ON focus_sessions(started_at);

    CREATE TABLE notes (
        day        REAL PRIMARY KEY,
        body       TEXT NOT NULL,
        updated_at REAL NOT NULL
    );

    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );",
    // 2: cloud sync bookkeeping
    "CREATE TABLE task_sync (
        task_id  TEXT PRIMARY KEY,
        field_ts TEXT NOT NULL DEFAULT '{}',
        dirty    TEXT NOT NULL DEFAULT '[]',
        deleted  INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE sync_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );",
    // 3: focus sessions get a stable id (to sync) and a flag for \"the server has this one\"
    "ALTER TABLE focus_sessions ADD COLUMN sync_id TEXT;
    ALTER TABLE focus_sessions ADD COLUMN pushed INTEGER NOT NULL DEFAULT 0;
    UPDATE focus_sessions SET sync_id = lower(
        hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)), 2) || '-' ||
        substr('89ab', 1 + (abs(random()) % 4), 1) || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6)));
    CREATE UNIQUE INDEX idx_sessions_sync_id ON focus_sessions(sync_id);",
];

/// Runs pending migrations up to `upto` (exclusive count). Each one is atomic with its version bump.
pub fn migrate(conn: &mut Connection, upto: usize) -> rusqlite::Result<()> {
    let current = conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? as usize;
    for (i, sql) in MIGRATIONS.iter().enumerate().take(upto) {
        if i < current {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
        tx.commit()?;
    }
    Ok(())
}

impl Db {
    /// `NOTCHCAT_DB` if set, otherwise `%APPDATA%\NotchCat\notchcat.sqlite`.
    pub fn default_path() -> PathBuf {
        if let Ok(p) = std::env::var("NOTCHCAT_DB") {
            let trimmed = p.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("NotchCat")
            .join("notchcat.sqlite")
    }

    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(dir);
            }
        }
        Self::finish(Connection::open(path)?, path.to_string_lossy().into_owned())
    }

    /// Opens the database at `path`, falling back to `:memory:` if disk storage fails.
    pub fn open_or_memory(path: &Path) -> Db {
        match Self::open(path) {
            Ok(db) => db,
            Err(err) => {
                eprintln!("NotchCat: could not open database ({err}); using in-memory storage");
                Self::open_memory().expect("open in-memory sqlite")
            }
        }
    }

    pub fn open_memory() -> rusqlite::Result<Db> {
        Self::finish(Connection::open_in_memory()?, ":memory:".into())
    }

    fn finish(mut conn: Connection, path: String) -> rusqlite::Result<Db> {
        conn.busy_timeout(Duration::from_millis(3000))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
        )?;
        migrate(&mut conn, MIGRATIONS.len())?;
        Ok(Db { conn, path })
    }

    /// Writes a consistent copy of the whole database to `dest` (used for backups).
    pub fn backup_to(&self, dest: &Path) -> rusqlite::Result<()> {
        let _ = std::fs::remove_file(dest);
        self.conn
            .execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_exists(db: &Db, name: &str) -> bool {
        db.conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            == 1
    }

    #[test]
    fn fresh_database_is_fully_migrated() {
        let db = Db::open_memory().unwrap();
        let v: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 3);
        for t in [
            "tasks",
            "focus_sessions",
            "notes",
            "settings",
            "task_sync",
            "sync_meta",
        ] {
            assert!(table_exists(&db, t), "missing table {t}");
        }
    }

    #[test]
    fn migrating_twice_is_a_noop() {
        let mut db = Db::open_memory().unwrap();
        migrate(&mut db.conn, MIGRATIONS.len()).unwrap();
        let v: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 3);
    }

    #[test]
    fn migration_three_backfills_session_uuids() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&mut conn, 2).unwrap();
        conn.execute(
            "INSERT INTO focus_sessions (task_id, started_at, active_seconds, planned_seconds, completed) VALUES (NULL, 1.0, 60, 60, 1)",
            [],
        )
        .unwrap();
        migrate(&mut conn, 3).unwrap();
        let id: String = conn
            .query_row("SELECT sync_id FROM focus_sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().nth(14), Some('4'));
    }

    #[test]
    fn backup_writes_a_readable_copy() {
        let db = Db::open_memory().unwrap();
        db.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES ('k', '\"v\"')",
                [],
            )
            .unwrap();
        let dest =
            std::env::temp_dir().join(format!("nc-backup-{}.sqlite", uuid::Uuid::new_v4()));
        db.backup_to(&dest).unwrap();
        let copy = Db::open(&dest).unwrap();
        let v: String = copy
            .conn
            .query_row("SELECT value FROM settings WHERE key='k'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "\"v\"");
        drop(copy);
        let _ = std::fs::remove_file(&dest);
    }
}
