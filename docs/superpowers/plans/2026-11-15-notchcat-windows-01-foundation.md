# NotchCat Windows, Plan 1: Foundation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri app whose island opens and closes at the top center of the screen (hover, Alt+N, tray icon, Esc), backed by a Rust SQLite data layer that is schema-identical to the Mac app.

**Architecture:** Rust owns state. `db.rs` and `repo.rs` are the data layer (a port of `Database.swift` and `Repositories.swift`). `island.rs` is pure layout math. `window.rs` and `commands.rs` wire it to a transparent, always-on-top Tauri window that is click-through while closed. The React UI is a thin shell that listens for the open/closed event.

**Tech Stack:** Rust (stable), Tauri 2, `rusqlite` (bundled SQLite), `serde`, `uuid`, `dirs`, `tauri-plugin-global-shortcut`, React 18 + TypeScript + Vite.

**Spec:** `docs/superpowers/specs/2026-11-15-notchcat-windows-design.md`

## Where this plan sits

The spec covers several independent subsystems, so it is split into plans that each produce working software. This is plan 1. The others are written after this one lands, because the island spike (Task 7) may change the window approach.

| Plan | Phases | Delivers |
| --- | --- | --- |
| **1 Foundation (this)** | 0, 1, 2 | Island shell, tokens, data layer |
| 2 Core cards | 3, 4 | Tasks, Focus timer, Notepad, Reminders, Week, Insights, History |
| 3 Settings and shell polish | 5 | Appearance, General, card order, startup launch, backup, dev flags |
| 4 Cloud sync | 6 | API client, credentials, outbox, engine, sign in/up UI |
| 5 Integrations | 7 | Now Playing (SMTC), battery, sleep pause, ICS events, updater |
| 6 Release | 8, 9 | NSIS installer, CI, parity QA |

## Global Constraints

- Tauri 2, bundle identifier `cloud.notchcat.windows`, product name `NotchCat`.
- Database path: `%APPDATA%\NotchCat\notchcat.sqlite`. `NOTCHCAT_DB` overrides it.
- Schema and migrations are identical to the Mac app's three migrations. Never edit a shipped migration, only append.
- Timestamps are `REAL` epoch seconds (same as the Mac app). Sync timestamps elsewhere are epoch ms.
- Pragmas on open: `journal_mode = WAL`, `synchronous = NORMAL`, `foreign_keys = ON`, busy timeout 3000 ms.
- Hotkey: `Alt+N`.
- Island pill: 116 px wide. Panel width is clamped to 780 min and `min(0.74 * screen width, 1120)` max. Panel height is `clamp(0.36 * screen height, 320, 400)`.
- Colors come from `Theme.swift` exactly (see Task 1).
- Windows 10 1809 or later with WebView2 (preinstalled on Windows 11).

## File Structure

```
notchcat-windows/
  src-tauri/
    Cargo.toml
    tauri.conf.json
    capabilities/default.json
    src/
      main.rs         entry, calls lib::run
      lib.rs          builder: plugins, state, tray, shortcut, window setup
      db.rs           Db: open, pragmas, migrations, backup, default path
      models.rs       TaskItem, FinishedTask, FocusSession (serde camelCase)
      repo.rs         task, session, note, settings repositories + tests
      island.rs       pure layout math + tests
      state.rs        IslandState (open, pinned, panel size), transition rules + tests
      window.rs       apply open/closed to the Tauri window, hover poller
      commands.rs     tauri commands
  src/
    main.tsx
    App.tsx           pill + tray shell
    tokens.css        Theme.swift colors and sizes
    app.css
    assets/mascot.png
  docs/superpowers/{specs,plans}/
```

---

### Task 0: Prerequisites

**Files:** none (machine setup).

- [ ] **Step 1: Install the Rust toolchain.** Rust is not installed on this machine yet (`cargo` is not found).

Run: `winget install Rustlang.Rustup`, then open a new terminal.

- [ ] **Step 2: Install C++ build tools.** Needed by Rust's MSVC target.

Run: `winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`

- [ ] **Step 3: Verify.**

Run: `cargo --version; rustc --version; node --version`
Expected: three version lines (Node is already v22).

- [ ] **Step 4: Confirm WebView2.**

Run: `Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' | Select pv`
Expected: a version number. If missing, install the Evergreen runtime from Microsoft.

---

### Task 1: Scaffold, git, design tokens, mascot

**Files:**
- Create: whole project skeleton, `src/tokens.css`, `src/assets/mascot.png`, `.gitignore`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Produces: CSS variables `--housing`, `--housing-raised`, `--mint`, `--lavender`, `--sand`, `--slate`, `--rose`, `--ink`, `--amber`, `--card-radius`.

- [ ] **Step 1: Scaffold into a temp folder, then move it in.** `notchcat-windows` already contains `docs/`, so scaffold beside it.

```powershell
cd c:\Users\Patriach\Downloads\saas
npm create tauri-app@latest notchcat-windows-tmp -- --template react-ts --manager npm --identifier cloud.notchcat.windows --yes
robocopy notchcat-windows-tmp notchcat-windows /E /XD node_modules .git
Remove-Item -Recurse -Force notchcat-windows-tmp
cd notchcat-windows
git init
npm install
```

- [ ] **Step 2: Set the product name and window defaults.** In `src-tauri/tauri.conf.json` set `productName` to `NotchCat`, `identifier` to `cloud.notchcat.windows`, and replace the `app.windows` array with an empty array (the window is created in Rust in Task 6):

