use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn normalize_uuid_upper(raw: &str) -> Option<String> {
    uuid::Uuid::parse_str(raw.trim())
        .ok()
        .map(|u| u.hyphenated().encode_upper(&mut uuid::Uuid::encode_buffer()).to_string())
}

pub fn normalize_uuid_lower(raw: &str) -> Option<String> {
    uuid::Uuid::parse_str(raw.trim())
        .ok()
        .map(|u| u.hyphenated().encode_lower(&mut uuid::Uuid::encode_buffer()).to_string())
}

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

// ---------- Settings models ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardID {
    Tasks,
    Timer,
    Notepad,
    Events,
    Music,
}

impl CardID {
    pub const ALL: [CardID; 5] = [
        CardID::Tasks,
        CardID::Timer,
        CardID::Notepad,
        CardID::Events,
        CardID::Music,
    ];

    pub fn raw_value(self) -> &'static str {
        match self {
            CardID::Tasks => "tasks",
            CardID::Timer => "timer",
            CardID::Notepad => "notepad",
            CardID::Events => "events",
            CardID::Music => "music",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "tasks" => Some(CardID::Tasks),
            "timer" => Some(CardID::Timer),
            "notepad" => Some(CardID::Notepad),
            "events" => Some(CardID::Events),
            "music" => Some(CardID::Music),
            _ => None,
        }
    }

    pub fn preferred_width(self) -> f64 {
        match self {
            CardID::Tasks => 330.0,
            CardID::Timer => 236.0,
            CardID::Notepad => 220.0,
            CardID::Events => 224.0,
            CardID::Music => 236.0,
        }
    }

    pub fn on_by_default(self) -> bool {
        self != CardID::Music
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FakeNotchStyle {
    Off,
    Notch,
    Island,
}

impl FakeNotchStyle {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" => Some(FakeNotchStyle::Off),
            "notch" => Some(FakeNotchStyle::Notch),
            "island" => Some(FakeNotchStyle::Island),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSetting {
    pub card: CardID,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsData {
    pub cards: Vec<CardSetting>,

    // Tabs
    pub show_week_tab: bool,
    pub show_insights_tab: bool,

    // Timer
    pub show_pill: bool,
    pub default_focus_minutes: i64,
    pub timer_sound: bool,
    pub time_up_island: bool,

    // Reminders & calendar
    pub reminders_enabled: bool,
    pub snooze_minutes: i64,
    pub show_reminders_in_events: bool,
    pub ics_url: Option<String>,

    // Opening
    pub hover_to_open: bool,
    pub click_to_open: bool,
    pub hotkey_enabled: bool,

    // Music
    pub show_music_chip: bool,

    // Appearance
    pub paper_grain: bool,
    pub fake_notch: FakeNotchStyle,
    pub show_battery: bool,
    pub check_for_updates: bool,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            cards: CardID::ALL
                .iter()
                .map(|&card| CardSetting {
                    card,
                    enabled: card.on_by_default(),
                })
                .collect(),
            show_week_tab: true,
            show_insights_tab: true,
            show_pill: true,
            default_focus_minutes: 25,
            timer_sound: true,
            time_up_island: true,
            reminders_enabled: true,
            snooze_minutes: 5,
            show_reminders_in_events: true,
            ics_url: None,
            hover_to_open: true,
            click_to_open: false,
            hotkey_enabled: true,
            show_music_chip: false,
            paper_grain: true,
            // Default to floating island on Windows so the top-center island is immediately visible.
            fake_notch: FakeNotchStyle::Island,
            show_battery: true,
            check_for_updates: true,
        }
    }
}

impl SettingsData {
    /// Tolerant JSON decoder matching AppSettings.swift: missing or invalid keys fall back to defaults.
    pub fn from_json_tolerant(json: &str) -> Self {
        let mut d = Self::default();
        let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(json) else {
            return d;
        };

        if let Some(v) = map.get("cards") {
            if let Ok(cards) = serde_json::from_value::<Vec<CardSetting>>(v.clone()) {
                d.cards = cards;
            }
        }
        let get_bool = |k: &str, fallback: bool| -> bool {
            map.get(k).and_then(|v| v.as_bool()).unwrap_or(fallback)
        };
        let get_i64 = |k: &str, fallback: i64| -> i64 {
            map.get(k).and_then(|v| v.as_i64()).unwrap_or(fallback)
        };

        d.show_week_tab = get_bool("showWeekTab", d.show_week_tab);
        d.show_insights_tab = get_bool("showInsightsTab", d.show_insights_tab);
        d.show_pill = get_bool("showPill", d.show_pill);
        d.default_focus_minutes = get_i64("defaultFocusMinutes", d.default_focus_minutes).clamp(1, 240);
        d.timer_sound = get_bool("timerSound", d.timer_sound);
        d.time_up_island = get_bool("timeUpIsland", d.time_up_island);
        d.reminders_enabled = get_bool("remindersEnabled", d.reminders_enabled);
        d.snooze_minutes = get_i64("snoozeMinutes", d.snooze_minutes).clamp(1, 60);
        d.show_reminders_in_events = get_bool("showRemindersInEvents", d.show_reminders_in_events);
        d.ics_url = map
            .get("icsUrl")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty());
        d.hover_to_open = get_bool("hoverToOpen", d.hover_to_open);
        d.click_to_open = get_bool("clickToOpen", d.click_to_open);
        d.hotkey_enabled = get_bool("hotkeyEnabled", d.hotkey_enabled);
        d.show_music_chip = get_bool("showMusicChip", d.show_music_chip);
        d.paper_grain = get_bool("paperGrain", d.paper_grain);
        if let Some(v) = map.get("fakeNotch") {
            if let Ok(style) = serde_json::from_value::<FakeNotchStyle>(v.clone()) {
                d.fake_notch = style;
            }
        }
        d.show_battery = get_bool("showBattery", d.show_battery);
        d.check_for_updates = get_bool("checkForUpdates", d.check_for_updates);
        d.normalize_cards();
        d
    }

    /// Makes sure every card appears exactly once and at least one is on.
    pub fn normalize_cards(&mut self) {
        let mut seen = HashSet::new();
        self.cards.retain(|item| seen.insert(item.card));
        for card in CardID::ALL {
            if !seen.contains(&card) {
                self.cards.push(CardSetting {
                    card,
                    enabled: card.on_by_default(),
                });
            }
        }
        if !self.cards.iter().any(|c| c.enabled) && !self.cards.is_empty() {
            self.cards[0].enabled = true;
        }
    }
}

