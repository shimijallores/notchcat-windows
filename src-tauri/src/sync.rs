use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::models::{
    normalize_uuid_lower, normalize_uuid_upper, FocusSession, JSONValue, PullResult, PushBody,
    PushResult, TokenPair, WireChange, WireRecord,
};
use crate::repo;
use crate::store::{now_ms, secs_to_ms, AppSettings, NoteStore, TaskStore};

pub const DEFAULT_SERVER: &str = "https://api.notchcat.cloud";
const KEYRING_SERVICE_PROD: &str = "app.notchcat.sync";
const KEYRING_SERVICE_TEST: &str = "app.notchcat.sync.test";
const CURSOR_META_KEY: &str = "cursor";
const USER_ID_META_KEY: &str = "user_id";
const USERNAME_META_KEY: &str = "username";
const SERVER_URL_META_KEY: &str = "server_url";
const LAST_SYNC_META_KEY: &str = "last_sync_at";
const ENROLLED_META_KEY: &str = "enrolled_for_sync";
const SYNC_SCHEMA_VERSION_KEY: &str = "sync_schema_version";
const CURRENT_SYNC_SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum SyncPhase {
    SignedOut,
    Idle,
    Syncing,
    Offline,
    Failed(String),
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusSnapshot {
    pub phase: SyncPhase,
    pub last_sync_at: Option<i64>,
    pub pending_count: usize,
    pub server_url: String,
    pub username: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SyncError {
    InvalidUrl,
    SignedOut,
    Offline(String),
    Unauthorized(String),
    Revoked(String),
    Server { status: u16, message: String },
    BadResponse(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::InvalidUrl => write!(f, "Invalid server URL"),
            SyncError::SignedOut => write!(f, "Signed out"),
            SyncError::Offline(msg) => write!(f, "Offline: {msg}"),
            SyncError::Unauthorized(msg) => write!(f, "{msg}"),
            SyncError::Revoked(msg) => write!(f, "{msg}"),
            SyncError::Server { status, message } => {
                if message.is_empty() {
                    write!(f, "Server error ({status})")
                } else {
                    write!(f, "{message}")
                }
            }
            SyncError::BadResponse(msg) => write!(f, "Invalid server response: {msg}"),
        }
    }
}

impl std::error::Error for SyncError {}

#[derive(Debug, Clone)]
pub struct CredentialStore {
    service: String,
    account: String,
    fallback_path: Option<PathBuf>,
    force_file_only: bool,
}

impl CredentialStore {
    pub fn for_db(db_path: Option<&Path>, account: &str, isolated: bool) -> Self {
        let env_db_set = std::env::var_os("NOTCHCAT_DB").is_some();
        let force_file_only = isolated || env_db_set;
        let service = if force_file_only {
            KEYRING_SERVICE_TEST.to_string()
        } else {
            KEYRING_SERVICE_PROD.to_string()
        };
        let fallback_path = db_path.map(|p| {
            let mut os = p.as_os_str().to_os_string();
            os.push(format!(".{account}.tok"));
            PathBuf::from(os)
        });
        Self {
            service,
            account: account.to_string(),
            fallback_path,
            force_file_only,
        }
    }

    pub fn read(&self) -> Option<String> {
        if !self.force_file_only {
            if let Ok(entry) = keyring::Entry::new(&self.service, &self.account) {
                if let Ok(val) = entry.get_password() {
                    if !val.is_empty() {
                        return Some(val);
                    }
                }
            }
        }
        let path = self.fallback_path.as_ref()?;
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn write(&self, token: &str) {
        if !self.force_file_only {
            if let Ok(entry) = keyring::Entry::new(&self.service, &self.account) {
                let _ = entry.set_password(token);
            }
        }
        if let Some(path) = &self.fallback_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, token);
        }
    }

    pub fn clear(&self) {
        if !self.force_file_only {
            if let Ok(entry) = keyring::Entry::new(&self.service, &self.account) {
                let _ = entry.delete_credential();
            }
        }
        if let Some(path) = &self.fallback_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn normalized_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let parsed = reqwest::Url::parse(&with_scheme).ok()?;
    let host = parsed.host_str()?;
    if host.is_empty() {
        return None;
    }
    Some(with_scheme)
}

#[derive(Debug, Serialize)]
struct CheckKeyReq<'a> {
    access_key: &'a str,
}