```json
"app": { "windows": [], "security": { "csp": null } }
```

- [ ] **Step 3: Copy the mascot.**

```powershell
New-Item -ItemType Directory -Force src\assets | Out-Null
Copy-Item ..\notchcat\Sources\NotchCat\Resources\mascot.png src\assets\mascot.png
```

- [ ] **Step 4: Write `src/tokens.css`** (values are `Theme.swift` colors multiplied by 255 and rounded):

```css
:root {
  --housing: rgb(9, 9, 10);
  --housing-raised: rgb(29, 29, 32);
  --mint: rgb(190, 218, 195);
  --lavender: rgb(204, 198, 237);
  --sand: rgb(217, 209, 156);
  --slate: rgb(163, 181, 204);
  --rose: rgb(237, 198, 204);
  --ink: rgb(27, 29, 28);
  --amber: rgb(252, 193, 80);
  --card-radius: 18px;
}
```

- [ ] **Step 5: Ignore build output.** `.gitignore`:

```
node_modules
dist
src-tauri/target
src-tauri/gen
*.sqlite
*.sqlite-wal
*.sqlite-shm
```

- [ ] **Step 6: Verify the scaffold builds.**

Run: `npm run tauri build -- --debug` (first build downloads crates and takes a few minutes)
Expected: build succeeds. It will start with no window, which is fine.

- [ ] **Step 7: Commit.**

```bash
git add -A
git commit -m "chore: scaffold Tauri 2 + React project with Theme tokens and mascot"
```

---

### Task 2: Database wrapper and migrations

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod db;`)

**Interfaces:**
- Produces:
  - `pub struct Db { pub conn: rusqlite::Connection, pub path: String }`
  - `Db::open(path: &std::path::Path) -> rusqlite::Result<Db>`
  - `Db::open_memory() -> rusqlite::Result<Db>`
  - `Db::default_path() -> std::path::PathBuf` (honors `NOTCHCAT_DB`)
  - `Db::backup_to(&self, dest: &std::path::Path) -> rusqlite::Result<()>`
  - `pub fn migrate(conn: &mut Connection, upto: usize) -> rusqlite::Result<()>`
  - `pub const MIGRATIONS: [&str; 3]`

- [ ] **Step 1: Add dependencies** in `src-tauri/Cargo.toml` under `[dependencies]`:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
dirs = "5"
```

- [ ] **Step 2: Write the failing tests.** Create `src-tauri/src/db.rs` containing only the tests module for now:

```rust
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
        let v: i64 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 3);
        for t in ["tasks", "focus_sessions", "notes", "settings", "task_sync", "sync_meta"] {
            assert!(table_exists(&db, t), "missing table {t}");
        }
    }

    #[test]
    fn migrating_twice_is_a_noop() {
        let mut db = Db::open_memory().unwrap();
        migrate(&mut db.conn, MIGRATIONS.len()).unwrap();
        let v: i64 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
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
        let id: String = conn.query_row("SELECT sync_id FROM focus_sessions", [], |r| r.get(0)).unwrap();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().nth(14), Some('4'));
    }

    #[test]
    fn backup_writes_a_readable_copy() {
        let db = Db::open_memory().unwrap();
        db.conn
            .execute("INSERT INTO settings (key, value) VALUES ('k', '\"v\"')", [])
            .unwrap();
        let dest = std::env::temp_dir().join(format!("nc-backup-{}.sqlite", uuid::Uuid::new_v4()));
        db.backup_to(&dest).unwrap();
        let copy = Db::open(&dest).unwrap();
        let v: String = copy.conn.query_row("SELECT value FROM settings WHERE key='k'", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "\"v\"");
        drop(copy);
        let _ = std::fs::remove_file(&dest);
    }
}
```

Add `mod db;` to `src-tauri/src/lib.rs`.

- [ ] **Step 3: Run the tests and confirm they fail to compile.**

Run: `cd src-tauri; cargo test db::`
Expected: FAIL, "cannot find type `Db`".

- [ ] **Step 4: Implement.** Put this above the tests module in `db.rs`:

```rust
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Db {
    pub conn: Connection,
    pub path: String,
}

/// Each entry runs once, in order. Never edit a shipped migration; append a new one.
/// These are byte-for-byte the Mac app's migrations.
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
            if !p.is_empty() {
                return PathBuf::from(p);
            }
        }
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("NotchCat")
            .join("notchcat.sqlite")
    }

    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::finish(Connection::open(path)?, path.to_string_lossy().into_owned())
    }

    pub fn open_memory() -> rusqlite::Result<Db> {
        Self::finish(Connection::open_in_memory()?, ":memory:".into())
    }

    fn finish(mut conn: Connection, path: String) -> rusqlite::Result<Db> {
        conn.busy_timeout(Duration::from_millis(3000))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;")?;
        migrate(&mut conn, MIGRATIONS.len())?;
        Ok(Db { conn, path })
    }

    /// Writes a consistent copy of the whole database to `dest` (used for backups).
    pub fn backup_to(&self, dest: &Path) -> rusqlite::Result<()> {
        let _ = std::fs::remove_file(dest);
        self.conn.execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])?;
        Ok(())
    }
}
```

- [ ] **Step 5: Run the tests.**

Run: `cargo test db::`
Expected: 4 passed.

- [ ] **Step 6: Commit.**