// ---------- Sync wire models (matching notchcat-server/app/schemas.py) ----------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JSONValue {
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    String(String),
}

impl JSONValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JSONValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JSONValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            JSONValue::Int(i) => Some(*i),
            JSONValue::Double(d) if d.fract() == 0.0 => Some(*d as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JSONValue::Double(d) => Some(*d),
            JSONValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn from_opt_i64(v: Option<i64>) -> Self {
        v.map(JSONValue::Int).unwrap_or(JSONValue::Null)
    }

    pub fn from_opt_f64(v: Option<f64>) -> Self {
        v.map(JSONValue::Double).unwrap_or(JSONValue::Null)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireChange {
    pub collection: String,
    pub id: String,
    pub fields: BTreeMap<String, JSONValue>,
    pub field_ts: BTreeMap<String, i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_seq: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushBody {
    pub changes: Vec<WireChange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireRecord {
    pub collection: String,
    pub id: String,
    pub fields: HashMap<String, JSONValue>,
    pub field_ts: HashMap<String, i64>,
    pub seq: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireRejected {
    pub collection: String,
    pub id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushResult {
    pub records: Vec<WireRecord>,
    #[serde(default)]
    pub rejected: Vec<WireRejected>,
    #[serde(default)]
    pub conflicts: Vec<WireRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PullResult {
    pub records: Vec<WireRecord>,
    pub cursor: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub user: AuthUser,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskSyncMeta {
    pub field_ts: HashMap<String, i64>,
    pub dirty: HashSet<String>,
    pub deleted: bool,
}