#[derive(Debug, Serialize)]
struct SignupReq<'a> {
    access_key: &'a str,
    username: &'a str,
    password: &'a str,
    device_name: &'a str,
}

#[derive(Debug, Serialize)]
struct LoginReq<'a> {
    username: &'a str,
    password: &'a str,
    device_name: &'a str,
}

#[derive(Debug, Serialize)]
struct RefreshReq<'a> {
    refresh_token: &'a str,
}

pub fn extract_server_error_message(body: &[u8]) -> Option<String> {
    let val: serde_json::Value = serde_json::from_slice(body).ok()?;
    let detail = val.get("detail")?;
    if let Some(s) = detail.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = detail.as_array() {
        let msgs: Vec<String> = arr
            .iter()
            .filter_map(|item| item.get("msg").and_then(|m| m.as_str()).map(str::to_string))
            .collect();
        if !msgs.is_empty() {
            return Some(msgs.join("; "));
        }
    }
    None
}

fn map_reqwest_error(err: reqwest::Error) -> SyncError {
    if err.is_connect() || err.is_timeout() || err.is_request() {
        SyncError::Offline(err.to_string())
    } else {
        SyncError::BadResponse(err.to_string())
    }
}

fn map_http_error(status: StatusCode, body: &[u8]) -> SyncError {
    let msg = extract_server_error_message(body)
        .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
    match status {
        StatusCode::UNAUTHORIZED => SyncError::Unauthorized(msg),
        StatusCode::FORBIDDEN => SyncError::Revoked(msg),
        _ => SyncError::Server {
            status: status.as_u16(),
            message: msg,
        },
    }
}

#[derive(Clone)]
pub struct SyncAPI {
    client: reqwest::Client,
}

impl SyncAPI {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    pub async fn check_key(&self, server_url: &str, access_key: &str) -> Result<(), SyncError> {
        let base = normalized_url(server_url).ok_or(SyncError::InvalidUrl)?;
        let resp = self
            .client
            .post(format!("{base}/v1/auth/check-key"))
            .json(&CheckKeyReq { access_key })
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(map_reqwest_error)?;
        if !status.is_success() {
            return Err(map_http_error(status, &bytes));
        }
        Ok(())
    }

    pub async fn sign_up(
        &self,
        server_url: &str,
        access_key: &str,
        username: &str,
        password: &str,
        device_name: &str,
    ) -> Result<TokenPair, SyncError> {
        let base = normalized_url(server_url).ok_or(SyncError::InvalidUrl)?;
        let resp = self
            .client
            .post(format!("{base}/v1/auth/signup"))
            .json(&SignupReq {
                access_key,
                username,
                password,
                device_name,
            })
            .send()
            .await
            .map_err(map_reqwest_error)?;
        self.parse_token_response(resp).await
    }

    pub async fn login(
        &self,
        server_url: &str,
        username: &str,
        password: &str,
        device_name: &str,
    ) -> Result<TokenPair, SyncError> {
        let base = normalized_url(server_url).ok_or(SyncError::InvalidUrl)?;
        let resp = self
            .client
            .post(format!("{base}/v1/auth/login"))
            .json(&LoginReq {
                username,
                password,
                device_name,
            })
            .send()
            .await
            .map_err(map_reqwest_error)?;
        self.parse_token_response(resp).await
    }

    pub async fn refresh(
        &self,
        server_url: &str,
        refresh_token: &str,
    ) -> Result<TokenPair, SyncError> {
        let base = normalized_url(server_url).ok_or(SyncError::InvalidUrl)?;
        let resp = self
            .client
            .post(format!("{base}/v1/auth/refresh"))
            .json(&RefreshReq { refresh_token })
            .send()
            .await
            .map_err(map_reqwest_error)?;
        self.parse_token_response(resp).await
    }