```bash
git add src-tauri
git commit -m "feat(data): SQLite wrapper with the Mac app's three migrations"
```

---

### Task 3: Models and task repository

**Files:**
- Create: `src-tauri/src/models.rs`, `src-tauri/src/repo.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod models; mod repo;`)

**Interfaces:**
- Consumes: `Db` from Task 2.
- Produces (`models.rs`, all `#[serde(rename_all = "camelCase")]`, derive `Clone, Debug, PartialEq, Serialize, Deserialize`):
  - `TaskItem { id: String, title: String, is_done: bool, day: f64, planned_day: f64, time_limit_minutes: Option<i64>, remind_at: Option<f64>, reminded: bool, created_at: f64, completed_at: Option<f64> }`
  - `FinishedTask { id: String, title: String, completed_at: f64, planned_day: f64, time_limit_minutes: Option<i64>, focus_seconds: i64 }`
  - `FocusSession { task_id: Option<String>, started_at: f64, active_seconds: i64, planned_seconds: i64, completed: bool }`
- Produces (`repo.rs`, functions taking `&Db`):
  - `task_load_all(db) -> Vec<TaskItem>` (ordered by position, created_at)
  - `task_positions(db) -> HashMap<String, i64>`
  - `task_upsert(db, &TaskItem, position: i64)`
  - `task_delete(db, id: &str)`
  - `task_save_positions(db, ids: &[String])`
  - `task_count(db) -> i64`
  - `task_finished(db, search: &str, limit: i64, offset: i64) -> Vec<FinishedTask>`
  - `task_finished_count(db, search: &str) -> i64`

- [ ] **Step 1: Write `models.rs`.**

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    pub is_done: bool,
    pub day: f64,
    pub planned_day: f64,
    pub time_limit_minutes: Option<i64>,
    pub remind_at: Option<f64>,
    pub reminded: bool,
    pub created_at: f64,
    pub completed_at: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishedTask {
    pub id: String,
    pub title: String,
    pub completed_at: f64,
    pub planned_day: f64,
    pub time_limit_minutes: Option<i64>,
    pub focus_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSession {
    pub task_id: Option<String>,
    pub started_at: f64,
    pub active_seconds: i64,
    pub planned_seconds: i64,
    pub completed: bool,
}
```

- [ ] **Step 2: Write the failing task tests.** Create `repo.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::models::*;

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
        assert_eq!(all.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["100% done", "1000 things"]);
        assert_eq!(task_finished(&db, "", 1, 1)[0].title, "1000 things");
    }
}
```

Add `mod models; mod repo;` to `lib.rs`.

- [ ] **Step 3: Run and confirm failure.**

Run: `cargo test repo::`
Expected: FAIL, "cannot find function `task_upsert`".

- [ ] **Step 4: Implement the task repository** above the tests module in `repo.rs`:

```rust
use crate::db::Db;
use crate::models::*;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;

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
            t.id, t.title, t.is_done as i64, t.day, t.planned_day, position,
            t.time_limit_minutes, t.remind_at, t.reminded as i64, t.created_at, t.completed_at
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
        let _ = db.conn.execute("UPDATE tasks SET position = ?1 WHERE id = ?2", params![i as i64, id]);
    }
    let _ = db.conn.execute_batch("COMMIT");
}

pub fn task_count(db: &Db) -> i64 {
    db.conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap_or(0)
}

/// Escapes a user search string for use inside a LIKE pattern (with ESCAPE '\').
fn like_pattern(search: &str) -> String {
    let escaped = search.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
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
```

Note: `OptionalExtension` is imported for Task 4; if the compiler warns it is unused, that clears after Task 4.

- [ ] **Step 5: Run the tests.**

Run: `cargo test repo::`
Expected: 5 passed.

- [ ] **Step 6: Commit.**

```bash
git add src-tauri
git commit -m "feat(data): models and task repository (port of TaskRepository)"
```

---

### Task 4: Session, note and settings repositories

**Files:**
- Modify: `src-tauri/src/repo.rs`

**Interfaces:**
- Consumes: `Db`, `FocusSession`, `task_upsert` and the `task()` test helper from Task 3.
- Produces:
  - `session_insert(db, &FocusSession)`
  - `session_unsent(db, limit: i64) -> Vec<(String, FocusSession)>` (sync id, session)
  - `session_unsent_count(db) -> i64`
  - `session_mark_pushed(db, &[String])`
  - `session_insert_remote(db, sync_id: &str, &FocusSession) -> bool` (false if already present)
  - `session_reset_pushed(db)`
  - `session_focus_seconds(db, from: f64, to: f64) -> i64`
  - `session_planned_count(db, from: f64, to: f64) -> i64`
  - `session_completed_count(db, from: f64, to: f64) -> i64`
  - `note_load_main(db) -> Option<String>`, `note_save_main(db, body: &str, now: f64)`, `note_legacy_daily(db) -> Vec<(f64, String)>`
  - `settings_load<T: DeserializeOwned>(db, key: &str) -> Option<T>`, `settings_save<T: Serialize>(db, key: &str, value: &T)`

- [ ] **Step 1: Add the failing tests** inside the existing `tests` module in `repo.rs`:

```rust
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
        db.conn.execute("INSERT INTO notes (day, body, updated_at) VALUES (86400.0, 'old', 1.0)", []).unwrap();
        db.conn.execute("INSERT INTO notes (day, body, updated_at) VALUES (172800.0, '  ', 1.0)", []).unwrap();
        assert_eq!(note_legacy_daily(&db), vec![(86400.0, "old".to_string())]);
    }

    #[test]
    fn settings_round_trip_json() {
        let db = Db::open_memory().unwrap();
        assert_eq!(settings_load::<Vec<i32>>(&db, "nums"), None);
        settings_save(&db, "nums", &vec![1, 2, 3]);
        settings_save(&db, "nums", &vec![4]);
        assert_eq!(settings_load::<Vec<i32>>(&db, "nums"), Some(vec![4]));
    }