    pub async fn logout(&self, server_url: &str, refresh_token: &str) {
        let Some(base) = normalized_url(server_url) else {
            return;
        };
        let _ = self
            .client
            .post(format!("{base}/v1/auth/logout"))
            .json(&RefreshReq { refresh_token })
            .send()
            .await;
    }

    pub async fn push(
        &self,
        server_url: &str,
        access_token: &str,
        body: &PushBody,
    ) -> Result<PushResult, SyncError> {
        let base = normalized_url(server_url).ok_or(SyncError::InvalidUrl)?;
        let resp = self
            .client
            .post(format!("{base}/v1/sync/push"))
            .bearer_auth(access_token)
            .json(body)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(map_reqwest_error)?;
        if !status.is_success() {
            return Err(map_http_error(status, &bytes));
        }
        serde_json::from_slice(&bytes).map_err(|e| SyncError::BadResponse(e.to_string()))
    }

    pub async fn pull(
        &self,
        server_url: &str,
        access_token: &str,
        cursor: i64,
        limit: i64,
    ) -> Result<PullResult, SyncError> {
        let base = normalized_url(server_url).ok_or(SyncError::InvalidUrl)?;
        let resp = self
            .client
            .get(format!("{base}/v1/sync/pull?cursor={cursor}&limit={limit}"))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(map_reqwest_error)?;
        if !status.is_success() {
            return Err(map_http_error(status, &bytes));
        }
        serde_json::from_slice(&bytes).map_err(|e| SyncError::BadResponse(e.to_string()))
    }

    async fn parse_token_response(&self, resp: reqwest::Response) -> Result<TokenPair, SyncError> {
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(map_reqwest_error)?;
        if !status.is_success() {
            return Err(map_http_error(status, &bytes));
        }
        serde_json::from_slice(&bytes).map_err(|e| SyncError::BadResponse(e.to_string()))
    }
}

pub struct SessionSync;

impl SessionSync {
    pub fn pending_changes(db: &Db, limit: usize) -> Vec<WireChange> {
        repo::session_unsent(db, limit as i64)
            .into_iter()
            .filter_map(|(sync_id, s)| {
                let id = normalize_uuid_lower(&sync_id)?;
                let ts = secs_to_ms(s.started_at);
                let mut fields = std::collections::BTreeMap::new();
                fields.insert(
                    "task_id".to_string(),
                    match s.task_id.as_deref().and_then(normalize_uuid_lower) {
                        Some(tid) => JSONValue::String(tid),
                        None => JSONValue::Null,
                    },
                );
                fields.insert("started_at".to_string(), JSONValue::Double(s.started_at));
                fields.insert("active_seconds".to_string(), JSONValue::Int(s.active_seconds));
                fields.insert("planned_seconds".to_string(), JSONValue::Int(s.planned_seconds));
                fields.insert("completed".to_string(), JSONValue::Bool(s.completed));
                let field_ts = fields.keys().map(|k| (k.clone(), ts)).collect();
                Some(WireChange {
                    collection: "sessions".to_string(),
                    id,
                    fields,
                    field_ts,
                    base_seq: None,
                })
            })
            .collect()
    }

    pub fn apply(db: &Db, records: &[WireRecord]) -> usize {
        let mut arrived = 0usize;
        let mut mark_ids = Vec::new();
        for rec in records {
            if rec.collection != "sessions" {
                continue;
            }
            let f = &rec.fields;
            let Some(started) = f.get("started_at").and_then(|v| v.as_f64()) else {
                continue;
            };
            let Some(active) = f.get("active_seconds").and_then(|v| v.as_i64()) else {
                continue;
            };
            let Some(planned) = f.get("planned_seconds").and_then(|v| v.as_i64()) else {
                continue;
            };
            let Some(completed) = f.get("completed").and_then(|v| v.as_bool()) else {
                continue;
            };
            let task_id = f
                .get("task_id")
                .and_then(|v| v.as_str())
                .and_then(normalize_uuid_upper);
            let session = FocusSession {
                task_id,
                started_at: started,
                active_seconds: active,
                planned_seconds: planned,
                completed,
            };
            if repo::session_insert_remote(db, &rec.id, &session) {
                arrived += 1;
            } else {
                mark_ids.push(rec.id.clone());
            }
        }
        if !mark_ids.is_empty() {
            repo::session_mark_pushed(db, &mark_ids);
        }
        arrived
    }

    pub fn drop_pending(db: &Db, ids: &[String]) {
        repo::session_mark_pushed(db, ids);
    }

    pub fn reset_sync_state(db: &Db) {
        repo::session_reset_pushed(db);
    }
}

pub trait SyncStoresAdapter {
    fn db(&self) -> &Db;
    fn pending_changes(&mut self, limit: usize) -> Vec<WireChange>;
    fn pending_count(&self) -> usize;
    fn apply_records(&mut self, records: &[WireRecord]);
    fn apply_conflicts(&mut self, conflicts: &[WireRecord]);
    fn drop_pending(&mut self, collection: &str, ids: &[String]);
    fn enroll_all(&mut self);
    fn reset_all_sync_state(&mut self);
}

pub struct DirectStores {
    pub db: Db,
    pub tasks: TaskStore,
    pub note: NoteStore,
    pub settings: AppSettings,
}

impl SyncStoresAdapter for DirectStores {
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
        let _ = SessionSync::apply(&self.db, records);
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

pub struct SyncEngine {
    creds: CredentialStore,
    api: SyncAPI,
    device_name: String,
    inner: Mutex<SyncInner>,
}

struct SyncInner {
    phase: SyncPhase,
    cursor: i64,
    last_sync_at: Option<i64>,
    server_url: String,
    username: Option<String>,
    access_token: Option<String>,
    access_expiry_ms: i64,
    sync_in_flight: bool,
}

impl SyncEngine {
    pub fn new(db: &Db, creds: CredentialStore) -> Self {
        let device_name = std::env::var("COMPUTERNAME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Windows PC".to_string());

        let env_server = std::env::var("NOTCHCAT_SERVER")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let server_url = repo::sync_meta_get(db, SERVER_URL_META_KEY)
            .or(env_server)
            .unwrap_or_else(|| DEFAULT_SERVER.to_string());

        let schema_ver = repo::sync_meta_get(db, SYNC_SCHEMA_VERSION_KEY)
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        if schema_ver < CURRENT_SYNC_SCHEMA_VERSION {
            repo::sync_meta_set(db, CURSOR_META_KEY, Some("0"));
            repo::sync_meta_set(
                db,
                SYNC_SCHEMA_VERSION_KEY,
                Some(&CURRENT_SYNC_SCHEMA_VERSION.to_string()),
            );
        }

        let cursor = repo::sync_meta_get(db, CURSOR_META_KEY)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let last_sync_at = repo::sync_meta_get(db, LAST_SYNC_META_KEY).and_then(|s| s.parse().ok());
        let username = repo::sync_meta_get(db, USERNAME_META_KEY);
        let has_refresh = creds.read().is_some();
        let phase = if username.is_some() && has_refresh {
            SyncPhase::Idle
        } else {
            SyncPhase::SignedOut
        };

        Self {
            creds,
            api: SyncAPI::new(),
            device_name,
            inner: Mutex::new(SyncInner {
                phase,
                cursor,
                last_sync_at,
                server_url,
                username,
                access_token: None,
                access_expiry_ms: 0,
                sync_in_flight: false,
            }),
        }
    }

    pub fn snapshot(&self, pending_count: usize) -> SyncStatusSnapshot {
        let g = self.inner.lock().unwrap();
        SyncStatusSnapshot {
            phase: g.phase.clone(),
            last_sync_at: g.last_sync_at,
            pending_count,
            server_url: g.server_url.clone(),
            username: g.username.clone(),
        }
    }

    pub fn is_signed_in(&self) -> bool {
        !matches!(self.inner.lock().unwrap().phase, SyncPhase::SignedOut)
    }