```

- [ ] **Step 2: Run and confirm failure.**

Run: `cargo test repo::`
Expected: FAIL, missing `session_insert` and the rest.

- [ ] **Step 3: Implement.** Append to `repo.rs` above the tests module:

```rust
// ---------- Focus sessions and Insights ----------

pub fn session_insert(db: &Db, s: &FocusSession) {
    let _ = db.conn.execute(
        "INSERT INTO focus_sessions (task_id, started_at, active_seconds, planned_seconds, completed, sync_id, pushed)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
        params![
            s.task_id, s.started_at, s.active_seconds, s.planned_seconds, s.completed as i64,
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
        .query_row("SELECT COUNT(*) FROM focus_sessions WHERE pushed = 0 AND sync_id IS NOT NULL", [], |r| r.get(0))
        .unwrap_or(0)
}

pub fn session_mark_pushed(db: &Db, sync_ids: &[String]) {
    let _ = db.conn.execute_batch("BEGIN IMMEDIATE");
    for id in sync_ids {
        let _ = db.conn.execute("UPDATE focus_sessions SET pushed = 1 WHERE sync_id = ?1", [id]);
    }
    let _ = db.conn.execute_batch("COMMIT");
}

/// Stores a session another device logged. Returns false if it was already here.
pub fn session_insert_remote(db: &Db, sync_id: &str, s: &FocusSession) -> bool {
    db.conn
        .execute(
            "INSERT OR IGNORE INTO focus_sessions (task_id, started_at, active_seconds, planned_seconds, completed, sync_id, pushed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
            params![s.task_id, s.started_at, s.active_seconds, s.planned_seconds, s.completed as i64, sync_id],
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
        .query_row("SELECT COUNT(*) FROM tasks WHERE planned_day >= ?1 AND planned_day < ?2", [from, to], |r| r.get(0))
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
        .query_row("SELECT body FROM notes WHERE day = ?1", [NOTE_MAIN_KEY], |r| r.get(0))
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
    let mut stmt = match db.conn.prepare("SELECT day, body FROM notes WHERE day != ?1 ORDER BY day ASC") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([NOTE_MAIN_KEY], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, String>(1)?)))
        .map(|rows| rows.filter_map(Result::ok).filter(|(_, b)| !b.trim().is_empty()).collect())
        .unwrap_or_default()
}

// ---------- Settings ----------

pub fn settings_load<T: serde::de::DeserializeOwned>(db: &Db, key: &str) -> Option<T> {
    let json: String = db
        .conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .ok()
        .flatten()?;
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
```

- [ ] **Step 4: Run the tests.**

Run: `cargo test`
Expected: all pass (4 db, 10 repo).

- [ ] **Step 5: Mac compatibility check (manual, needs the Mac).** On the Mac, run `NOTCHCAT_DB=/tmp/mac.sqlite build/NotchCat.app/Contents/MacOS/NotchCat --seed-demo`, copy `/tmp/mac.sqlite` to `fixtures\mac.sqlite` in this repo, then add this test to `repo.rs`:

```rust
    #[test]
    #[ignore = "needs fixtures/mac.sqlite copied from a Mac; run with --ignored"]
    fn mac_database_opens_and_reads() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/mac.sqlite");
        let db = Db::open(&path).unwrap();
        assert!(task_count(&db) > 0);
        assert!(!task_load_all(&db).is_empty());
    }
```

Run: `cargo test mac_database -- --ignored`
Expected: PASS. If you cannot get the file yet, skip this step; it does not block later tasks.

- [ ] **Step 6: Commit.**

```bash
git add src-tauri fixtures
git commit -m "feat(data): session, note and settings repositories"
```

---

### Task 5: Island layout math

**Files:**
- Create: `src-tauri/src/island.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod island;`)

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct Rect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }` with `fn contains(&self, px: f64, py: f64) -> bool`
  - `pub const PILL_W: f64 = 116.0; pub const PILL_H: f64 = 22.0; pub const TOP_GAP: f64 = 6.0;`
  - `pub struct IslandLayout { pub window: Rect, pub pill: Rect, pub hover: Rect }`
  - `pub fn layout(monitor: Rect) -> IslandLayout` (monitor in logical pixels, top-left origin)
  - `pub fn panel_rect(monitor: Rect, panel_w: f64, panel_h: f64) -> Rect` (the visible open panel, flush with the top edge, centered)

All coordinates are logical pixels with a top-left origin (Windows convention; the Mac code uses bottom-left, so y is flipped).

- [ ] **Step 1: Write the failing tests** in `island.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn mon(w: f64, h: f64) -> Rect {
        Rect { x: 0.0, y: 0.0, w, h }
    }

    #[test]
    fn window_on_1080p_is_1120_wide_and_388_tall_centered() {
        let l = layout(mon(1920.0, 1080.0));
        assert_eq!(l.window, Rect { x: 400.0, y: 0.0, w: 1120.0, h: 388.0 });
    }

    #[test]
    fn narrow_screens_use_74_percent_rounded_to_even() {
        let l = layout(mon(1280.0, 720.0));
        assert_eq!(l.window.w, 948.0); // 0.74 * 1280 = 947.2 -> 948
        assert_eq!(l.window.h, 320.0); // 0.36 * 720 = 259 -> clamped up to 320
    }

    #[test]
    fn tall_screens_clamp_height_to_400() {
        assert_eq!(layout(mon(3840.0, 2160.0)).window.h, 400.0);
    }

    #[test]
    fn pill_is_centered_with_top_gap() {
        let l = layout(mon(1920.0, 1080.0));
        assert_eq!(l.pill, Rect { x: 902.0, y: TOP_GAP, w: PILL_W, h: PILL_H });
    }

    #[test]
    fn hover_reaches_from_screen_top_to_40_below_the_pill() {
        let l = layout(mon(1920.0, 1080.0));
        assert_eq!(l.hover, Rect { x: 896.0, y: 0.0, w: PILL_W + 12.0, h: TOP_GAP + PILL_H + 40.0 });
        assert!(l.hover.contains(960.0, 0.0));
        assert!(l.hover.contains(960.0, 67.0));
        assert!(!l.hover.contains(960.0, 69.0));
        assert!(!l.hover.contains(890.0, 10.0));
    }

    #[test]
    fn monitor_offset_is_respected() {
        let m = Rect { x: 1920.0, y: 0.0, w: 1920.0, h: 1080.0 };
        let l = layout(m);
        assert_eq!(l.window.x, 2320.0);
        assert_eq!(l.pill.x, 2822.0);
    }

    #[test]
    fn panel_rect_is_flush_with_top_and_centered_and_clamped() {
        let m = mon(1920.0, 1080.0);
        assert_eq!(panel_rect(m, 780.0, 300.0), Rect { x: 570.0, y: 0.0, w: 780.0, h: 300.0 });
        assert_eq!(panel_rect(m, 5000.0, 300.0).w, 1120.0);
        assert_eq!(panel_rect(m, 100.0, 300.0).w, 780.0);
    }
}
```

Add `mod island;` to `lib.rs`.

- [ ] **Step 2: Run and confirm failure.**

Run: `cargo test island::`
Expected: FAIL, missing `layout`.

- [ ] **Step 3: Implement** above the tests module:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

pub const PILL_W: f64 = 116.0;
pub const PILL_H: f64 = 22.0;
pub const TOP_GAP: f64 = 6.0;

pub struct IslandLayout {
    pub window: Rect,
    pub pill: Rect,
    pub hover: Rect,
}

/// Rounds to the nearest even whole number so centered edges land on whole pixels.
fn even(v: f64) -> f64 {
    (v / 2.0).round() * 2.0
}

fn max_panel_width(monitor_w: f64) -> f64 {
    (monitor_w * 0.74).min(1120.0)
}

pub fn layout(monitor: Rect) -> IslandLayout {
    // The window is as big as the largest panel, so the panel can animate inside it.
    let win_w = even(max_panel_width(monitor.w).max(780.0));
    let win_h = even((monitor.h * 0.36).clamp(320.0, 400.0));
    let mid = monitor.x + monitor.w / 2.0;
    let window = Rect { x: (mid - win_w / 2.0).round(), y: monitor.y, w: win_w, h: win_h };
    let pill = Rect { x: (mid - PILL_W / 2.0).round(), y: monitor.y + TOP_GAP, w: PILL_W, h: PILL_H };
    let hover = Rect { x: pill.x - 6.0, y: monitor.y, w: pill.w + 12.0, h: TOP_GAP + PILL_H + 40.0 };
    IslandLayout { window, pill, hover }
}

/// The visible open panel: flush with the top edge, centered, width clamped like the Mac app.
pub fn panel_rect(monitor: Rect, panel_w: f64, panel_h: f64) -> Rect {
    let w = even(panel_w.clamp(780.0, max_panel_width(monitor.w).max(780.0)));
    let mid = monitor.x + monitor.w / 2.0;
    Rect { x: (mid - w / 2.0).round(), y: monitor.y, w, h: panel_h }
}
```

- [ ] **Step 4: Run the tests.**

Run: `cargo test island::`
Expected: 7 passed. (If the 1280 case differs by the rounding rule, fix the code, not the test: the Mac rounds to an even whole number.)

- [ ] **Step 5: Commit.**

```bash
git add src-tauri
git commit -m "feat(island): pure layout math for the window, pill, hover and panel"
```

---

### Task 6: Island state machine

**Files:**
- Create: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod state;`)

**Interfaces:**
- Produces:
  - `#[derive(Default)] pub struct IslandState { pub open: bool, pub pinned: bool, pub panel_w: f64, pub panel_h: f64 }`
  - `impl IslandState`:
    - `pub fn hover_enter(&mut self) -> bool` (returns true if the open state changed)
    - `pub fn hover_leave(&mut self) -> bool` (closes only if not pinned)
    - `pub fn toggle(&mut self) -> bool` (hotkey / tray; opening this way pins it)
    - `pub fn pin(&mut self)` (click inside the panel)
    - `pub fn close(&mut self) -> bool` (Esc, click outside, focus lost; always closes, unpins)

Rules, from the Mac behavior: hover opens it; leaving closes it unless the user clicked inside (pinned); the hotkey and tray open it pinned; Esc, click outside and Alt+N again close it.

- [ ] **Step 1: Write the failing tests** in `state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_opens_and_leaving_closes() {
        let mut s = IslandState::default();
        assert!(s.hover_enter());
        assert!(s.open);
        assert!(!s.hover_enter()); // no change the second time
        assert!(s.hover_leave());
        assert!(!s.open);
    }

    #[test]
    fn clicking_inside_pins_it_open_through_hover_leave() {
        let mut s = IslandState::default();
        s.hover_enter();
        s.pin();
        assert!(!s.hover_leave());
        assert!(s.open);
    }

    #[test]
    fn toggle_opens_pinned_and_closes() {
        let mut s = IslandState::default();
        assert!(s.toggle());
        assert!(s.open && s.pinned);
        assert!(s.toggle());
        assert!(!s.open && !s.pinned);
    }

    #[test]
    fn close_always_closes_and_unpins() {
        let mut s = IslandState::default();
        s.toggle();
        assert!(s.close());
        assert!(!s.open && !s.pinned);
        assert!(!s.close()); // already closed
    }
}
```