    pub fn set_server_url(&self, db: &Db, raw: &str) {
        let trimmed = raw.trim().to_string();
        let effective = if trimmed.is_empty() {
            DEFAULT_SERVER.to_string()
        } else {
            trimmed
        };
        repo::sync_meta_set(db, SERVER_URL_META_KEY, Some(&effective));
        self.inner.lock().unwrap().server_url = effective;
    }

    pub async fn check_access_key(&self, server_url: &str, key: &str) -> Result<(), String> {
        let base = normalized_url(server_url).ok_or_else(|| "Invalid server URL".to_string())?;
        self.api
            .check_key(&base, key.trim())
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn sign_up<S: SyncStoresAdapter + Send>(
        &self,
        stores: Arc<Mutex<S>>,
        server_url: &str,
        access_key: &str,
        username: &str,
        password: &str,
    ) -> Result<(), String> {
        let base = normalized_url(server_url).ok_or_else(|| "Invalid server URL".to_string())?;
        let tokens = self
            .api
            .sign_up(&base, access_key.trim(), username.trim(), password, &self.device_name)
            .await
            .map_err(|e| e.to_string())?;

        self.accept_tokens(&stores, &base, &tokens);
        self.sync_now(stores).await;
        Ok(())
    }

    pub async fn sign_in<S: SyncStoresAdapter + Send>(
        &self,
        stores: Arc<Mutex<S>>,
        server_url: &str,
        username: &str,
        password: &str,
    ) -> Result<(), String> {
        let base = normalized_url(server_url).ok_or_else(|| "Invalid server URL".to_string())?;
        let tokens = self
            .api
            .login(&base, username.trim(), password, &self.device_name)
            .await
            .map_err(|e| e.to_string())?;

        self.accept_tokens(&stores, &base, &tokens);
        self.sync_now(stores).await;
        Ok(())
    }

    fn accept_tokens<S: SyncStoresAdapter>(
        &self,
        stores: &Arc<Mutex<S>>,
        server_url: &str,
        tokens: &TokenPair,
    ) {
        self.creds.write(&tokens.refresh_token);
        let mut new_cursor: i64 = 0;

        {
            let mut s = stores.lock().unwrap();
            let prev_uid = repo::sync_meta_get(s.db(), USER_ID_META_KEY);
            let account_changed = prev_uid.as_deref() != Some(tokens.user.id.as_str());
            if account_changed {
                s.reset_all_sync_state();
                repo::sync_meta_set(s.db(), CURSOR_META_KEY, Some("0"));
                repo::sync_meta_set(s.db(), ENROLLED_META_KEY, Some("0"));
            } else {
                new_cursor = repo::sync_meta_get(s.db(), CURSOR_META_KEY)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
            repo::sync_meta_set(s.db(), SERVER_URL_META_KEY, Some(server_url));
            repo::sync_meta_set(s.db(), USER_ID_META_KEY, Some(&tokens.user.id));
            repo::sync_meta_set(s.db(), USERNAME_META_KEY, Some(&tokens.user.username));
            if repo::sync_meta_get(s.db(), ENROLLED_META_KEY).as_deref() != Some("1") {
                s.enroll_all();
                repo::sync_meta_set(s.db(), ENROLLED_META_KEY, Some("1"));
            }
        }

        let mut g = self.inner.lock().unwrap();
        g.phase = SyncPhase::Idle;
        g.cursor = new_cursor;
        g.server_url = server_url.to_string();
        g.username = Some(tokens.user.username.clone());
        g.access_token = Some(tokens.access_token.clone());
        g.access_expiry_ms = now_ms() + ((tokens.expires_in - 30).max(10) * 1000);
    }

    pub async fn sign_out<S: SyncStoresAdapter + Send>(&self, stores: Arc<Mutex<S>>) {
        let server_url = self.inner.lock().unwrap().server_url.clone();
        if let Some(rt) = self.creds.read() {
            self.api.logout(&server_url, &rt).await;
        }
        self.creds.clear();

        {
            let mut s = stores.lock().unwrap();
            s.reset_all_sync_state();
            repo::sync_meta_set(s.db(), CURSOR_META_KEY, Some("0"));
            repo::sync_meta_set(s.db(), ENROLLED_META_KEY, Some("0"));
            repo::sync_meta_set(s.db(), USER_ID_META_KEY, None);
            repo::sync_meta_set(s.db(), USERNAME_META_KEY, None);
            repo::sync_meta_set(s.db(), LAST_SYNC_META_KEY, None);
        }

        let mut g = self.inner.lock().unwrap();
        g.phase = SyncPhase::SignedOut;
        g.cursor = 0;
        g.username = None;
        g.last_sync_at = None;
        g.access_token = None;
        g.access_expiry_ms = 0;
    }

    pub async fn sync_now<S: SyncStoresAdapter + Send>(&self, stores: Arc<Mutex<S>>) {
        let (server_url, mut access_token, access_valid) = {
            let mut g = self.inner.lock().unwrap();
            if g.sync_in_flight || matches!(g.phase, SyncPhase::SignedOut | SyncPhase::Revoked) {
                return;
            }
            if g.server_url.is_empty() {
                g.phase = SyncPhase::SignedOut;
                return;
            }
            g.sync_in_flight = true;
            g.phase = SyncPhase::Syncing;
            let valid = g.access_token.is_some() && now_ms() < g.access_expiry_ms;
            (g.server_url.clone(), g.access_token.clone(), valid)
        };

        let result = self
            .run_sync_loop(&stores, &server_url, &mut access_token, access_valid)
            .await;

        let mut g = self.inner.lock().unwrap();
        g.sync_in_flight = false;
        match result {
            Ok(()) => {
                let ts = now_ms();
                let s = stores.lock().unwrap();
                repo::sync_meta_set(s.db(), LAST_SYNC_META_KEY, Some(&ts.to_string()));
                g.last_sync_at = Some(ts);
                g.phase = SyncPhase::Idle;
            }
            Err(SyncError::Offline(_)) => g.phase = SyncPhase::Offline,
            Err(SyncError::Revoked(_)) => g.phase = SyncPhase::Revoked,
            Err(SyncError::Unauthorized(msg)) => {
                g.phase = SyncPhase::Failed(format!("Session expired ({msg}). Please sign in again."));
            }
            Err(SyncError::SignedOut) => g.phase = SyncPhase::SignedOut,
            Err(e) => g.phase = SyncPhase::Failed(e.to_string()),
        }
    }

    async fn refresh_access_token(&self, server_url: &str) -> Result<String, SyncError> {
        let rt = self
            .creds
            .read()
            .ok_or_else(|| SyncError::Unauthorized("missing refresh token".into()))?;
        let tokens = self.api.refresh(server_url, &rt).await?;
        self.creds.write(&tokens.refresh_token);
        let mut g = self.inner.lock().unwrap();
        g.access_token = Some(tokens.access_token.clone());
        g.access_expiry_ms = now_ms() + ((tokens.expires_in - 30).max(10) * 1000);
        g.username = Some(tokens.user.username);
        Ok(tokens.access_token)
    }

    async fn run_sync_loop<S: SyncStoresAdapter>(
        &self,
        stores: &Arc<Mutex<S>>,
        server_url: &str,
        access_token: &mut Option<String>,
        access_valid: bool,
    ) -> Result<(), SyncError> {
        let mut token = if access_valid {
            access_token.clone().unwrap()
        } else {
            self.refresh_access_token(server_url).await?
        };
        let mut refreshed = false;

        // 1. Push dirty batches (up to 50 rounds x 200 items)
        for _ in 0..50 {
            let changes = {
                let mut s = stores.lock().unwrap();
                s.pending_changes(200)
            };
            if changes.is_empty() {
                break;
            }
            let body = PushBody { changes };
            let push_res = match self.api.push(server_url, &token, &body).await {
                Ok(r) => r,
                Err(SyncError::Unauthorized(_)) if !refreshed => {
                    refreshed = true;
                    token = self.refresh_access_token(server_url).await?;
                    self.api.push(server_url, &token, &body).await?
                }
                Err(e) => return Err(e),
            };

            let had_conflicts = !push_res.conflicts.is_empty();
            {
                let mut s = stores.lock().unwrap();
                if !push_res.records.is_empty() {
                    s.apply_records(&push_res.records);
                }
                if !push_res.conflicts.is_empty() {
                    s.apply_conflicts(&push_res.conflicts);
                }
                let mut reject_map: HashMap<String, Vec<String>> = HashMap::new();
                for rej in &push_res.rejected {
                    if rej.reason != "stale" && rej.reason != "conflict" {
                        reject_map
                            .entry(rej.collection.clone())
                            .or_default()
                            .push(rej.id.clone());
                    }
                }
                for (coll, ids) in reject_map {
                    s.drop_pending(&coll, &ids);
                }
            }

            if !had_conflicts && body.changes.len() < 200 {
                // Continue loop until pending_changes is empty (e.g. if conflict produced reconciled note)
            }
        }

        // 2. Pull remote changes
        let mut cursor = self.inner.lock().unwrap().cursor;
        for _ in 0..100 {
            let pull_res = match self.api.pull(server_url, &token, cursor, 500).await {
                Ok(r) => r,
                Err(SyncError::Unauthorized(_)) if !refreshed => {
                    refreshed = true;
                    token = self.refresh_access_token(server_url).await?;
                    self.api.pull(server_url, &token, cursor, 500).await?
                }
                Err(e) => return Err(e),
            };

            {
                let mut s = stores.lock().unwrap();
                if !pull_res.records.is_empty() {
                    s.apply_records(&pull_res.records);
                }
                cursor = pull_res.cursor;
                repo::sync_meta_set(s.db(), CURSOR_META_KEY, Some(&cursor.to_string()));
            }
            self.inner.lock().unwrap().cursor = cursor;
            if !pull_res.has_more {
                break;
            }
        }

        // 3. Re-push if conflict resolution or pull merge created new local edits (e.g. reconciled note)
        let extra = stores.lock().unwrap().pending_changes(200);
        if !extra.is_empty() {
            let push_res = self
                .api
                .push(server_url, &token, &PushBody { changes: extra })
                .await?;
            let mut s = stores.lock().unwrap();
            if !push_res.records.is_empty() {
                s.apply_records(&push_res.records);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FocusSession;
    use uuid::Uuid;

    #[test]
    fn normalized_url_adds_https_and_strips_trailing_slash() {
        assert_eq!(
            normalized_url("api.notchcat.cloud/"),
            Some("https://api.notchcat.cloud".to_string())
        );
        assert_eq!(
            normalized_url("http://127.0.0.1:8787///"),
            Some("http://127.0.0.1:8787".to_string())
        );
        assert_eq!(normalized_url("   "), None);
    }

    #[test]
    fn extract_server_error_message_reads_fastapi_shapes() {
        let s = br#"{"detail":"Invalid access key"}"#;
        assert_eq!(
            extract_server_error_message(s).as_deref(),
            Some("Invalid access key")
        );
        let arr = br#"{"detail":[{"msg":"field required"},{"msg":"too short"}]}"#;
        assert_eq!(
            extract_server_error_message(arr).as_deref(),
            Some("field required; too short")
        );
    }

    #[test]
    fn session_sync_round_trips_remote_records() {
        let db = Db::open_memory().unwrap();
        let tid = Uuid::new_v4().to_string().to_uppercase();
        repo::session_insert(
            &db,
            &FocusSession {
                task_id: Some(tid.clone()),
                started_at: 1000.0,
                active_seconds: 1500,
                planned_seconds: 1500,
                completed: true,
            },
        );

        let changes = SessionSync::pending_changes(&db, 10);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].collection, "sessions");
        let sync_id = changes[0].id.clone();

        let db2 = Db::open_memory().unwrap();
        let rec = WireRecord {
            collection: "sessions".into(),
            id: sync_id,
            fields: changes[0].fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            field_ts: changes[0].field_ts.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            seq: 1,
        };
        let arrived = SessionSync::apply(&db2, &[rec]);
        assert_eq!(arrived, 1);
        assert_eq!(repo::session_focus_seconds(&db2, 0.0, 2000.0), 1500);
    }
}