Add `mod state;` to `lib.rs`.

- [ ] **Step 2: Run and confirm failure.**

Run: `cargo test state::`
Expected: FAIL, missing `IslandState`.

- [ ] **Step 3: Implement** above the tests module:

```rust
#[derive(Default)]
pub struct IslandState {
    pub open: bool,
    pub pinned: bool,
    pub panel_w: f64,
    pub panel_h: f64,
}

impl IslandState {
    pub fn hover_enter(&mut self) -> bool {
        if self.open {
            return false;
        }
        self.open = true;
        true
    }

    pub fn hover_leave(&mut self) -> bool {
        if !self.open || self.pinned {
            return false;
        }
        self.open = false;
        true
    }

    pub fn toggle(&mut self) -> bool {
        if self.open {
            self.close()
        } else {
            self.open = true;
            self.pinned = true;
            true
        }
    }

    pub fn pin(&mut self) {
        if self.open {
            self.pinned = true;
        }
    }

    pub fn close(&mut self) -> bool {
        self.pinned = false;
        if !self.open {
            return false;
        }
        self.open = false;
        true
    }
}
```

- [ ] **Step 4: Run the tests.**

Run: `cargo test state::`
Expected: 4 passed.

- [ ] **Step 5: Commit.**

```bash
git add src-tauri
git commit -m "feat(island): open/close/pin state machine"
```

---

### Task 7: Island window, hover poller, hotkey, tray icon (the spike)

**Files:**
- Create: `src-tauri/src/window.rs`, `src-tauri/src/commands.rs`, `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: `island::{layout, panel_rect, Rect}`, `state::IslandState`, `db::Db`.
- Produces:
  - Tauri commands: `close_tray()`, `pin_tray()`, `set_panel_size(w: f64, h: f64)`, `tray_is_open() -> bool`
  - Event `tray-open-changed` with payload `bool`
  - Managed state `Mutex<IslandState>` and `Mutex<Db>`

This task is a spike. Tauri's window APIs can differ slightly between 2.x releases, so confirm each call against the version `cargo` resolves, and note any deviation in the commit message. The behavior to achieve is what matters.

- [ ] **Step 1: Add dependencies** to `[dependencies]` in `Cargo.toml` and make sure the tray feature is on:

```toml
tauri = { version = "2", features = ["tray-icon", "image-png"] }
tauri-plugin-global-shortcut = "2"
```

- [ ] **Step 2: Capabilities.** `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "windows": ["island"],
  "permissions": ["core:default", "global-shortcut:default"]
}
```

- [ ] **Step 3: Write `window.rs`.** It creates the window, applies open/closed, and runs the hover poller.

```rust
use crate::island::{layout, panel_rect, Rect};
use crate::state::IslandState;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "island";

/// Logical-pixel rectangle of the monitor the window is on (or the primary one).
fn monitor_rect(win: &WebviewWindow) -> Option<(Rect, f64)> {
    let m = win.current_monitor().ok().flatten().or_else(|| win.primary_monitor().ok().flatten())?;
    let s = m.scale_factor();
    let p = m.position();
    let z = m.size();
    Some((Rect { x: p.x as f64 / s, y: p.y as f64 / s, w: z.width as f64 / s, h: z.height as f64 / s }, s))
}

pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::default())
        .title("NotchCat")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;
    position(&win);
    win.set_ignore_cursor_events(true)?; // closed means click-through
    win.show()?;
    Ok(win)
}

/// Sizes and places the window at the top center of its monitor.
pub fn position(win: &WebviewWindow) {
    if let Some((mon, s)) = monitor_rect(win) {
        let l = layout(mon).window;
        let _ = win.set_size(tauri::LogicalSize::new(l.w, l.h));
        let _ = win.set_position(tauri::LogicalPosition::new(l.x, l.y));
        let _ = s;
    }
}

/// Applies the new open/closed state to the window and tells the UI.
pub fn apply(app: &AppHandle, open: bool) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.set_ignore_cursor_events(!open);
        if open {
            position(&win); // follow the monitor in case it changed
            let _ = win.set_focus();
        }
    }
    let _ = app.emit("tray-open-changed", open);
}

/// Polls the cursor (about 30 times a second) to open on hover and close on leave.
/// The window is click-through while closed, so it cannot receive mouse events itself.
pub fn start_hover_poller(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(33));
        let Some(win) = app.get_webview_window(LABEL) else { continue };
        let Some((mon, scale)) = monitor_rect(&win) else { continue };
        let Ok(pos) = app.cursor_position() else { continue };
        let (cx, cy) = (pos.x / scale, pos.y / scale);

        let state = app.state::<Mutex<IslandState>>();
        let changed = {
            let mut s = state.lock().unwrap();
            if !s.open {
                if layout(mon).hover.contains(cx, cy) { s.hover_enter().then_some(true) } else { None }
            } else {
                let panel = panel_rect(mon, s.panel_w, s.panel_h);
                // A little slack below and beside the panel so the edge is forgiving.
                let slack = Rect { x: panel.x - 12.0, y: panel.y, w: panel.w + 24.0, h: panel.h + 16.0 };
                let over_pill = layout(mon).hover.contains(cx, cy);
                if !slack.contains(cx, cy) && !over_pill { s.hover_leave().then_some(false) } else { None }
            }
        };
        if let Some(open) = changed {
            apply(&app, open);
        }
    });
}
```

- [ ] **Step 4: Write `commands.rs`.**

```rust
use crate::state::IslandState;
use crate::window;
use std::sync::Mutex;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn close_tray(app: AppHandle, state: State<Mutex<IslandState>>) {
    let changed = state.lock().unwrap().close();
    if changed {
        window::apply(&app, false);
    }
}

#[tauri::command]
pub fn pin_tray(state: State<Mutex<IslandState>>) {
    state.lock().unwrap().pin();
}

#[tauri::command]
pub fn set_panel_size(w: f64, h: f64, state: State<Mutex<IslandState>>) {
    let mut s = state.lock().unwrap();
    s.panel_w = w;
    s.panel_h = h;
}

#[tauri::command]
pub fn tray_is_open(state: State<Mutex<IslandState>>) -> bool {
    state.lock().unwrap().open
}
```

- [ ] **Step 5: Wire everything in `lib.rs`.** Replace the generated `run` with:

```rust
mod commands;
mod db;
mod island;
mod models;
mod repo;
mod state;
mod window;

use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

fn toggle(app: &tauri::AppHandle) {
    let open = {
        let st = app.state::<Mutex<state::IslandState>>();
        let mut s = st.lock().unwrap();
        s.toggle();
        s.open
    };
    window::apply(app, open);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let alt_n = Shortcut::new(Some(Modifiers::ALT), Code::KeyN);

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &alt_n && event.state() == ShortcutState::Pressed {
                        toggle(app);
                    }
                })
                .build(),
        )
        .manage(Mutex::new(state::IslandState { panel_w: 780.0, panel_h: 320.0, ..Default::default() }))
        .invoke_handler(tauri::generate_handler![
            commands::close_tray,
            commands::pin_tray,
            commands::set_panel_size,
            commands::tray_is_open
        ])
        .setup(move |app| {
            // Data layer: opened once, shared as managed state.
            let db = db::Db::open(&db::Db::default_path()).expect("open database");
            app.manage(Mutex::new(db));

            let win = window::create(app.handle())?;
            // Clicking away from the open tray closes it.
            let handle = app.handle().clone();
            win.on_window_event(move |e| {
                if let WindowEvent::Focused(false) = e {
                    let changed = handle.state::<Mutex<state::IslandState>>().lock().unwrap().close();
                    if changed {
                        window::apply(&handle, false);
                    }
                }
            });

            if app.global_shortcut().register(alt_n).is_err() {
                eprintln!("NotchCat: Alt+N is taken by another app; use the tray icon instead");
            }

            // Tray icon: Open / Quit.
            let open_item = MenuItem::with_id(app, "open", "Open NotchCat", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit NotchCat", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &quit_item])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().expect("app icon"))
                .menu(&menu)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "open" => toggle(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            window::start_hover_poller(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running NotchCat");
}
```

`main.rs` stays as the scaffold generated it (`notchcat_windows_lib::run()` or similar; keep the generated crate name).

- [ ] **Step 6: Build.**

Run: `cd src-tauri; cargo build`
Expected: compiles. Fix any API differences against the resolved Tauri version (`cursor_position`, `set_ignore_cursor_events`, builder methods) and keep the behavior the same.

- [ ] **Step 7: Commit.**

```bash
git add src-tauri
git commit -m "feat(island): transparent top-center window, hover poller, Alt+N, tray icon"
```

---

### Task 8: Frontend shell and visual verification

**Files:**
- Create/replace: `src/App.tsx`, `src/app.css`, `src/main.tsx`
- Delete: the scaffold's demo CSS and logos (`src/App.css`, `src/assets/react.svg`, `public/*.svg`)

**Interfaces:**
- Consumes: event `tray-open-changed` (`boolean`), commands `close_tray`, `pin_tray`, `set_panel_size`, `tray_is_open`.
- Produces: `<App/>` with a `.pill` (collapsed) and a `.panel` (open) that animate between each other. The panel is a placeholder; Plan 2 fills it with cards.

- [ ] **Step 1: `src/main.tsx`.**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./tokens.css";
import "./app.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 2: `src/App.tsx`.**

```tsx
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import mascot from "./assets/mascot.png";

const PANEL_W = 780;
const PANEL_H = 320;

export default function App() {
  const [open, setOpen] = useState(false);
  const panel = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke("set_panel_size", { w: PANEL_W, h: PANEL_H });
    invoke<boolean>("tray_is_open").then(setOpen);
    const un = listen<boolean>("tray-open-changed", (e) => setOpen(e.payload));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") invoke("close_tray");
    };
    window.addEventListener("keydown", onKey);
    return () => {
      un.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  return (
    <div className={`island ${open ? "open" : ""}`}>
      <div className="pill" aria-hidden={open} />
      <div
        ref={panel}
        className="panel"
        style={{ width: PANEL_W, height: PANEL_H }}
        onMouseDown={() => invoke("pin_tray")}
      >
        <img src={mascot} alt="" width={48} height={48} />
        <span>NotchCat</span>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: `src/app.css`.** The panel grows out of the pill: it starts pill-sized, flush with the top, and scales to full size. Timings are a first guess; Plan 3 tunes them against the Mac recording.

```css
html, body, #root { margin: 0; height: 100%; background: transparent; overflow: hidden; }
body { font-family: "Segoe UI Variable", "Segoe UI", system-ui, sans-serif; }

.island { position: relative; width: 100%; height: 100%; display: flex; justify-content: center; align-items: flex-start; }

.pill {
  position: absolute; top: 6px; width: 116px; height: 22px;
  background: var(--housing); border-radius: 11px;
  transition: opacity 120ms ease; opacity: 1;
}
.island.open .pill { opacity: 0; }

.panel {
  position: absolute; top: 0;
  background: var(--housing);
  border-radius: 0 0 var(--card-radius) var(--card-radius);
  color: var(--amber);
  display: flex; align-items: center; justify-content: center; gap: 12px;
  font-size: 20px;
  transform-origin: top center;
  transform: scale(0.15, 0.07);
  opacity: 0; pointer-events: none;
  transition: transform 280ms cubic-bezier(0.2, 0.9, 0.25, 1), opacity 140ms ease;
}
.island.open .panel { transform: scale(1); opacity: 1; pointer-events: auto; }
```

- [ ] **Step 4: Remove scaffold leftovers.**

```powershell
Remove-Item src\App.css, src\assets\react.svg, public\*.svg -ErrorAction SilentlyContinue
```

- [ ] **Step 5: Run the app and verify by hand.**

Run: `npm run tauri dev`
Check each item:
1. A black pill shows at the top center of the screen, floating 6 px from the top, over other windows.
2. It does not appear in the taskbar, and clicking it while closed does not steal clicks from apps under it (outside the pill area).
3. Moving the cursor to the pill opens a black panel that grows out of it, with the mascot and "NotchCat".
4. Moving the cursor away closes it.
5. Hover to open, then click inside the panel: moving away no longer closes it. Esc closes it. Clicking another app also closes it.
6. Alt+N opens it pinned. Alt+N again closes it. The tray cat icon's "Open NotchCat" does the same, and "Quit NotchCat" exits.
7. Idle CPU: Task Manager shows NotchCat near 0% while closed.
8. If you have two monitors or a scaled display (125% or 150%), the pill is still centered on the monitor with the cursor... note that the window follows the monitor it was created on. Record any problem; fixing multi-monitor follow is Plan 3.

Expected: items 1 to 7 pass. Write down anything that fails in `docs/superpowers/notes/island-spike.md` (monitor, scale, what happened) so Plan 3 can fix it. Transparent-window edges, a shadow around the panel, or flicker on open are the likely issues; if the transparent window is unusable on this machine, stop and report, because the spec's risk section says this decides the window approach.

- [ ] **Step 6: Commit.**

```bash
git add -A
git commit -m "feat(ui): pill and panel shell wired to the island state"
```

---

## Self-Review

**Spec coverage (Plan 1 only).**
- Phase 0 reference capture: the Theme tokens are extracted (Task 1). Geometry numbers come from `NotchGeometry.swift`. Mac snapshot PNGs and animation timings are not captured here; they need the Mac and are folded into Plan 3 (animation tuning) and Plan 6 (parity QA). The Mac DB fixture is Task 4 Step 5.
- Phase 1 island spike: Tasks 5 to 8 (window, hover, Alt+N, tray icon, Esc, click-outside). Multi-monitor follow and fullscreen-app hiding are deferred and recorded by Task 8 Step 5.
- Phase 2 data layer: Tasks 2 to 4. `LegacyImporter` (v1 `state.json`) is intentionally not ported: Windows never had a v1.
- Spec items not in this plan (cards, sync, integrations, installer) belong to Plans 2 to 6.

**Placeholder scan.** No TBD or "similar to" steps. Task 7 is explicitly a spike with the API-version caveat stated, and every step has full code.

**Type consistency.** `Db`, `TaskItem`, `FinishedTask`, `FocusSession` and the repo function names match between Tasks 2, 3 and 4. `IslandState` fields (`open`, `pinned`, `panel_w`, `panel_h`) and methods match between Tasks 6 and 7. `layout`, `panel_rect`, `Rect` match between Tasks 5 and 7. Event name `tray-open-changed` and the four command names match between Tasks 7 and 8.

**Known soft spots.** Hover leave uses the declared panel size reported by the UI (`set_panel_size`); Plan 2 must update it whenever the panel changes size. The pill height (22) and top gap (6) are Windows guesses, since the Mac pill sits inside the menu bar; tune them in Plan 3.
